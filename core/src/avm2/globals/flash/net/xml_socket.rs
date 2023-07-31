pub use crate::avm2::object::xml_socket_allocator;
use crate::avm2::{Activation, Error, Object, Value};
use crate::{avm2_stub_getter, avm2_stub_method, avm2_stub_setter};

pub fn get_connected<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm2_stub_getter!(activation, "flash.net.XMLSocket", "connected");
    Ok(Value::Bool(false))
}

pub fn get_timeout<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm2_stub_getter!(activation, "flash.net.XMLSocket", "timeout");
    Ok(Value::Undefined)
}

pub fn set_timeout<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm2_stub_setter!(activation, "flash.net.XMLSocket", "timeout");
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
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm2_stub_method!(activation, "flash.net.XMLSocket", "close");
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
