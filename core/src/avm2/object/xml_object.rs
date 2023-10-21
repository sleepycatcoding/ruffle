//! Object representation for XML objects

use crate::avm2::activation::Activation;
use crate::avm2::e4x::{name_to_multiname, E4XNode, E4XNodeKind};
use crate::avm2::error::make_error_1087;
use crate::avm2::object::script_object::ScriptObjectData;
use crate::avm2::object::{ClassObject, Object, ObjectPtr, TObject, XmlListObject};
use crate::avm2::string::AvmString;
use crate::avm2::value::Value;
use crate::avm2::Namespace;
use crate::avm2::{Error, Multiname};
use core::fmt;
use gc_arena::{Collect, GcCell, GcWeakCell, Mutation};
use ruffle_wstr::WString;
use std::cell::{Ref, RefMut};

use super::xml_list_object::{E4XOrXml, XmlOrXmlListObject};
use super::PrimitiveObject;

/// A class instance allocator that allocates XML objects.
pub fn xml_allocator<'gc>(
    class: ClassObject<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> Result<Object<'gc>, Error<'gc>> {
    let base = ScriptObjectData::new(class);

    Ok(XmlObject(GcCell::new(
        activation.context.gc_context,
        XmlObjectData {
            base,
            node: E4XNode::dummy(activation.context.gc_context),
        },
    ))
    .into())
}

#[derive(Clone, Collect, Copy)]
#[collect(no_drop)]
pub struct XmlObject<'gc>(pub GcCell<'gc, XmlObjectData<'gc>>);

#[derive(Clone, Collect, Copy, Debug)]
#[collect(no_drop)]
pub struct XmlObjectWeak<'gc>(pub GcWeakCell<'gc, XmlObjectData<'gc>>);

impl fmt::Debug for XmlObject<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XmlObject")
            .field("ptr", &self.0.as_ptr())
            .finish()
    }
}

#[derive(Clone, Collect)]
#[collect(no_drop)]
pub struct XmlObjectData<'gc> {
    /// Base script object
    base: ScriptObjectData<'gc>,

    node: E4XNode<'gc>,
}

impl<'gc> XmlObject<'gc> {
    pub fn new(node: E4XNode<'gc>, activation: &mut Activation<'_, 'gc>) -> Self {
        XmlObject(GcCell::new(
            activation.context.gc_context,
            XmlObjectData {
                base: ScriptObjectData::new(activation.context.avm2.classes().xml),
                node,
            },
        ))
    }

    pub fn length(&self) -> Option<usize> {
        self.node().length()
    }

    pub fn set_node(&self, mc: &Mutation<'gc>, node: E4XNode<'gc>) {
        self.0.write(mc).node = node;
    }

    pub fn local_name(&self) -> Option<AvmString<'gc>> {
        self.0.read().node.local_name()
    }

    pub fn namespace(&self, activation: &mut Activation<'_, 'gc>) -> Namespace<'gc> {
        match self.0.read().node.namespace() {
            Some(ns) => Namespace::package(ns, &mut activation.context.borrow_gc()),
            None => activation.avm2().public_namespace,
        }
    }

    pub fn matches_name(&self, multiname: &Multiname<'gc>) -> bool {
        self.0.read().node.matches_name(multiname)
    }

    pub fn node(&self) -> Ref<'_, E4XNode<'gc>> {
        Ref::map(self.0.read(), |data| &data.node)
    }

    pub fn deep_copy(&self, activation: &mut Activation<'_, 'gc>) -> XmlObject<'gc> {
        let node = self.node();
        XmlObject::new(node.deep_copy(activation.gc()), activation)
    }

    pub fn child(
        &self,
        activation: &mut Activation<'_, 'gc>,
        name: &Multiname<'gc>,
    ) -> XmlListObject<'gc> {
        let children = if let E4XNodeKind::Element { children, .. } = &*self.node().kind() {
            if let Some(local_name) = name.local_name() {
                if let Ok(index) = local_name.parse::<usize>() {
                    let children = if let Some(node) = children.get(index) {
                        vec![E4XOrXml::E4X(*node)]
                    } else {
                        Vec::new()
                    };
                    return XmlListObject::new(activation, children, None, None);
                }
            }

            children
                .iter()
                .filter(|node| node.matches_name(&name))
                .map(|node| E4XOrXml::E4X(*node))
                .collect()
        } else {
            Vec::new()
        };

        // FIXME: If name is not a number index, then we should call [[Get]] (get_property_local) with the name.
        XmlListObject::new(
            activation,
            children,
            Some(XmlOrXmlListObject::Xml(*self)),
            Some(name.clone()),
        )
    }

    pub fn equals(
        &self,
        other: &Value<'gc>,
        _activation: &mut Activation<'_, 'gc>,
    ) -> Result<bool, Error<'gc>> {
        // 1. If Type(V) is not XML, return false.
        let other = if let Some(xml_obj) = other.as_object().and_then(|obj| obj.as_xml_object()) {
            xml_obj
        } else {
            return Ok(false);
        };

        // It seems like an XML object should always be equal to itself
        if Object::ptr_eq(*self, other) {
            return Ok(true);
        }

        let node = other.node();
        Ok(self.node().equals(&node))
    }

    // Implements "The Abstract Equality Comparison Algorithm" as defined
    // in ECMA-357 when one side is an XML type (object).
    pub fn abstract_eq(
        &self,
        other: &Value<'gc>,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<bool, Error<'gc>> {
        // 3.a. If both x and y are the same type (XML)
        if let Value::Object(obj) = other {
            if let Some(xml_obj) = obj.as_xml_object() {
                if (matches!(
                    &*self.node().kind(),
                    E4XNodeKind::Text(_) | E4XNodeKind::CData(_) | E4XNodeKind::Attribute(_)
                ) && xml_obj.node().has_simple_content())
                    || (matches!(
                        &*xml_obj.node().kind(),
                        E4XNodeKind::Text(_) | E4XNodeKind::CData(_) | E4XNodeKind::Attribute(_)
                    ) && self.node().has_simple_content())
                {
                    return Ok(self.node().xml_to_string(activation)
                        == xml_obj.node().xml_to_string(activation));
                }

                return self.equals(other, activation);
            }
        }

        // 4. If (Type(x) is XML) and x.hasSimpleContent() == true)
        if self.node().has_simple_content() {
            return Ok(self.node().xml_to_string(activation) == other.coerce_to_string(activation)?);
        }

        // It seems like everything else will just ultimately fall-through to the last step.
        Ok(false)
    }
}

impl<'gc> TObject<'gc> for XmlObject<'gc> {
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

    fn as_xml_object(&self) -> Option<Self> {
        Some(*self)
    }

    fn xml_descendants(
        &self,
        activation: &mut Activation<'_, 'gc>,
        multiname: &Multiname<'gc>,
    ) -> Option<XmlListObject<'gc>> {
        let mut descendants = Vec::new();
        self.0.read().node.descendants(multiname, &mut descendants);
        Some(XmlListObject::new(activation, descendants, None, None))
    }

    fn get_property_local(
        self,
        name: &Multiname<'gc>,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<Value<'gc>, Error<'gc>> {
        // FIXME - implement everything from E4X spec (XMLObject::getMultinameProperty in avmplus)
        let read = self.0.read();

        if !name.has_explicit_namespace() {
            if let Some(local_name) = name.local_name() {
                // The only supported numerical index is 0
                if let Ok(index) = local_name.parse::<usize>() {
                    if index == 0 {
                        return Ok(self.into());
                    } else {
                        return Ok(Value::Undefined);
                    }
                }
            }
        }

        let matched_children = if let E4XNodeKind::Element {
            children,
            attributes,
        } = &*read.node.kind()
        {
            let search_children = if name.is_attribute() {
                attributes
            } else {
                children
            };

            search_children
                .iter()
                .filter_map(|child| {
                    if child.matches_name(name) {
                        Some(E4XOrXml::E4X(*child))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        return Ok(XmlListObject::new(
            activation,
            matched_children,
            Some(self.into()),
            Some(name.clone()),
        )
        .into());
    }

    fn call_property_local(
        self,
        multiname: &Multiname<'gc>,
        arguments: &[Value<'gc>],
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<Value<'gc>, Error<'gc>> {
        let this = self.as_xml_object().unwrap();

        let method = self
            .proto()
            .expect("XMLList misisng prototype")
            .get_property(multiname, activation)?;

        // If the method doesn't exist on the prototype, and we have simple content,
        // then coerce this XML to a string and call the method on that.
        // This lets things like `new XML("<p>Hello world</p>").split(" ")` work.
        if matches!(method, Value::Undefined) {
            // Checking if we have a child with the same name as the method is probably
            // unecessary - if we had such a child, then we wouldn't have simple content,
            // so we already would bail out before calling the method. Nevertheless,
            // avmplus has this check, so we do it out of an abundance of caution.
            // Compare to the very similar case in XMLListObject::call_property_local
            let prop = self.get_property_local(multiname, activation)?;
            if let Some(list) = prop.as_object().and_then(|obj| obj.as_xml_list_object()) {
                if list.length() == 0 && this.node().has_simple_content() {
                    let receiver = PrimitiveObject::from_primitive(
                        this.node().xml_to_string(activation).into(),
                        activation,
                    )?;
                    return receiver.call_property(multiname, arguments, activation);
                }
            }
        }

        return method
            .as_callable(activation, Some(multiname), Some(self.into()), false)?
            .call(self.into(), arguments, activation);
    }

    fn has_own_property(self, name: &Multiname<'gc>) -> bool {
        let read = self.0.read();

        // FIXME - see if we can deduplicate this with get_property_local in
        // an efficient way
        if !name.has_explicit_namespace() {
            if let Some(local_name) = name.local_name() {
                // The only supported numerical index is 0
                if let Ok(index) = local_name.parse::<usize>() {
                    return index == 0;
                }

                if let E4XNodeKind::Element {
                    children,
                    attributes,
                } = &*read.node.kind()
                {
                    let search_children = if name.is_attribute() {
                        attributes
                    } else {
                        children
                    };

                    return search_children.iter().any(|child| child.matches_name(name));
                }
            }
        }
        read.base.has_own_dynamic_property(name)
    }

    fn has_own_property_string(
        self,
        name: impl Into<AvmString<'gc>>,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<bool, Error<'gc>> {
        let name = name_to_multiname(activation, &Value::String(name.into()), false)?;
        Ok(self.has_own_property(&name))
    }

    // ECMA-357 9.1.1.2 [[Put]] (P, V)
    fn set_property_local(
        self,
        name: &Multiname<'gc>,
        value: Value<'gc>,
        activation: &mut Activation<'_, 'gc>,
    ) -> Result<(), Error<'gc>> {
        // 1. If ToString(ToUint32(P)) == P, throw a TypeError exception
        if let Some(local_name) = name.local_name() {
            if local_name.parse::<usize>().is_ok() {
                return Err(make_error_1087(activation));
            }
        }

        // 2. If x.[[Class]] ∈ {"text", "comment", "processing-instruction", "attribute"}, return
        if !matches!(*self.node().kind(), E4XNodeKind::Element { .. }) {
            return Ok(());
        }

        // 4. Else
        // 4.a. Let c be the result of calling the [[DeepCopy]] method of V
        let value = if let Some(xml) = value.as_object().and_then(|x| x.as_xml_object()) {
            xml.deep_copy(activation).into()
        } else if let Some(list) = value.as_object().and_then(|x| x.as_xml_list_object()) {
            list.deep_copy(activation).into()
        // 3. If (Type(V) ∉ {XML, XMLList}) or (V.[[Class]] ∈ {"text", "attribute"})
        // 3.a. Let c = ToString(V)
        } else {
            value
        };

        // 5. Let n = ToXMLName(P)
        // 6. If Type(n) is AttributeName
        if name.is_attribute() {
            // 6.b. If Type(c) is XMLList
            let value = if let Some(list) = value.as_object().and_then(|x| x.as_xml_list_object()) {
                let mut out = WString::new();

                // 6.b.i. If c.[[Length]] == 0, let c be the empty string, NOTE: String is already empty, no case needed.
                // 6.b.ii. Else
                if list.length() != 0 {
                    // 6.b.ii.1. Let s = ToString(c[0])
                    out.push_str(
                        list.children()[0]
                            .node()
                            .xml_to_string(activation)
                            .as_wstr(),
                    );

                    // 6.b.ii.2. For i = 1 to c.[[Length]]-1
                    for child in list.children().iter().skip(1) {
                        // 6.b.ii.2.a. Let s be the result of concatenating s, the string " " (space) and ToString(c[i])
                        out.push_char(' ');
                        out.push_str(child.node().xml_to_string(activation).as_wstr())
                    }
                }

                AvmString::new(activation.gc(), out)
            // 6.c. Else
            } else {
                value.coerce_to_string(activation)?
            };

            let mc = activation.context.gc_context;
            self.delete_property_local(activation, name)?;
            let Some(local_name) = name.local_name() else {
                return Err(format!("Cannot set attribute {:?} without a local name", name).into());
            };
            let new_attr = E4XNode::attribute(mc, local_name, value, Some(*self.node()));

            let write = self.0.write(mc);
            let mut kind = write.node.kind_mut(mc);
            let E4XNodeKind::Element { attributes, .. } = &mut *kind else {
                return Ok(());
            };

            attributes.push(new_attr);
            return Ok(());
        }

        // 7. Let isValidName be the result of calling the function isXMLName (section 13.1.2.1) with argument n
        let is_valid_name = name
            .local_name()
            .map(crate::avm2::e4x::is_xml_name)
            .unwrap_or(false);
        // 8. If isValidName is false and n.localName is not equal to the string "*", return
        if !is_valid_name && !name.is_any_name() {
            return Ok(());
        }

        // 10. Let primitiveAssign = (Type(c) ∉ {XML, XMLList}) and (n.localName is not equal to the string "*")
        let primitive_assign = !value.as_object().map_or(false, |x| {
            x.as_xml_list_object().is_some() || x.as_xml_object().is_some()
        }) && !name.is_any_name();

        let self_node = self.node();

        // 9. Let i = undefined
        // 11.
        let index = self_node.remove_matching_children(activation.gc(), name);

        let index = if let Some((index, node)) = index {
            self_node.insert_at(activation.gc(), index, node);
            index
        // 12. If i == undefined
        } else {
            // 12.a. Let i = x.[[Length]]
            let index = self_node.length().expect("Node should be of element kind");
            self_node.insert_at(activation.gc(), index, E4XNode::dummy(activation.gc()));

            // 12.b. If (primitiveAssign == true)
            if primitive_assign {
                // 12.b.i. If (n.uri == null)
                // 12.b.i.1. Let name be a new QName created as if by calling the constructor new
                //           QName(GetDefaultNamespace(), n)
                // 12.b.ii. Else
                // 12.b.ii.1. Let name be a new QName created as if by calling the constructor new QName(n)

                // 12.b.iii. Create a new XML object y with y.[[Name]] = name, y.[[Class]] = "element" and y.[[Parent]] = x
                let node = E4XNode::element(
                    activation.gc(),
                    name.explict_namespace(),
                    name.local_name().unwrap(),
                    Some(*self_node),
                );
                // 12.b.v. Call the [[Replace]] method of x with arguments ToString(i) and y
                self_node.replace(index, XmlObject::new(node, activation).into(), activation)?;
                // FIXME: 12.b.iv. Let ns be the result of calling [[GetNamespace]] on name with no arguments
                // 12.b.vi. Call [[AddInScopeNamespace]] on y with argument ns
            }

            index
        };

        // 13. If (primitiveAssign == true)
        if primitive_assign {
            let E4XNodeKind::Element { children, .. } = &mut *self_node.kind_mut(activation.gc())
            else {
                unreachable!("Node should be of Element kind");
            };

            // 13.a. Delete all the properties of the XML object x[i]
            children[index].remove_all_children(activation.gc());

            // 13.b. Let s = ToString(c)
            let val = value.coerce_to_string(activation)?;

            // 13.c. If s is not the empty string, call the [[Replace]] method of x[i] with arguments "0" and s
            if !val.is_empty() {
                children[index].replace(0, value, activation)?;
            }
        // 14. Else
        } else {
            // 14.a. Call the [[Replace]] method of x with arguments ToString(i) and c
            self_node.replace(index, value, activation)?;
        }

        // 15. Return
        Ok(())
    }

    fn delete_property_local(
        self,
        activation: &mut Activation<'_, 'gc>,
        name: &Multiname<'gc>,
    ) -> Result<bool, Error<'gc>> {
        if name.has_explicit_namespace() {
            return Err(format!(
                "Can not set property {:?} with an explicit namespace yet",
                name
            )
            .into());
        }

        let mc = activation.context.gc_context;
        let write = self.0.write(mc);
        let mut kind = write.node.kind_mut(mc);
        let E4XNodeKind::Element {
            children,
            attributes,
            ..
        } = &mut *kind
        else {
            return Ok(false);
        };

        let retain_non_matching = |node: &E4XNode<'gc>| {
            if node.matches_name(name) {
                node.set_parent(None, mc);
                false
            } else {
                true
            }
        };

        if name.is_attribute() {
            attributes.retain(retain_non_matching);
        } else {
            children.retain(retain_non_matching);
        }
        Ok(true)
    }
}
