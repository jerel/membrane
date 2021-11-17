import 'package:test/test.dart';
import 'package:dart_example/accounts.dart';
import 'package:dart_example/locations.dart';

void main() {
  test('can handle a class enum', () async {
    final accounts = AccountsApi();
    expect((await accounts.enumReturn(status: StatusActiveItem())),
        equals(StatusActiveItem()));
    expect((await accounts.enumReturn(status: StatusPendingItem())),
        isNot(equals(StatusActiveItem())));
  });
}
