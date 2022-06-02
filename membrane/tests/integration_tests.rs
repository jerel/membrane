mod test_utils;

mod test {
  use super::test_utils::*;
  use example;
  use membrane::Membrane;
  use serial_test::serial;
  use std::{fs::read_to_string, path::Path};

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

    let api = read_to_string(path.join("lib").join("accounts.dart")).unwrap();
    assert!(api.contains("@immutable\nclass AccountsApi {"));
    assert!(api.contains("Future<Contact> contact({required String userId}) async {"));

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
      "int32_t *membrane_accounts_contact(int64_t port, const char *user_id);",
    );

    build_lib(&path.to_path_buf());
    run_dart(&path.to_path_buf(), vec!["pub", "add", "test"], false);
    run_dart(
      &path.to_path_buf(),
      vec!["test", "test/main_test.dart"],
      true,
    );

    #[cfg(feature = "test-c-example")]
    run_dart(
      &path.to_path_buf(),
      vec!["test", "test/async_c_test.dart"],
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

    build_lib(&path.to_path_buf());
    run_dart(&path.to_path_buf(), vec!["pub", "add", "test"], false);
    run_dart(
      &path.to_path_buf(),
      vec!["test", "test/enum_test.dart"],
      true,
    );
  }
}
