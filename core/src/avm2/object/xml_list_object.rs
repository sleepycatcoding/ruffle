use crate::avm2::activation::Activation;
use crate::avm2::e4x::{E4XNode, E4XNodeKind};
use crate::avm2::object::script_object::ScriptObjectData;
use crate::avm2::object::{Object, ObjectPtr, TObject};
use crate::avm2::value::Value;
use crate::avm2::{Error, Multiname};
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
        target_object: Option<Object<'gc>>,
        target_property: Option<Multiname<'gc>>,
    ) -> Self {
        let base = ScriptObjectData::new(activation.context.avm2.classes().xml_list);
        XmlListObject(GcCell::new(
            activation.context.gc_context,
            XmlListObjectData {
                base,
                children,
                target_object,
                target_property
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

    pub fn target_object(&self) -> Option<Object<'gc>> {
        self.0.read().target_object
    }

    // ECMA-357 9.2.1.6 [[Append]] (V)
    pub fn append(&self, activation: &mut Activation<'_, 'gc>, object: Object<'gc>) {
        // 3. If Type(V) is XMLList,
        if let Some(xml_list) = object.as_xml_list_object() {
            let mut write = self.0.write(activation.gc());
            // 3.a. Let x.[[TargetObject]] = V.[[TargetObject]]
            write.target_object = xml_list.target_object();
            // 3.b. Let x.[[TargetProperty]] = V.[[TargetProperty]]
            write.target_property = xml_list.0.read().target_property.clone();

            // 3.c. Let n = V.[[Length]]
            // 3.d. If n == 0, Return
            if xml_list.length() == 0 {
                return;
            }

            // 3.e. For j = 0 to V.[[Length]]-1, let x[i + j] = V[j]
            for child in &*xml_list.children() {
                write.children.push(child.clone());
            }
        } else if let Some(xml) = object.as_xml_object() {
            let mut write = self.0.write(activation.gc());

            write.children.push(E4XOrXml::Xml(xml));
        }
    }

    // ECMA-357 9.2.1.10 [[ResolveValue]] ( )
    pub fn resolve_value(self, activation: &mut Activation<'_, 'gc>) -> Option<Object<'gc>> {
        // 1. If x.[[Length]] > 0, return x
        if self.length() > 0 {
            return Some(Object::XmlListObject(self));
        }

        let read = self.0.read();

        // 2. Else
        // 2.a. If (x.[[TargetObject]] == null) or (x.[[TargetProperty]] == null)
        let Some(target_object) = read.target_object else {
            return None;
        };
        let Some(target_property) = read.target_property.clone() else {
            return None;
        };

        // or (type(x.[[TargetProperty]]) is AttributeName) or (x.[[TargetProperty]].localName == "*")
        if target_property.is_attribute() || target_property.is_any_name() {
            return None;
        }

        // 2.b. Let base be the result of calling the [[ResolveValue]] method of x.[[TargetObject]] recursively
        let base = if let Some(xml_list) = target_object.as_xml_list_object() {
            // FIXME: These clones here are not the best, but lifetimes are hard.
            xml_list.resolve_value(activation).clone()
        } else if let Some(xml) = target_object.as_xml_object() {
            xml.resolve_value().clone()
        } else {
            unreachable!("Base should be XmlObject or XmlListObject");
        };

        // 2.c. If base == null, return null
        let Some(base) = base else {
            return None;
        };

        // 2.d. Let target be the result of calling [[Get]] on base with argument x.[[TargetProperty]]
        let mut target = base.get_property_local(&target_property, activation).ok()?.as_object()?;

        let length = if let Some(xml_list) = target.as_xml_list_object() {
            xml_list.length()
        } else if let Some(_xml) = target.as_xml_object() {
            // FIXME: This is probably incorrect, fix later.
            1
        } else {
            unreachable!("Target should be XmlListObject or XmlObject");
        };

        // 2.e. If (target.[[Length]] == 0)
        if length == 0 {
            // 2.e.i. If (Type(base) is XMLList) and (base.[[Length]] > 1), return null
            if let Some(xml_list) = base.as_xml_list_object() {
                if xml_list.length() > 1 {
                    return None;
                }
            }

            // 2.e.ii. Call [[Put]] on base with arguments x.[[TargetProperty]] and the empty string
            // FIXME: Return result.
            base.set_property_local(&target_property, "".into(), activation);

            // 2.e.iii. Let target be the result of calling [[Get]] on base with argument x.[[TargetProperty]]
            target = base.get_property_local(&target_property, activation).ok()?.as_object()?;
        }

        // 2.f. Return target
        Some(target)
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
            // FIXME: Check spec to see if we need to copy that to
            Self::new(activation, out, None, None)
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
    target_object: Option<Object<'gc>>,

    target_property: Option<Multiname<'gc>>,
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
        Some(XmlListObject::new(
            activation,
            descendants,
            Some((*self).into()),
            // FIXME: Check spec to see if we need to copy that to
            None,
        ))
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

        // FIXME: Check spec to see if we need to copy that to
        Ok(XmlListObject::new(activation, matched_children, Some(self.into()), None).into())
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
        value: Value<'gc>,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<(), Error<'gc>> {
        let mut write = self.0.write(activation.context.gc_context);

        // 1. Let i = ToUint32(P)
        // 2. If ToString(i) == P
        if !name.is_any_name() && !name.is_attribute() {
            if let Some(local_name) = name.local_name() {
                if let Ok(index) = local_name.parse::<usize>() {
                    // 2.a. If x.[[TargetObject]] is not null
                    if let Some(target) = write.target_object {
                        return Err(format!(
                            "Modifying an XMLList object is not yet implemented: target {:?}",
                            target
                        )
                        .into());
                    }

                    if index >= write.children.len() {
                        if let Some(value_xml) =
                            value.as_object().and_then(|obj| obj.as_xml_object())
                        {
                            write.children.push(E4XOrXml::Xml(value_xml));
                            return Ok(());
                        }
                    }

                    return Err(format!(
                        "Modifying an XMLList object is not supported yet for index {:?} = {:?}",
                        index, value
                    )
                    .into());
                }
            }
        }

        // 3. Else if x.[[Length]] is less than or equal to 1
        if write.children.len() <= 1 {
            // 3.a. If x.[[Length]] == 0
            if write.children.is_empty() {
                return Err(
                    "Modifying an XMLList object is not yet implemented: need to resolve".into(),
                );
            }

            // 3.b. Call the [[Put]] method of x[0] with arguments P and V
            let xml = write.children[0].get_or_create_xml(activation);
            return xml.set_property_local(name, value, activation);
        }

        // 4. Return
        Ok(())
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
