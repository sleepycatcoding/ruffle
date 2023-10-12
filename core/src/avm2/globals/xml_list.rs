//! XMLList builtin and prototype

use ruffle_wstr::WString;

pub use crate::avm2::object::xml_list_allocator;
use crate::avm2::{
    e4x::{name_to_multiname, simple_content_to_string, E4XNode, E4XNodeKind},
    error::type_error,
    multiname::Multiname,
    object::{E4XOrXml, XmlListObject, XmlObject},
    parameters::ParametersExt,
    string::AvmString,
    Activation, Error, Object, TObject, Value,
};

fn has_complex_content_inner(children: &[E4XOrXml<'_>]) -> bool {
    match children {
        [] => false,
        [child] => child.node().has_complex_content(),
        _ => children
            .iter()
            .any(|child| matches!(&*child.node().kind(), E4XNodeKind::Element { .. })),
    }
}

fn has_simple_content_inner(children: &[E4XOrXml<'_>]) -> bool {
    match children {
        [] => true,
        [child] => child.node().has_simple_content(),
        _ => children
            .iter()
            .all(|child| !matches!(&*child.node().kind(), E4XNodeKind::Element { .. })),
    }
}

pub fn init<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let this = this.as_xml_list_object().unwrap();
    let value = args[0];
    let ignore_comments = args.get_bool(1);
    let ignore_processing_instructions = args.get_bool(2);
    let ignore_whitespace = args.get_bool(3);

    if let Some(obj) = value.as_object() {
        if let Some(xml) = obj.as_xml_object() {
            // Note - we re-use the XML object that was passed in, which makes
            // `this[0] === xmlObjArg` true.
            // This logic does *not* go in `E4XNode::parse`, as it does not apply
            // to the `XML` constructor: `new XML(xmlObj) === xmlObj` is false.
            this.set_children(activation.context.gc_context, vec![E4XOrXml::Xml(xml)]);
            return Ok(Value::Undefined);
        }
    }

    match E4XNode::parse(
        value,
        activation,
        ignore_comments,
        ignore_processing_instructions,
        ignore_whitespace,
    ) {
        Ok(nodes) => {
            this.set_children(
                activation.context.gc_context,
                nodes.into_iter().map(E4XOrXml::E4X).collect(),
            );
        }
        Err(e) => {
            return Err(Error::RustError(
                format!("Failed to parse XML: {e:?}").into(),
            ))
        }
    }

    Ok(Value::Undefined)
}

pub fn call_handler<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    if args.len() > 0 {
        // We do *not* create a new object when AS does 'XMLList(someXMLList)'
        if let Some(obj) = args.try_get_object(activation, 0) {
            if let Some(xml_list) = obj.as_xml_list_object() {
                return Ok(xml_list.into());
            }
        }
    }

    Ok(activation
        .avm2()
        .classes()
        .xml_list
        .construct(activation, args)?
        .into())
}

pub fn has_complex_content<'gc>(
    _activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();
    let children = list.children();
    Ok(has_complex_content_inner(&children).into())
}

pub fn has_simple_content<'gc>(
    _activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();
    let children = list.children();
    Ok(has_simple_content_inner(&children).into())
}

pub fn to_string<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();
    let children = list.children();
    if has_simple_content_inner(&children) {
        Ok(simple_content_to_string(children.iter().cloned(), activation).into())
    } else {
        to_xml_string(activation, this, args)
    }
}

pub fn to_xml_string<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();
    let children = list.children();
    let mut out = WString::new();
    for (i, child) in children.iter().enumerate() {
        if i != 0 {
            out.push_char('\n');
        }
        out.push_str(child.node().xml_to_xml_string(activation).as_wstr())
    }
    Ok(AvmString::new(activation.context.gc_context, out).into())
}

pub fn length<'gc>(
    _activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();
    let children = list.children();
    Ok(children.len().into())
}

pub fn child<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();
    let multiname = name_to_multiname(activation, &args[0], false)?;
    let children = list.children();
    let mut sub_children = Vec::new();
    for child in &*children {
        if let E4XNodeKind::Element { ref children, .. } = &*child.node().kind() {
            sub_children.extend(
                children
                    .iter()
                    .filter(|node| node.matches_name(&multiname))
                    .map(|node| E4XOrXml::E4X(*node)),
            );
        }
    }
    Ok(XmlListObject::new(activation, sub_children, Some(list.into()), None).into())
}

pub fn children<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();
    let children = list.children();
    let mut sub_children = Vec::new();
    for child in &*children {
        if let E4XNodeKind::Element { ref children, .. } = &*child.node().kind() {
            sub_children.extend(children.iter().map(|node| E4XOrXml::E4X(*node)));
        }
    }
    // FIXME: This method should just call get_property_local with "*".
    Ok(XmlListObject::new(
        activation,
        sub_children,
        Some(list.into()),
        Some(Multiname::any(activation.gc())),
    )
    .into())
}

// ECMA-357 13.5.4.8 XMLList.prototype.contains ( value )
pub fn contains<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();
    let xml = args
        .get_object(activation, 0, "value")?
        .as_xml_object()
        .unwrap();

    // 1. For i = 0 to list.[[Length]]-1
    for child in &*list.children() {
        // 1.a. If the result of the comparison list[i] == value is true, return true
        if child.node().equals(&xml.node()) {
            return Ok(true.into());
        }
    }

    // 2. Return false
    Ok(false.into())
}

pub fn copy<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();
    Ok(list.deep_copy(activation).into())
}

pub fn attribute<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();

    let name = args[0];
    let multiname = name_to_multiname(activation, &name, true)?;

    let children = list.children();
    let mut sub_children = Vec::new();
    for child in &*children {
        if let E4XNodeKind::Element { ref attributes, .. } = &*child.node().kind() {
            if let Some(found) = attributes
                .iter()
                .find(|node| node.matches_name(&multiname))
                .copied()
            {
                sub_children.push(E4XOrXml::E4X(found));
            }
        }
    }

    // FIXME: This should just use get_property_local with an attribute Multiname.
    Ok(XmlListObject::new(activation, sub_children, Some(list.into()), Some(multiname)).into())
}

pub fn attributes<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();

    let mut child_attrs = Vec::new();
    for child in list.children().iter() {
        if let E4XNodeKind::Element { ref attributes, .. } = &*child.node().kind() {
            child_attrs.extend(attributes.iter().map(|node| E4XOrXml::E4X(*node)));
        }
    }

    // FIXME: This should just use get_property_local with an any attribute Multiname.
    Ok(XmlListObject::new(
        activation,
        child_attrs,
        Some(list.into()),
        Some(Multiname::any_attribute(activation.gc())),
    )
    .into())
}

pub fn name<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();

    let mut children = list.children_mut(activation.context.gc_context);
    match &mut children[..] {
        [child] => {
            child
                .get_or_create_xml(activation)
                .call_public_property("name", &[], activation)
        }
        _ => Err(Error::AvmError(type_error(
            activation,
            "Error #1086: The name method only works on lists containing one item.",
            1086,
        )?)),
    }
}

pub fn descendants<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let multiname = name_to_multiname(activation, &args[0], false)?;
    if let Some(descendants) = this.xml_descendants(activation, &multiname) {
        Ok(descendants.into())
    } else {
        Ok(Value::Undefined)
    }
}

pub fn text<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let xml_list = this.as_xml_list_object().unwrap();
    let mut nodes = Vec::new();
    for child in xml_list.children().iter() {
        if let E4XNodeKind::Element { ref children, .. } = &*child.node().kind() {
            nodes.extend(
                children
                    .iter()
                    .filter(|node| matches!(&*node.kind(), E4XNodeKind::Text(_)))
                    .map(|node| E4XOrXml::E4X(*node)),
            );
        }
    }
    Ok(XmlListObject::new(activation, nodes, Some(xml_list.into()), None).into())
}

pub fn comments<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let xml_list = this.as_xml_list_object().unwrap();
    let mut nodes = Vec::new();
    for child in xml_list.children().iter() {
        if let E4XNodeKind::Element { ref children, .. } = &*child.node().kind() {
            nodes.extend(
                children
                    .iter()
                    .filter(|node| matches!(&*node.kind(), E4XNodeKind::Comment(_)))
                    .map(|node| E4XOrXml::E4X(*node)),
            );
        }
    }
    Ok(XmlListObject::new(activation, nodes, Some(xml_list.into()), None).into())
}

// ECMA-357 13.5.4.17 XMLList.prototype.parent ( )
pub fn parent<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let list = this.as_xml_list_object().unwrap();

    // 1. If list.[[Length]] = 0, return undefined
    if list.length() == 0 {
        return Ok(Value::Undefined);
    }

    // 2. Let parent = list[0].[[Parent]]
    let parent = list.children()[0].node().parent();

    // 3. For i = 1 to list.[[Length]]-1, if list[i].[[Parent]] is not equal to parent, return undefined
    for child in list.children().iter().skip(1) {
        let other = child.node().parent();

        match (parent, other) {
            (Some(v1), Some(v2)) if !E4XNode::ptr_eq(v1, v2) => {
                return Ok(Value::Undefined);
            }
            (None, Some(_)) => return Ok(Value::Undefined),
            (Some(_), None) => return Ok(Value::Undefined),
            _ => {}
        }
    }

    // 4. Return parent
    if let Some(parent) = parent {
        Ok(XmlObject::new(parent, activation).into())
    } else {
        Ok(Value::Undefined)
    }
}

pub fn processing_instructions<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let xml_list = this.as_xml_list_object().unwrap();
    let multiname = name_to_multiname(activation, &args[0], false)?;
    let mut nodes = Vec::new();
    for child in xml_list.children().iter() {
        if let E4XNodeKind::Element { ref children, .. } = &*child.node().kind() {
            nodes.extend(
                children
                    .iter()
                    .filter(|node| {
                        matches!(&*node.kind(), E4XNodeKind::ProcessingInstruction(_))
                            && node.matches_name(&multiname)
                    })
                    .map(|node| E4XOrXml::E4X(*node)),
            );
        }
    }

    Ok(XmlListObject::new(activation, nodes, Some(xml_list.into()), None).into())
}
