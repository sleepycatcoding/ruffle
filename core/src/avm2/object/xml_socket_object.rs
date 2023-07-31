use crate::avm2::object::script_object::ScriptObjectData;
use crate::avm2::object::{ClassObject, Object, ObjectPtr, TObject};
use crate::avm2::value::Value;
use crate::avm2::{Activation, Error};
use crate::socket::SocketHandle;
use gc_arena::barrier::unlock;
use gc_arena::lock::RefLock;
use gc_arena::{Collect, Gc, GcWeak, Mutation};
use std::cell::{Cell, Ref, RefMut};
use std::fmt;

pub fn xml_socket_allocator<'gc>(
    class: ClassObject<'gc>,
    activation: &mut Activation<'_, 'gc>,
) -> Result<Object<'gc>, Error<'gc>> {
    let base = ScriptObjectData::new(class).into();

    Ok(XmlSocketObject(Gc::new(
        activation.context.gc(),
        XmlSocketObjectData {
            base,
            handle: Cell::new(None),
            timeout: Cell::new(0),
        },
    ))
    .into())
}

#[derive(Clone, Collect, Copy)]
#[collect(no_drop)]
pub struct XmlSocketObject<'gc>(pub Gc<'gc, XmlSocketObjectData<'gc>>);

#[derive(Clone, Collect, Copy, Debug)]
#[collect(no_drop)]
pub struct XmlSocketObjectWeak<'gc>(pub GcWeak<'gc, XmlSocketObjectData<'gc>>);

impl<'gc> TObject<'gc> for XmlSocketObject<'gc> {
    fn base(&self) -> Ref<ScriptObjectData<'gc>> {
        self.0.base.borrow()
    }

    fn base_mut(&self, mc: &Mutation<'gc>) -> RefMut<ScriptObjectData<'gc>> {
        unlock!(Gc::write(mc, self.0), XmlSocketObjectData, base).borrow_mut()
    }

    fn as_ptr(&self) -> *const ObjectPtr {
        Gc::as_ptr(self.0) as *const ObjectPtr
    }

    fn value_of(&self, _mc: &Mutation<'gc>) -> Result<Value<'gc>, Error<'gc>> {
        Ok(Value::Object(Object::from(*self)))
    }

    fn as_xml_socket(&self) -> Option<XmlSocketObject<'gc>> {
        Some(*self)
    }
}

impl<'gc> XmlSocketObject<'gc> {
    pub fn timeout(&self) -> u32 {
        self.0.timeout.get()
    }

    pub fn set_timeout(&self, timeout: u32) {
        // NOTE: When a timeout of smaller than 250 milliseconds is provided,
        //       we clamp it to 250 milliseconds.
        self.0.timeout.set(std::cmp::max(250, timeout));
    }

    pub fn handle(&self) -> Option<SocketHandle> {
        self.0.handle.get()
    }

    pub fn set_handle(&self, handle: SocketHandle) -> Option<SocketHandle> {
        self.0.handle.replace(Some(handle))
    }
}

#[derive(Collect)]
#[collect(no_drop)]
pub struct XmlSocketObjectData<'gc> {
    /// Base script object
    base: RefLock<ScriptObjectData<'gc>>,

    handle: Cell<Option<SocketHandle>>,

    /// XmlSocket connection timeout in milliseconds.
    timeout: Cell<u32>,
}

impl fmt::Debug for XmlSocketObject<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "XmlSocketObject")
    }
}
