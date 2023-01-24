mod test_utils;

mod test {
  use super::test_utils::*;
  use example;
  use membrane::Membrane;
  use serial_test::serial;
  use std::{fs::read_to_string, path::Path};

  #[test]
  #[serial]
  #[cfg(feature = "c-example")]
  fn test_c_project() {
    let path = Path::new("../dart_example");

    // reference the example lib so it doesn't get optimized away
    let _ = example::load();

    Membrane::new()
      .timeout(200)
      .package_destination_dir(path)
      .using_lib("libexample")
      .create_pub_package()
      .write_api()
      .write_c_headers()
      .write_bindings();

    build_lib(&path.to_path_buf(), &mut vec!["--features", "c-example"]);
    run_dart(&path.to_path_buf(), vec!["pub", "add", "test"], false);

    run_dart(
      &path.to_path_buf(),
      vec![
        "test",
        "test/threading_c_test.dart",
        "test/render_c_test.dart",
      ],
      true,
    );
  }

  #[test]
  #[serial]
  fn base_project() {
    let path = Path::new("../dart_example");

    // reference the example lib so it doesn't get optimized away
    let _ = example::load();

    Membrane::new()
      .timeout(200)
      .package_destination_dir(path)
      .using_lib("libexample")
      .create_pub_package()
      .write_api()
      .write_c_headers()
      .write_bindings();

    let api = read_to_string(path.join("lib/src").join("accounts_ffi.dart")).unwrap();
    assert!(api.contains("@immutable\nclass AccountsApi {"));
    assert!(api.contains("Future<Contact> contact({required String userId}) async {"));

    let web_api = read_to_string(path.join("lib/src").join("accounts_web.dart")).unwrap();
    assert!(web_api.contains("@immutable\nclass AccountsApi {"));
    assert!(web_api.contains("Future<Contact> contact({required String userId}) async {"));

    let dart_type = read_to_string(
      path
        .join("lib")
        .join("src")
        .join("accounts")
        .join("contact.dart"),
    )
    .unwrap();

    assert_contains_part(
      &dart_type,
      r#"
@immutable
class Contact {
  const Contact({
    required this.id,
    required this.fullName,
    required this.status,
  });"#,
    );

    let headers = read_to_string(
      path
        .join("lib")
        .join("src")
        .join("accounts")
        .join("accounts.h"),
    )
    .unwrap();

    assert_contains_part(
      &headers,
      "MembraneResponse membrane_accounts_contact(int64_t port, const char *user_id);",
    );

    // verify that borrowed types are no longer created in the borrowing namespace
    assert!(path.join("lib/src/locations/location.dart").exists() == true);
    assert!(path.join("lib/src/accounts/contact.dart").exists() == true);
    assert!(path.join("lib/src/accounts/status.dart").exists() == true);
    assert!(path.join("lib/src/accounts/filter.dart").exists() == true);
    assert!(path.join("lib/src/accounts/match.dart").exists() == true);
    assert!(path.join("lib/src/common/shared_type.dart").exists() == true);
    assert!(path.join("lib/src/orgs/location.dart").exists() == false);
    assert!(path.join("lib/src/orgs/contact.dart").exists() == false);
    assert!(path.join("lib/src/orgs/status.dart").exists() == false);
    assert!(path.join("lib/src/orgs/filter.dart").exists() == false);
    assert!(path.join("lib/src/orgs/match.dart").exists() == false);
    assert!(path.join("lib/src/orgs/shared_type.dart").exists() == false);

    let imports =
      read_to_string(path.join("lib").join("src").join("orgs").join("orgs.dart")).unwrap();

    assert_contains_part(
      &imports,
      "import '../accounts/accounts.dart' show Contact, Filter, Match, Reports, Status, StatusExtension;
import '../common/common.dart' show Arg, Error, Mixed, SharedType, VecWrapper;
import '../locations/locations.dart' show Location;
",
    );

    build_lib(&path.to_path_buf(), &mut vec![]);
    run_dart(&path.to_path_buf(), vec!["pub", "add", "test"], false);
    run_dart(
      &path.to_path_buf(),
      vec!["test", "test/main_test.dart"],
      true,
    );
  }

  #[test]
  #[serial]
  fn test_class_enums() {
    let path = Path::new("../dart_example");

    // reference the example lib so it doesn't get optimized away
    let _ = example::load();

    Membrane::new()
      .with_c_style_enums(false)
      .package_destination_dir(path)
      .using_lib("libexample")
      .create_pub_package()
      .write_api()
      .write_c_headers()
      .write_bindings();

    build_lib(&path.to_path_buf(), &mut vec![]);
    run_dart(&path.to_path_buf(), vec!["pub", "add", "test"], false);
    run_dart(
      &path.to_path_buf(),
      vec!["test", "test/enum_test.dart"],
      true,
    );
  }

  #[test]
  #[serial]
  fn base_project_loading_cdylib() {
    let path = Path::new("../dart_example");

    build_lib(&path.to_path_buf(), &mut vec![]);

    Membrane::new_from_cdylib(&path.join("libexample.so"))
      .timeout(200)
      .package_destination_dir(path)
      .using_lib("libexample")
      .create_pub_package()
      .write_api()
      .write_c_headers()
      .write_bindings();

    run_dart(&path.to_path_buf(), vec!["pub", "add", "test"], false);
    run_dart(
      &path.to_path_buf(),
      vec!["test", "test/main_test.dart"],
      true,
    );
  }
}
