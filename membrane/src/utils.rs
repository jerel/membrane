use allo_isolate::Isolate;
use serde::ser::Serialize;

pub fn send<T: Serialize, E: Serialize>(isolate: Isolate, result: Result<T, E>) -> bool {
  match result {
    Ok(value) => {
      if let Ok(buffer) = crate::bincode::serialize(&(crate::MembraneResultType::Data as u8, value))
      {
        isolate.post(crate::allo_isolate::ZeroCopyBuffer(buffer))
      } else {
        false
      }
    }
    Err(err) => {
      if let Ok(buffer) = crate::bincode::serialize(&(crate::MembraneResultType::Error as u8, err))
      {
        isolate.post(crate::allo_isolate::ZeroCopyBuffer(buffer))
      } else {
        false
      }
    }
  }
}
