mod mock;
use crate::mock::RUNTIME;

mod test {
  use membrane::Membrane;
  use pretty_assertions::assert_eq;
  use std::path::Path;

  #[test]
  fn test_borrow_errors() {
    mod app {
      use membrane::async_dart;

      mod data {
        #[derive(serde::Deserialize, serde::Serialize)]
        pub struct Location(pub String);
      }

      #[async_dart(
        namespace = "a",
        // circular reference
        borrow = "b::Location",
      )]
      pub async fn reborrow_one() -> Result<data::Location, String> {
        todo!()
      }

      #[async_dart(
        namespace = "b",
        // circular reference
        borrow = "a::Location",
      )]
      pub async fn reborrow_two() -> Result<data::Location, String> {
        todo!()
      }
    }

    // gather metadata types and ensure that the above circular dependency is caught
    let mut membrane = Membrane::new();

    membrane
      .package_destination_dir(Path::new("../dart_example"))
      .create_pub_package()
      .write_api();

    assert_eq!(
      membrane.drain_errors(),
      vec!["The following `borrows` were found which attempt to reborrow a type which is not owned by the target namespace: `a::Location, b::Location`"]
    );
  }
}
