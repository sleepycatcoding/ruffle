use crate::avm2::error::invalid_socket_error;
pub use crate::avm2::object::xml_socket_allocator;
use crate::avm2::parameters::ParametersExt;
use crate::avm2::{Activation, Error, Object, TObject, Value};
use crate::avm2_stub_method;
use crate::context::UpdateContext;

pub fn get_connected<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let xml_socket = match this.as_xml_socket() {
        Some(xml_socket) => xml_socket,
        None => return Ok(Value::Undefined),
    };

    let UpdateContext { sockets, .. } = &mut activation.context;

    let handle = match xml_socket.handle() {
        Some(handle) => handle,
        None => return Ok(Value::Bool(false)),
    };

    Ok(Value::Bool(sockets.is_connected(handle)))
}

pub fn get_timeout<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    if let Some(xml_socket) = this.as_xml_socket() {
        return Ok(xml_socket.timeout().into());
    }

    Ok(Value::Undefined)
}

pub fn set_timeout<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    if let Some(xml_socket) = this.as_xml_socket() {
        let new_timeout = args.get_u32(activation, 0)?;
        xml_socket.set_timeout(new_timeout);
    }

    Ok(Value::Undefined)
}

pub fn connect<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm2_stub_method!(activation, "flash.net.XMLSocket", "connect");
    Ok(Value::Undefined)
}

pub fn close<'gc>(
    activation: &mut Activation<'_, 'gc>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    if let Some(xml_socket) = this.as_xml_socket() {
        // We throw an IOError when socket is not open.
        let handle = xml_socket
            .handle()
            .ok_or(invalid_socket_error(activation))?;

        if !activation.context.sockets.is_connected(handle) {
            return Err(invalid_socket_error(activation));
        }

        let UpdateContext { sockets, .. } = &mut activation.context;

        sockets.close(handle)
    }

    Ok(Value::Undefined)
}

pub fn send<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm2_stub_method!(activation, "flash.net.XMLSocket", "send");
    Ok(Value::Undefined)
}
