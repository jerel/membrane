use crate::SourceCodeLocation;
use allo_isolate::Isolate;
use serde::ser::Serialize;

pub fn send<T: Serialize, E: Serialize>(isolate: Isolate, result: Result<T, E>) -> bool {
  match result {
    Ok(value) => {
      if let Ok(buffer) = crate::bincode::serialize(&(crate::MembraneMsgKind::Ok as u8, value)) {
        isolate.post(crate::allo_isolate::ZeroCopyBuffer(buffer))
      } else {
        false
      }
    }
    Err(err) => {
      if let Ok(buffer) = crate::bincode::serialize(&(crate::MembraneMsgKind::Error as u8, err)) {
        isolate.post(crate::allo_isolate::ZeroCopyBuffer(buffer))
      } else {
        false
      }
    }
  }
}

pub(crate) fn display_code_location(location: Option<&Vec<SourceCodeLocation>>) -> String {
  match location {
    Some(loc) if !loc.is_empty() => {
      let last = loc.len() - 1;
      format!(
        " at {}",
        loc
          .iter()
          .enumerate()
          .map(|(index, path)| {
            if last == 0 {
              // single item
              path.to_string()
            } else if index == 0 && last == 1 {
              // on the first of two
              format!("{} and", path)
            } else if index == 1 && last == 1 {
              // on the second of two
              path.to_string()
            } else if index == (last - 1) {
              // on the next to last of many
              format!("{}, and", path)
            } else if index == last {
              // on the last of many
              path.to_string()
            } else {
              format!("{},", path)
            }
          })
          .collect::<Vec<String>>()
          .join(" ")
      )
    }
    _ => String::new(),
  }
}

pub(crate) fn new_style_export<S: AsRef<str>>(namespace: S, config: &crate::DartConfig) -> bool {
  !config.v1_import_style.contains(&namespace.as_ref())
}

#[cfg(test)]
mod tests {
  use super::display_code_location;

  #[test]
  fn test_source_code_display_location() {
    assert_eq!(display_code_location(Some(&vec![])), "");

    assert_eq!(
      display_code_location(Some(&vec!["app.rs:30"])),
      " at app.rs:30"
    );

    assert_eq!(
      display_code_location(Some(&vec!["app.rs:30", "foo.rs:10"])),
      " at app.rs:30 and foo.rs:10"
    );

    assert_eq!(
      display_code_location(Some(&vec!["app.rs:30", "foo.rs:10", "bar.rs:5"])),
      " at app.rs:30, foo.rs:10, and bar.rs:5"
    );
  }
}
