import 'package:test/test.dart';
import 'package:dart_tests/c_render.dart';

// This test is exercised by `cargo test` which compiles a C program via build.rs.
// To run this test run via `cargo test --features c-render`
void main() {
  test(
      'can call Rust which then splits data streams and sends to both C and Dart',
      () async {
    final c_render = CRenderApi();
    try {
      final strings = await c_render.renderViaC().take(2).toList();
      expect(strings.length, 2);
      expect(strings, ["hello world 10", "hello world 20"]);
    } on CRenderApiError catch (e) {
      print('Received error: ${e.e}');
    }
  });
}
