mod mock;
use crate::mock::RUNTIME;

mod test {
  use membrane::Membrane;
  use pretty_assertions::assert_eq;
  use std::path::Path;

  #[test]
  fn test_self_borrow_errors() {
    mod app {
      use membrane::async_dart;

      mod data {
        #[derive(serde::Deserialize, serde::Serialize)]
        pub struct Location(pub String);
      }

      #[async_dart(
        namespace = "a",
        // self reference
        borrow = "a::Location",
      )]
      pub async fn borrow_one() -> Result<data::Location, String> {
        todo!()
      }
    }

    // gather metadata types and ensure that the above self reference is caught
    let mut membrane = Membrane::new();

    membrane
      .package_destination_dir(Path::new("../dart_example"))
      .create_pub_package()
      .write_api();

    assert_eq!(
      membrane.drain_errors(),
      vec!["`a::Location` at membrane/tests/codegen_self_borrow_test.rs:19 was borrowed by `a` which is a self reference"]
    );
  }
}
