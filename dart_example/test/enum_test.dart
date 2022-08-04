import 'package:test/test.dart';
import 'package:dart_example/accounts.dart';

// This test is exercised by `cargo test` which generates Dart code
// with the `.with_c_style_enums(false)` membrane option enabled. To run this test
// directly via `dart test` you must first generate accounts.dart with this option as it
// defaults to `.with_c_style_enums(true)` and consequently expects `Status.active` enums.
// You may accomplish this by modifying generator.rs and then running `cargo run`
void main() {
  test('can handle a class enum when `with_c_style_enums` is set to `false`',
      () async {
    final accounts = AccountsApi();
    expect((await accounts.enumReturn(status: StatusActiveItem())),
        equals(StatusActiveItem()));
    expect((await accounts.enumReturn(status: StatusPendingItem())),
        isNot(equals(StatusActiveItem())));
  });
}
