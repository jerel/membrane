use crate::{DeferredEnumTrace, DeferredTrace};

pub fn enums() -> Vec<&'static DeferredEnumTrace> {
  inventory::iter::<DeferredEnumTrace>().collect()
}

pub fn functions() -> Vec<&'static DeferredTrace> {
  inventory::iter::<DeferredTrace>().collect()
}
