import 'package:test/test.dart';
import 'package:dart_example/accounts.dart';
import 'package:dart_example/locations.dart';

void main() {
  test('can take one item from a stream', () async {
    final accounts = AccountsApi();
    expect(
        await accounts.contacts().take(1).toList(),
        equals(
            [Contact(id: 1, fullName: "Alice Smith", status: Status.pending)]));
  });

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

  test('can call a function that returns a scalar value', () async {
    final accounts = AccountsApi();
    expect(await accounts.scalarI32(val: 123), equals(123));
    expect(await accounts.scalarI64(val: 10), equals(10));
    expect((await accounts.scalarF32(val: 21.1)).toStringAsFixed(1),
        equals('21.1'));
    expect(await accounts.scalarF64(val: 11.1), equals(11.1));
    expect(
        await accounts.scalarString(val: "hello world"), equals("hello world"));
    expect(await accounts.scalarBool(val: true), equals(true));
    expect(() async => await accounts.scalarEmpty(), returnsNormally);
  });

  test(
      'test that a function throws an ApiError instance when an error is returned',
      () async {
    final accounts = AccountsApi();
    expect(() async => await accounts.scalarError(),
        throwsA(isA<AccountsApiError>()));
  });

  test('test that a function throws a string when an error is returned',
      () async {
    final accounts = AccountsApi();
    try {
      await accounts.scalarError();
    } on AccountsApiError catch (err) {
      expect(err.e, "an error message");
    }
  });

  test(
      'test that u8, u32, u64, u128, i8, i32, i64, and i128 types are supported',
      () async {
    final accounts = AccountsApi();
    final types = MoreTypes(
        unsigned8: 255,
        unsigned16: 65535,
        unsigned32: 4294967295,
        unsigned64: Uint64.parse('18446744073709551615'),
        signed8: 127,
        signed16: 32767,
        signed32: 2147483647,
        signed64: 9223372036854775807,
        unsigned128Min: Uint128.parse('0'),
        // fits in 64 bit
        unsigned12864: Uint128.parse('200'),
        unsigned128Max:
            Uint128.parse('340282366920938463463374607431768211455'),
        signed128Min: Int128.parse('-170141183460469231731687303715884105728'),
        // fits in 64 bit
        signed12864: Int128.parse('300'),
        // fits in 64 bit
        signed128Neg64: Int128.parse('-300'),
        signed128Max: Int128.parse('170141183460469231731687303715884105727'),
        float32: 3.140000104904175,
        float64: 1.7976931348623157e+308);

    final returned = await accounts.moreTypes(types: types);
    expect(returned.toString(), types.toString());
  });

  test('can pass an enum as a function arg', () async {
    final accounts = AccountsApi();
    expect((await accounts.enumArg(status: Status.active)).status,
        equals(Status.active));
    expect((await accounts.enumArg(status: Status.pending)).status,
        isNot(equals(Status.active)));
    expect((await accounts.enumArg(status: Status.pending)).status,
        equals(Status.pending));
  });

  test('can pass an optional enum as a function arg', () async {
    final accounts = AccountsApi();
    expect((await accounts.optionalEnumArg()).status, equals(Status.pending));
    expect((await accounts.optionalEnumArg(status: Status.pending)).status,
        equals(Status.pending));
    expect((await accounts.optionalEnumArg(status: Status.active)).status,
        equals(Status.active));
  });

  test('can receive an enum as the only returned value', () async {
    final accounts = AccountsApi();
    expect((await accounts.enumReturn(status: Status.active)),
        equals(Status.active));
    expect((await accounts.enumReturn(status: Status.pending)),
        isNot(equals(Status.active)));
  });

  test('can fetch a vector from a separate namespace', () async {
    final locations = LocationsApi();
    expect(
        (await locations.getLocation(id: 10)).polylineCoords,
        equals([
          [-104.0185546875, 43.004647127794435],
          [-104.0625, 37.78808138412046],
          [-94.130859375, 37.85750715625203]
        ]));
  });
}
