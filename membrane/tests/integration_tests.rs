mod test_utils;

mod test {
  use super::test_utils::*;
  use example;
  use membrane::Membrane;
  use std::fs::read_to_string;
  use tempfile::tempdir;

  #[test]
  fn base_project() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test_project");

    // reference the example lib so it doesn't get optimized away
    let _ = example::load();

    Membrane::new()
      .package_destination_dir(path.to_str().unwrap())
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
      "int32_t membrane_accounts_contact(int64_t port, const char *user_id);",
    );

    build_lib(&path);
    write_dart_tests(&path, "main_test.dart", &DART_TESTS);
    run_dart(&path, vec!["pub", "add", "--dev", "test"], false);
    run_dart(&path, vec!["test"], true);
  }

  static DART_TESTS: &str = r#"
import 'package:test/test.dart';
import 'package:test_project/accounts.dart';

void main() {
  test('can get a contact from Rust by String arg', () async {
    final accounts = AccountsApi();
    expect(
        await accounts.contact(userId: "1"),
        equals(
            Contact(id: 1, fullName: "Alice Smith", status: Status.pending)));
  });

  test(
      'can call a function with optional args with none of the args or all of the args',
      () async {
    final accounts = AccountsApi();
    expect(await accounts.optionsDemo(), equals(OptionsDemo()));

    final low = OptionsDemo(
        one: "",
        two: -9223372036854775807,
        three: double.minPositive,
        four: false);
    expect(
        await accounts.optionsDemo(
            one: "",
            two: -9223372036854775807,
            three: double.minPositive,
            four: false),
        equals(low));

    final high = OptionsDemo(
        one: "a string",
        two: 9223372036854775807,
        three: double.maxFinite,
        four: true,
        five: Arg(value: 20));
    expect(
        await accounts.optionsDemo(
            one: "a string",
            two: 9223372036854775807,
            three: double.maxFinite,
            four: true,
            five: Arg(value: 20)),
        equals(high));
  });

  test(
      'can call a function that returns a scalar value',
      () async {
    final accounts = AccountsApi();
    expect(await accounts.scalarI64(val: 10), equals(10));
    expect(await accounts.scalarF64(val: 11.1), equals(11.1));
    expect(await accounts.scalarString(val: "hello world"), equals("hello world"));
    expect(await accounts.scalarBool(val: true), equals(true));
  });

  test(
      'test that a function throws an ApiError instance when an error is returned',
      () async {
    final accounts = AccountsApi();
    expect(() async => await accounts.scalarError(), throwsA(isA<AccountsApiError>()));
  });

  test(
      'test that a function throws a string when an error is returned',
      () async {
    final accounts = AccountsApi();
    try {
      await accounts.scalarError();
    } on AccountsApiError catch(err) {
      expect(err.e, "an error message");
    }
  });

}
"#;
}
