import 'package:test/test.dart';
import 'package:dart_example/accounts.dart';
import 'package:dart_example/locations.dart';

void main() {
  test('can handle a class enum', () async {
    final accounts = AccountsApi();
    expect((await accounts.enumReturn(status: Status.active)),
        equals(Status.active));
    expect((await accounts.enumReturn(status: Status.active)),
        isNot(equals(Status.pending)));
  });
}
