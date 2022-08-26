import 'package:test/test.dart';
import 'package:dart_tests/c_example.dart';

// This test is exercised by `cargo test` which compiles a C program via build.rs.
// To run this test run via `cargo test --features c-example`
void main() {
  test('can call C function with background C threads emitting a stream',
      () async {
    final c_example = CExampleApi();
    final strings = await c_example.callAsyncC().take(2).toList();
    expect(strings.length, 2);
    expect(strings.every((s) => s.startsWith("This is a string from")), true);
  });
}
