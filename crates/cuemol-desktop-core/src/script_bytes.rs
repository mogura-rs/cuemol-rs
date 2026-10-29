use boa_engine::{Finalize, JsData, JsNativeError, JsResult, JsValue, Trace};
use cuemol_core::ByteArray;
use std::cell::RefCell;

#[derive(Default, Trace, Finalize, JsData)]
pub(crate) struct Storage {
    #[unsafe_ignore_trace]
    bytes: RefCell<ByteArray>,
}

fn error(message: impl Into<String>) -> boa_engine::JsError {
    JsNativeError::typ().with_message(message.into()).into()
}

pub(crate) fn read<T>(
    value: &JsValue,
    operation: impl FnOnce(&ByteArray) -> cuemol_core::Result<T>,
) -> JsResult<T> {
    let object = value
        .as_object()
        .ok_or_else(|| error("ByteArray receiver is not an object"))?;
    let storage = object
        .downcast_ref::<Storage>()
        .ok_or_else(|| error("ByteArray receiver has no byte storage"))?;
    let bytes = storage.bytes.borrow();
    operation(&bytes).map_err(|value| error(value.to_string()))
}

pub(crate) fn write<T>(
    value: &JsValue,
    operation: impl FnOnce(&mut ByteArray) -> cuemol_core::Result<T>,
) -> JsResult<T> {
    let object = value
        .as_object()
        .ok_or_else(|| error("ByteArray receiver is not an object"))?;
    let storage = object
        .downcast_ref::<Storage>()
        .ok_or_else(|| error("ByteArray receiver has no byte storage"))?;
    let mut bytes = storage.bytes.borrow_mut();
    operation(&mut bytes).map_err(|value| error(value.to_string()))
}

pub(crate) fn replace(value: &JsValue, bytes: ByteArray) -> JsResult<()> {
    write(value, |storage| {
        *storage = bytes;
        Ok(())
    })
}
