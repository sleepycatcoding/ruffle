use crate::avm2::activation::Activation;
use crate::avm2::e4x::{E4XNode, E4XNodeKind};
use crate::avm2::error::make_error_1089;
use crate::avm2::object::script_object::ScriptObjectData;
use crate::avm2::object::{Object, ObjectPtr, TObject};
use crate::avm2::value::Value;
use crate::avm2::{Error, Multiname, Namespace};
use gc_arena::{Collect, GcCell, GcWeakCell, Mutation};
use std::cell::{Ref, RefMut};
use std::fmt::{self, Debug};
use std::ops::Deref;

use super::{ClassObject, XmlObject};

/// A class instance allocator that allocates XMLList objects.
pub fn xml_list_allocator<'gc>(
    class: ClassObject<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> Result<Object<'gc>, Error<'gc>> {
    let base = ScriptObjectData::new(class);

    Ok(XmlListObject(GcCell::new(
        activation.context.gc_context,
        XmlListObjectData {
            base,
            children: Vec::new(),
            // An XMLList created by 'new XMLList()' is not linked
            // to any object
            target_object: None,
            target_property: None,
            target_dirty: false,
        },
    ))
    .into())
}

#[derive(Clone, Collect, Copy)]
#[collect(no_drop)]
pub struct XmlListObject<'gc>(pub GcCell<'gc, XmlListObjectData<'gc>>);

#[derive(Clone, Collect, Copy, Debug)]
#[collect(no_drop)]
pub struct XmlListObjectWeak<'gc>(pub GcWeakCell<'gc, XmlListObjectData<'gc>>);

impl<'gc> Debug for XmlListObject<'gc> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XmlListObject")
            .field("ptr", &self.0.as_ptr())
            .finish()
    }
}

impl<'gc> XmlListObject<'gc> {
    pub fn new(
        activation: &mut Activation<'_, 'gc>,
        children: Vec<E4XOrXml<'gc>>,
        target_object: Option<XmlOrXmlListObject<'gc>>,
        target_property: Option<Multiname<'gc>>,
    ) -> Self {
        let base = ScriptObjectData::new(activation.context.avm2.classes().xml_list);
        XmlListObject(GcCell::new(
            activation.context.gc_context,
            XmlListObjectData {
                base,
                children,
                target_object,
                target_property,
                target_dirty: false,
            },
        ))
    }

    /// Same as `new`, but with target object/property dirty flag set.
    pub fn new_dirty(
        activation: &mut Activation<'_, 'gc>,
        children: Vec<E4XOrXml<'gc>>,
        target_object: Option<XmlOrXmlListObject<'gc>>,
        target_property: Option<Multiname<'gc>>,
    ) -> Self {
        let base = ScriptObjectData::new(activation.context.avm2.classes().xml_list);
        XmlListObject(GcCell::new(
            activation.context.gc_context,
            XmlListObjectData {
                base,
                children,
                target_object,
                target_property,
                target_dirty: true,
            },
        ))
    }

    pub fn length(&self) -> usize {
        self.0.read().children.len()
    }

    pub fn xml_object_child(
        &self,
        index: usize,
        activation: &mut Activation<'_, 'gc>,
    ) -> Option<XmlObject<'gc>> {
        let mut write = self.0.write(activation.context.gc_context);
        if let Some(child) = write.children.get_mut(index) {
            Some(child.get_or_create_xml(activation))
        } else {
            None
        }
    }

    pub fn children(&self) -> Ref<'_, Vec<E4XOrXml<'gc>>> {
        Ref::map(self.0.read(), |d| &d.children)
    }

    pub fn children_mut(&self, mc: &Mutation<'gc>) -> RefMut<'_, Vec<E4XOrXml<'gc>>> {
        RefMut::map(self.0.write(mc), |d| &mut d.children)
    }

    pub fn set_children(&self, mc: &Mutation<'gc>, children: Vec<E4XOrXml<'gc>>) {
        self.0.write(mc).children = children;
    }

    pub fn target_object(&self) -> Option<XmlOrXmlListObject<'gc>> {
        self.0.read().target_object
    }

    pub fn target_property(&self) -> Option<Multiname<'gc>> {
        self.0.read().target_property.clone()
    }

    pub fn deep_copy(&self, activation: &mut Activation<'_, 'gc>) -> XmlListObject<'gc> {
        self.reevaluate_target_object(activation);

        let children = self
            .children()
            .iter()
            .map(|child| E4XOrXml::E4X(child.node().deep_copy(activation.context.gc_context)))
            .collect();
        XmlListObject::new(
            activation,
            children,
            self.target_object(),
            self.target_property(),
        )
    }

    // Based on https://github.com/adobe/avmplus/blob/858d034a3bd3a54d9b70909386435cf4aec81d21/core/XMLListObject.cpp#L621
    pub fn reevaluate_target_object(&self, activation: &mut Activation<'_, 'gc>) {
        let mut write = self.0.write(activation.gc());

        if write.target_dirty && !write.children.is_empty() {
            let last_node = *write
                .children
                .last()
                .expect("At least one child exists")
                .node();

            if let Some(parent) = last_node.parent() {
                if let Some(XmlOrXmlListObject::Xml(target_obj)) = write.target_object {
                    if !E4XNode::ptr_eq(*target_obj.node(), parent) {
                        write.target_object = Some(XmlObject::new(parent, activation).into());
                    }
                }
            } else {
                write.target_object = None;
            }

            if !matches!(*last_node.kind(), E4XNodeKind::ProcessingInstruction(_)) {
                if let Some(name) = last_node.local_name() {
                    let ns = match last_node.namespace() {
                        Some(ns) => Namespace::package(ns, &mut activation.context.borrow_gc()),
                        None => activation.avm2().public_namespace,
                    };

                    write.target_property = Some(Multiname::new(ns, name));
                }
            }

            write.target_dirty = false;
        }
    }

    // ECMA-357 9.2.1.6 [[Append]] (V)
    pub fn append(&self, value: Value<'gc>, activation: &mut Activation<'_, 'gc>) {
        let mut write = self.0.write(activation.gc());

        // 3. If Type(V) is XMLList,
        if let Some(list) = value.as_object().and_then(|x| x.as_xml_list_object()) {
            write.target_dirty = false;
            // 3.a. Let x.[[TargetObject]] = V.[[TargetObject]]
            write.target_object = list.target_object();
            // 3.b. Let x.[[TargetProperty]] = V.[[TargetProperty]]
            write.target_property = list.target_property();

            for el in &*list.children() {
                write.children.push(el.clone());
            }
        }

        if let Some(xml) = value.as_object().and_then(|x| x.as_xml_object()) {
            write.target_dirty = true;
            write.children.push(E4XOrXml::Xml(xml));
        }
    }

    // ECMA-357 9.2.1.10 [[ResolveValue]] ( )
    pub fn resolve_value(
        &self,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<Option<XmlOrXmlListObject<'gc>>, Error<'gc>> {
        // 1. If x.[[Length]] > 0, return x
        if self.length() > 0 {
            Ok(Some(XmlOrXmlListObject::XmlList(*self)))
        // 2. Else
        } else {
            self.reevaluate_target_object(activation);

            // 2.a. If (x.[[TargetObject]] == null)
            let Some(target_object) = self.target_object() else {
                // 2.a.i. Return null
                return Ok(None);
            };
            // or (x.[[TargetProperty]] == null)
            let Some(target_property) = self.target_property() else {
                // 2.a.i. Return null
                return Ok(None);
            };

            // or (type(x.[[TargetProperty]]) is AttributeName) or (x.[[TargetProperty]].localName == "*")
            if target_property.is_attribute() || target_property.is_any_name() {
                // 2.a.i. Return null
                return Ok(None);
            }

            // 2.b. Let base be the result of calling the [[ResolveValue]] method of x.[[TargetObject]] recursively
            let Some(base) = target_object.resolve_value(activation)? else {
                // 2.c. If base == null, return null
                return Ok(None);
            };

            // 2.d. Let target be the result of calling [[Get]] on base with argument x.[[TargetProperty]]
            let Some(target) = base.get_property_local(&target_property, activation)? else {
                // NOTE: Not specified in spec, but avmplus checks if null/undefined was returned, so we do the same, since there is
                //       an invariant in get_property_local of XmlListObject/XmlObject.
                return Ok(None);
            };

            // 2.e. If (target.[[Length]] == 0)
            if target.length().unwrap_or(0) == 0 {
                // 2.e.i. If (Type(base) is XMLList) and (base.[[Length]] > 1), return null
                if let XmlOrXmlListObject::XmlList(x) = &base {
                    if x.length() > 1 {
                        // NOTE: Not mentioned in the spec, but avmplus throws an Error 1089 here.
                        return Err(make_error_1089(activation));
                    }
                }

                // 2.e.ii. Call [[Put]] on base with arguments x.[[TargetProperty]] and the empty string
                base.as_object()
                    .set_property_local(&target_property, "".into(), activation)?;

                // 2.e.iii. Let target be the result of calling [[Get]] on base with argument x.[[TargetProperty]]
                return base.get_property_local(&target_property, activation);
            }

            // 2.f. Return target
            Ok(Some(target))
        }
    }

    pub fn equals(
        &self,
        other: &Value<'gc>,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<bool, Error<'gc>> {
        if *other == Value::Undefined && self.length() == 0 {
            return Ok(true);
        }

        if let Value::Object(obj) = other {
            if let Some(xml_list_obj) = obj.as_xml_list_object() {
                if self.length() != xml_list_obj.length() {
                    return Ok(false);
                }

                for n in 0..self.length() {
                    let value = xml_list_obj.xml_object_child(n, activation).unwrap().into();
                    if !self
                        .xml_object_child(n, activation)
                        .unwrap()
                        .abstract_eq(&value, activation)?
                    {
                        return Ok(false);
                    }
                }

                return Ok(true);
            }
        }

        if self.length() == 1 {
            return self
                .xml_object_child(0, activation)
                .unwrap()
                .abstract_eq(other, activation);
        }

        Ok(false)
    }

    pub fn concat(
        activation: &mut Activation<'_, 'gc>,
        left: XmlListObject<'gc>,
        right: XmlListObject<'gc>,
    ) -> XmlListObject<'gc> {
        if left.length() == 0 {
            right
        } else if right.length() == 0 {
            left
        } else {
            let mut out = vec![];
            out.extend(left.children().clone());
            out.extend(right.children().clone());
            Self::new_dirty(activation, out, None, None)
        }
    }
}

#[derive(Clone, Collect)]
#[collect(no_drop)]
pub struct XmlListObjectData<'gc> {
    /// Base script object
    base: ScriptObjectData<'gc>,

    /// The children stored by this list.
    children: Vec<E4XOrXml<'gc>>,

    /// The XML or XMLList object that this list was created from.
    /// If `Some`, then modifications to this list are reflected
    /// in the original object.
    target_object: Option<XmlOrXmlListObject<'gc>>,

    target_property: Option<Multiname<'gc>>,

    target_dirty: bool,
}

/// Holds either an `E4XNode` or an `XmlObject`. This can be converted
/// in-palce to an `XmlObject` via `get_or_create_xml`.
/// This deliberately does not implement `Copy`, since `get_or_create_xml`
/// takes `&mut self`
#[derive(Clone, Collect, Debug)]
#[collect(no_drop)]
pub enum E4XOrXml<'gc> {
    E4X(E4XNode<'gc>),
    Xml(XmlObject<'gc>),
}

impl<'gc> E4XOrXml<'gc> {
    pub fn get_or_create_xml(&mut self, activation: &mut Activation<'_, 'gc>) -> XmlObject<'gc> {
        match self {
            E4XOrXml::E4X(node) => {
                let xml = XmlObject::new(*node, activation);
                *self = E4XOrXml::Xml(xml);
                xml
            }
            E4XOrXml::Xml(xml) => *xml,
        }
    }

    pub fn node(&self) -> E4XWrapper<'_, 'gc> {
        match self {
            E4XOrXml::E4X(node) => E4XWrapper::E4X(*node),
            E4XOrXml::Xml(xml) => E4XWrapper::XmlRef(xml.node()),
        }
    }
}

// Allows using `E4XOrXml` as an `E4XNode` via deref coercions, while
// storing the needed `Ref` wrappers
#[derive(Debug)]
pub enum E4XWrapper<'a, 'gc> {
    E4X(E4XNode<'gc>),
    XmlRef(Ref<'a, E4XNode<'gc>>),
}

impl<'a, 'gc> Deref for E4XWrapper<'a, 'gc> {
    type Target = E4XNode<'gc>;

    fn deref(&self) -> &Self::Target {
        match self {
            E4XWrapper::E4X(node) => node,
            E4XWrapper::XmlRef(node) => node,
        }
    }
}

/// Represents either a XmlObject or a XmlListObject. Used
/// for resolving the value of empty XMLLists.
#[derive(Clone, Collect, Copy, Debug)]
#[collect(no_drop)]
pub enum XmlOrXmlListObject<'gc> {
    XmlList(XmlListObject<'gc>),
    Xml(XmlObject<'gc>),
}

impl<'gc> XmlOrXmlListObject<'gc> {
    pub fn length(&self) -> Option<usize> {
        match self {
            XmlOrXmlListObject::Xml(x) => x.length(),
            XmlOrXmlListObject::XmlList(x) => Some(x.length()),
        }
    }

    pub fn resolve_value(
        &self,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<Option<XmlOrXmlListObject<'gc>>, Error<'gc>> {
        match self {
            // NOTE: XmlObjects just resolve to themselves.
            XmlOrXmlListObject::Xml(x) => Ok(Some(XmlOrXmlListObject::Xml(*x))),
            XmlOrXmlListObject::XmlList(x) => x.resolve_value(activation),
        }
    }

    pub fn as_object(&self) -> Object<'gc> {
        match self {
            XmlOrXmlListObject::Xml(x) => Object::XmlObject(*x),
            XmlOrXmlListObject::XmlList(x) => Object::XmlListObject(*x),
        }
    }

    pub fn get_property_local(
        &self,
        name: &Multiname<'gc>,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<Option<XmlOrXmlListObject<'gc>>, Error<'gc>> {
        let value = self.as_object().get_property_local(name, activation)?;

        if let Some(xml) = value.as_object().and_then(|x| x.as_xml_object()) {
            return Ok(Some(XmlOrXmlListObject::Xml(xml)));
        }

        if let Some(list) = value.as_object().and_then(|x| x.as_xml_list_object()) {
            return Ok(Some(XmlOrXmlListObject::XmlList(list)));
        }

        if matches!(value, Value::Null | Value::Undefined) {
            return Ok(None);
        }

        unreachable!(
            "Invalid value {:?}, expected XmlListObject/XmlObject or a null value",
            value
        );
    }
}

impl<'gc> From<XmlListObject<'gc>> for XmlOrXmlListObject<'gc> {
    fn from(value: XmlListObject<'gc>) -> XmlOrXmlListObject<'gc> {
        XmlOrXmlListObject::XmlList(value)
    }
}

impl<'gc> From<XmlObject<'gc>> for XmlOrXmlListObject<'gc> {
    fn from(value: XmlObject<'gc>) -> XmlOrXmlListObject<'gc> {
        XmlOrXmlListObject::Xml(value)
    }
}

impl<'gc> TObject<'gc> for XmlListObject<'gc> {
    fn base(&self) -> Ref<ScriptObjectData<'gc>> {
        Ref::map(self.0.read(), |read| &read.base)
    }

    fn base_mut(&self, mc: &Mutation<'gc>) -> RefMut<ScriptObjectData<'gc>> {
        RefMut::map(self.0.write(mc), |write| &mut write.base)
    }

    fn as_ptr(&self) -> *const ObjectPtr {
        self.0.as_ptr() as *const ObjectPtr
    }

    fn value_of(&self, _mc: &Mutation<'gc>) -> Result<Value<'gc>, Error<'gc>> {
        Ok(Value::Object(Object::from(*self)))
    }

    fn as_xml_list_object(&self) -> Option<Self> {
        Some(*self)
    }

    fn xml_descendants(
        &self,
        activation: &mut Activation<'_, 'gc>,
        multiname: &Multiname<'gc>,
    ) -> Option<XmlListObject<'gc>> {
        let mut descendants = Vec::new();
        for child in self.0.read().children.iter() {
            child.node().descendants(multiname, &mut descendants);
        }
        Some(XmlListObject::new(activation, descendants, None, None))
    }

    fn get_property_local(
        self,
        name: &Multiname<'gc>,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<Value<'gc>, Error<'gc>> {
        // FIXME - implement everything from E4X spec (XMLListObject::getMultinameProperty in avmplus)
        let mut write = self.0.write(activation.context.gc_context);

        if !name.has_explicit_namespace() {
            if let Some(local_name) = name.local_name() {
                if let Ok(index) = local_name.parse::<usize>() {
                    if let Some(child) = write.children.get_mut(index) {
                        return Ok(Value::Object(child.get_or_create_xml(activation).into()));
                    } else {
                        return Ok(Value::Undefined);
                    }
                }
            }
        }

        let matched_children = write
            .children
            .iter_mut()
            .flat_map(|child| {
                let child_prop = child
                    .get_or_create_xml(activation)
                    .get_property_local(name, activation)
                    .unwrap();
                if let Some(prop_xml) = child_prop.as_object().and_then(|obj| obj.as_xml_object()) {
                    vec![E4XOrXml::Xml(prop_xml)]
                } else if let Some(prop_xml_list) = child_prop
                    .as_object()
                    .and_then(|obj| obj.as_xml_list_object())
                {
                    // Flatten children
                    prop_xml_list.children().clone()
                } else {
                    vec![]
                }
            })
            .collect();

        Ok(XmlListObject::new(
            activation,
            matched_children,
            Some(self.into()),
            Some(name.clone()),
        )
        .into())
    }

    fn call_property_local(
        self,
        multiname: &Multiname<'gc>,
        arguments: &[Value<'gc>],
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<Value<'gc>, Error<'gc>> {
        let method = self
            .proto()
            .expect("XMLList missing prototype")
            .get_property(multiname, activation)?;

        // See https://github.com/adobe/avmplus/blob/858d034a3bd3a54d9b70909386435cf4aec81d21/core/XMLListObject.cpp#L50
        // in avmplus.
        // If we have exactly one child, then we forward the method to the child,
        // so long as none of our *children* have a property matching the method name
        // (it doesn't matter if a child's *name* matches, because XMLList methods work
        //  by running an operation on each child. For example,
        // 'new XMLList('<child attr="Outer"><name attr="Inner"></name</child>').name'
        // gives us back an XMLList with '<name attr=Inner></name>'
        //
        // It seems like it may be unnecessary to check if any of our children contain
        // a property matching the method name:
        // * XMLList defines all of the same methods as XML on its prototype (e.g. 'name', 'nodeType', etc.)
        //   If we're attempting to call one of these XML-related methods, then we'll find it on the prototype
        //   in the above check.
        // * If we're calling a method that *doesn't* exist on the prototype, it must not be an XML-related
        //   method. In that case, the method will only be callable on our XML child if the child has simple
        //   content (as we'll automatically convert it to a String, and call the method on that String).
        // * However, in order for a child to have a property matching the meethod name, it must be
        //   a non-simple XML object (simple XML objects have no properties to match).
        //
        // Nevertheless, there may be some weird edge case where this actually matters.
        // To be safe, we'll just perform exactly the same check that avmplus does.
        if matches!(method, Value::Undefined) {
            let prop = self.get_property_local(multiname, activation)?;
            if let Some(list) = prop.as_object().and_then(|obj| obj.as_xml_list_object()) {
                if list.length() == 0 && self.length() == 1 {
                    let mut this = self.0.write(activation.context.gc_context);
                    return this.children[0]
                        .get_or_create_xml(activation)
                        .call_property(multiname, arguments, activation);
                }
            }
        }

        return method
            .as_callable(activation, Some(multiname), Some(self.into()), false)?
            .call(self.into(), arguments, activation);
    }

    // ECMA-357 9.2.1.2 [[Put]] (P, V)
    fn set_property_local(
        self,
        name: &Multiname<'gc>,
        mut value: Value<'gc>,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<(), Error<'gc>> {
        // 1. Let i = ToUint32(P)
        // 2. If ToString(i) == P
        if !name.is_any_name() && !name.is_attribute() {
            if let Some(local_name) = name.local_name() {
                if let Ok(mut index) = local_name.parse::<usize>() {
                    self.reevaluate_target_object(activation);

                    // 2.a. If x.[[TargetObject]] is not null
                    let r = if let Some(target) = self.target_object() {
                        // 2.a.i. Let r be the result of calling the [[ResolveValue]] method of x.[[TargetObject]]
                        let r = target.resolve_value(activation)?;

                        // 2.a.ii. If r == null, return
                        let Some(r) = r else {
                            return Ok(());
                        };

                        Some(r)
                    // 2.b. Else let r = null
                    } else {
                        None
                    };

                    // 2.c. If i is greater than or equal to x.[[Length]]
                    if index >= self.length() {
                        let r = match r {
                            Some(XmlOrXmlListObject::Xml(x)) => Some(*x.node()),
                            // 2.c.i. If Type(r) is XMLList
                            Some(XmlOrXmlListObject::XmlList(x)) => {
                                // 2.c.i.1. If r.[[Length]] is not equal to 1, return
                                if x.length() != 1 {
                                    return Ok(());
                                }

                                // 2.c.i.2. Else let r = r[0]
                                Some(*x.children()[0].node())
                            }
                            None => None,
                        };

                        // 2.c.ii. If r.[[Class]] is not equal to "element", return
                        if let Some(r) = r {
                            if !matches!(*r.kind(), E4XNodeKind::Element { .. }) {
                                return Ok(());
                            }
                        }

                        // 2.c.iii Create a new XML object y with y.[[Parent]] = r, y.[[Name]] = x.[[TargetProperty]],
                        //         y.[[Attributes]] = {}, y.[[Length]] = 0
                        let y = match self.target_property() {
                            // 2.c.iv. If Type(x.[[TargetProperty]]) is AttributeName
                            Some(x) if x.is_attribute() => {
                                // 2.c.iv.1. Let attributeExists be the result of calling the [[Get]] method of r with argument y.[[Name]]
                                let attribute_exists = XmlObject::new(r.unwrap(), activation)
                                    .get_property_local(&x, activation)?;

                                // 2.c.iv.2. If (attributeExists.[[Length]] > 0), return
                                if let Some(list) = attribute_exists
                                    .as_object()
                                    .and_then(|x| x.as_xml_list_object())
                                {
                                    if list.length() > 0 {
                                        return Ok(());
                                    }
                                }

                                // 2.c.iv.3. Let y.[[Class]] = "attribute"
                                E4XNode::attribute(
                                    activation.gc(),
                                    x.local_name().unwrap(),
                                    "".into(),
                                    r,
                                )
                            }
                            // 2.c.v. Else if x.[[TargetProperty]] == null or x.[[TargetProperty]].localName == "*"
                            // 2.c.v.1. Let y.[[Name]] = null
                            // 2.c.v.2. Let y.[[Class]] = "text"
                            Some(x) if x.is_any_name() => {
                                E4XNode::text(activation.gc(), "".into(), r)
                            }
                            None => E4XNode::text(activation.gc(), "".into(), r),
                            // 2.c.vi. Else let y.[[Class]] = "element"
                            Some(property) => E4XNode::element(
                                activation.gc(),
                                property.explict_namespace(),
                                property.local_name().expect("Local name should exist"),
                                r,
                            ),
                        };

                        // 2.c.vii. Let i = x.[[Length]]
                        index = self.length();

                        // 2.c.viii. If (y.[[Class]] is not equal to "attribute")
                        if !matches!(*y.kind(), E4XNodeKind::Attribute(_)) {
                            // 2.c.viii.1. If r is not null
                            if let Some(r) = r {
                                let j = if let E4XNodeKind::Element { children, .. } = &*r.kind() {
                                    // 2.c.viii.1.a. If (i > 0)
                                    let j = if index > 0 {
                                        // 2.c.viii.1.a.i. Let j = 0
                                        let mut j = 0;

                                        // 2.c.viii.1.a.ii. While (j < r.[[Length]]-1) and (r[j] is not the same object as x[i-1])
                                        while j < children.len() - 1
                                            && !E4XNode::ptr_eq(
                                                children[j],
                                                *self.children()[index - 1].node(),
                                            )
                                        {
                                            // 2.c.viii.1.a.ii.1. Let j = j + 1
                                            j += 1;
                                        }

                                        // NOTE: Not listed in spec, but avmplus does this, so we do the same.
                                        j + 1
                                    // 2.c.viii.1.b. Else
                                    } else {
                                        // 2.c.viii.1.b.i. Let j = r.[[Length]]-1
                                        children.len()
                                    };

                                    Some(j)
                                } else {
                                    None
                                };

                                // NOTE: This is to bypass borrow errors.
                                if let Some(j) = j {
                                    // 2.c.viii.1.c. Call the [[Insert]] method of r with arguments ToString(j+1) and y
                                    r.insert(j, XmlObject::new(y, activation).into(), activation)?;
                                }
                            }

                            // 2.c.viii.2. If Type(V) is XML, let y.[[Name]] = V.[[Name]]
                            if let Some(xml) = value.as_object().and_then(|x| x.as_xml_object()) {
                                // FIXME: What if XML value does not have a local name?
                                y.set_local_name(
                                    xml.node().local_name().expect("Not validated yet"),
                                    activation.gc(),
                                );
                                // FIXME: Also set the namespace.
                            }

                            // 2.c.viii.3. Else if Type(V) is XMLList, let y.[[Name]] = V.[[TargetProperty]]
                            if let Some(list) =
                                value.as_object().and_then(|x| x.as_xml_list_object())
                            {
                                // FIXME: What if XMLList does not have a target property.
                                let target_property =
                                    list.target_property().expect("Not validated yet");

                                if let Some(name) = target_property.local_name() {
                                    y.set_local_name(name, activation.gc());
                                }
                                if let Some(namespace) = target_property.explict_namespace() {
                                    y.set_namespace(namespace, activation.gc());
                                }
                            }
                        }

                        // 2.c.ix. Call the [[Append]] method of x with argument y
                        self.append(XmlObject::new(y, activation).into(), activation);
                    }

                    // 2.d. If (Type(V) ∉ {XML, XMLList}) or (V.[[Class]] ∈ {"text", "attribute"}), let V = ToString(V)
                    if let Some(list) = value.as_object().and_then(|x| x.as_xml_list_object()) {
                        if list.length() == 1 {
                            let xml = list
                                .xml_object_child(0, activation)
                                .expect("List length was just verified");

                            if matches!(
                                *xml.node().kind(),
                                E4XNodeKind::Attribute(_) | E4XNodeKind::Text(_)
                            ) {
                                value = Value::Object(xml.into())
                                    .coerce_to_string(activation)?
                                    .into();
                            }
                        }
                    } else if let Some(xml) = value.as_object().and_then(|x| x.as_xml_object()) {
                        if matches!(
                            *xml.node().kind(),
                            E4XNodeKind::Attribute(_) | E4XNodeKind::Text(_)
                        ) {
                            value = value.coerce_to_string(activation)?.into();
                        }
                    } else {
                        value = value.coerce_to_string(activation)?.into();
                    }

                    // NOTE: Get x[i] for future operations. Also we need to drop ref to the children as we need to borrow as mutable later.
                    let children = self.children();
                    let child = *children[index].node();
                    drop(children);

                    // 2.e. If x[i].[[Class]] == "attribute"
                    if matches!(*child.kind(), E4XNodeKind::Attribute(_)) {
                        // FIXME: We probably need to take the namespace too.
                        // 2.e.i. Let z = ToAttributeName(x[i].[[Name]])
                        let z = Multiname::attribute(
                            activation.avm2().public_namespace,
                            child.local_name().expect("Attribute should have a name"),
                        );
                        // 2.e.ii. Call the [[Put]] method of x[i].[[Parent]] with arguments z and V
                        if let Some(parent) = child.parent() {
                            let parent = XmlObject::new(parent, activation);
                            parent.set_property_local(&z, value, activation)?;

                            // 2.e.iii. Let attr be the result of calling [[Get]] on x[i].[[Parent]] with argument z
                            let attr = parent
                                .get_property_local(&z, activation)?
                                .as_object()
                                .and_then(|x| x.as_xml_list_object())
                                .expect("XmlObject get_property_local should return XmlListObject");
                            // 2.e.iv. Let x[i] = attr[0]
                            self.children_mut(activation.gc())[index] = attr.children()[0].clone();
                        }
                    // 2.f. Else if Type(V) is XMLList
                    } else if let Some(list) =
                        value.as_object().and_then(|x| x.as_xml_list_object())
                    {
                        // 2.f.i. Create a shallow copy c of V
                        let c = XmlListObject::new(
                            activation,
                            list.children().clone(),
                            list.target_object(),
                            list.target_property(),
                        );
                        // 2.f.ii. Let parent = x[i].[[Parent]]
                        let parent = child.parent();

                        // 2.f.iii. If parent is not null
                        if let Some(parent) = parent {
                            // 2.f.iii.1. Let q be the property of parent, such that parent[q] is the same object as x[i]
                            let q = if let E4XNodeKind::Element { children, .. } = &*parent.kind() {
                                children.iter().position(|x| E4XNode::ptr_eq(*x, child))
                            } else {
                                None
                            };

                            if let Some(q) = q {
                                // 2.f.iii.2. Call the [[Replace]] method of parent with arguments q and c
                                parent.replace(q, c.into(), activation)?;

                                let E4XNodeKind::Element { children, .. } = &*parent.kind() else {
                                    unreachable!()
                                };

                                // 2.f.iii.3. For j = 0 to c.[[Length]]-1
                                for (index, child) in
                                    c.children_mut(activation.gc()).iter_mut().enumerate()
                                {
                                    // 2.f.iii.3.a. Let c[j] = parent[ToUint32(q)+j]
                                    *child = E4XOrXml::E4X(children[q + index]);
                                }
                            }

                            let mut children = self.children_mut(activation.gc());
                            children.remove(index);
                            for (index2, child) in c.children().iter().enumerate() {
                                children.insert(index + index2, child.clone());
                            }
                        }
                    // 2.g. Else if (Type(V) is XML) or (x[i].[[Class]] ∈ {"text", "comment", "processing-instruction"})
                    } else if value
                        .as_object()
                        .map_or(false, |x| x.as_xml_object().is_some())
                        || matches!(
                            *child.kind(),
                            E4XNodeKind::Text(_)
                                | E4XNodeKind::Comment(_)
                                | E4XNodeKind::ProcessingInstruction(_)
                                | E4XNodeKind::CData(_)
                        )
                    {
                        // 2.g.i. Let parent = x[i].[[Parent]]
                        let parent = child.parent();

                        // 2.g.ii. If parent is not null
                        if let Some(parent) = parent {
                            // 2.g.ii.1. Let q be the property of parent, such that parent[q] is the same object as x[i]
                            let q = if let E4XNodeKind::Element { children, .. } = &*parent.kind() {
                                children.iter().position(|x| E4XNode::ptr_eq(*x, child))
                            } else {
                                None
                            };

                            if let Some(q) = q {
                                // 2.g.ii.2. Call the [[Replace]] method of parent with arguments q and V
                                parent.replace(q, value, activation)?;

                                let E4XNodeKind::Element { children, .. } = &*parent.kind() else {
                                    unreachable!()
                                };

                                // 2.g.ii.3. Let V = parent[q]
                                value = XmlObject::new(children[q], activation).into();
                            }
                        }

                        let mut children = self.children_mut(activation.gc());
                        // NOTE: Avmplus does not follow the spec here, it instead checks if value is XML
                        //       and sets it, otherwise uses ToXML (our closest equivalent is the XML constructor).
                        if let Some(xml) = value.as_object().and_then(|x| x.as_xml_object()) {
                            children[index] = E4XOrXml::Xml(xml);
                        } else {
                            let xml = activation
                                .avm2()
                                .classes()
                                .xml
                                .construct(activation, &[value])?
                                .as_xml_object()
                                .expect("Should be XML Object");
                            children[index] = E4XOrXml::Xml(xml);
                        }
                    // 2.h. Else
                    } else {
                        // 2.h.i. Call the [[Put]] method of x[i] with arguments "*" and V
                        self.xml_object_child(index, activation)
                            .unwrap()
                            .set_property_local(
                                &Multiname::any(activation.gc()),
                                value,
                                activation,
                            )?;
                    }

                    // NOTE: Not specified in the spec, but avmplus returns here, so we do the same.
                    return Ok(());
                }
            }
        }

        // 3. Else if x.[[Length]] is less than or equal to 1
        if self.length() <= 1 {
            // 3.a. If x.[[Length]] == 0
            if self.length() == 0 {
                // 3.a.i. Let r be the result of calling the [[ResolveValue]] method of x
                let r = self.resolve_value(activation)?;

                // 3.a.ii. If (r == null)
                let Some(r) = r else {
                    return Ok(());
                };

                // or (r.[[Length]] is not equal to 1), return
                if r.length().unwrap_or(0) != 1 {
                    return Ok(());
                }

                // 3.a.iii. Call the [[Append]] method of x with argument r
                self.append(r.as_object().into(), activation);
            }

            let mut write = self.0.write(activation.gc());

            // 3.b. Call the [[Put]] method of x[0] with arguments P and V
            let xml = write.children[0].get_or_create_xml(activation);
            return xml.set_property_local(name, value, activation);
        }

        // 4. Return
        Err(make_error_1089(activation))
    }

    fn get_next_enumerant(
        self,
        last_index: u32,
        _activation: &mut Activation<'_, 'gc>,
    ) -> Result<Option<u32>, Error<'gc>> {
        let read = self.0.read();
        if (last_index as usize) < read.children.len() {
            return Ok(Some(last_index + 1));
        }
        // Return `Some(0)` instead of `None`, as we do *not* want to
        // fall back to the prototype chain. XMLList is special, and enumeration
        // *only* ever considers the XML children.
        Ok(Some(0))
    }

    fn get_enumerant_value(
        self,
        index: u32,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<Value<'gc>, Error<'gc>> {
        let mut write = self.0.write(activation.context.gc_context);
        let children_len = write.children.len() as u32;

        if children_len >= index {
            Ok(index
                .checked_sub(1)
                .map(|index| {
                    write.children[index as usize]
                        .get_or_create_xml(activation)
                        .into()
                })
                .unwrap_or(Value::Undefined))
        } else {
            Ok(Value::Undefined)
        }
    }

    fn get_enumerant_name(
        self,
        index: u32,
        _activation: &mut Activation<'_, 'gc>,
    ) -> Result<Value<'gc>, Error<'gc>> {
        let children_len = self.0.read().children.len() as u32;
        if children_len >= index {
            Ok(index
                .checked_sub(1)
                .map(|index| index.into())
                .unwrap_or(Value::Undefined))
        } else {
            Ok(self
                .base()
                .get_enumerant_name(index - children_len)
                .unwrap_or(Value::Undefined))
        }
    }

    fn delete_property_local(
        self,
        activation: &mut Activation<'_, 'gc>,
        name: &Multiname<'gc>,
    ) -> Result<bool, Error<'gc>> {
        let mut write = self.0.write(activation.context.gc_context);

        if !name.is_any_name() && !name.is_attribute() {
            if let Some(local_name) = name.local_name() {
                if let Ok(index) = local_name.parse::<usize>() {
                    if index < write.children.len() {
                        let removed = write.children.remove(index);
                        let removed_node = removed.node();
                        if let Some(parent) = removed_node.parent() {
                            if let E4XNodeKind::Attribute(_) = &*removed_node.kind() {
                                parent
                                    .remove_attribute(activation.context.gc_context, &removed_node);
                            } else {
                                parent.remove_child(activation.context.gc_context, &removed_node);
                            }
                        }
                    }
                    return Ok(true);
                }
            }
        }

        for child in write.children.iter_mut() {
            if matches!(&*child.node().kind(), E4XNodeKind::Element { .. }) {
                child
                    .get_or_create_xml(activation)
                    .delete_property_local(activation, name)?;
            }
        }

        Ok(true)
    }
}
