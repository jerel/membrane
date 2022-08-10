import 'dart:async';
import 'dart:typed_data';

import 'package:logging/logging.dart';
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

  test('can get contacts synchronously', () {
    final accounts = AccountsApi();
    final result = accounts.contactSync(count: 100).value;
    expect(result.length, 100);

    expect(
        result.first,
        equals(
            Contact(id: 1, fullName: "Alice Smith", status: Status.pending)));
  });

  test('can call os-threaded Rust and get contact', () async {
    final accounts = AccountsApi();
    expect(
        await accounts.contactOsThread(userId: "1"),
        equals(
            Contact(id: 1, fullName: "Alice Smith", status: Status.pending)));
  });

  test('can call C in os thread and get contact async via emitter', () async {
    final accounts = AccountsApi();
    expect(
        await accounts.contactAsyncEmitter(userId: "1"),
        equals(
            Contact(id: 1, fullName: "Alice Smith", status: Status.pending)));
  });
  test('can call Rust in os thread and get contact async via streaming emitter',
      () async {
    final accounts = AccountsApi();
    final contacts =
        await accounts.contactAsyncStreamEmitter(userId: "1").take(2).toList();
    contacts.sort((a, b) => a.id.compareTo(b.id));

    expect(
        contacts,
        equals([
          Contact(id: 1, fullName: "Alice Smith", status: Status.pending),
          Contact(id: 2, fullName: "Alice Smith", status: Status.pending)
        ]));
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
    expect(await accounts.scalarI8(val: 123), equals(123));
    expect(await accounts.scalarU8(val: 123), equals(123));
    expect(await accounts.scalarI16(val: 123), equals(123));
    expect(await accounts.scalarU16(val: 123), equals(123));
    expect(await accounts.scalarI32(val: 123), equals(123));
    expect(await accounts.scalarU32(val: 123), equals(123));
    expect(await accounts.scalarI64(val: 10), equals(10));
    expect((await accounts.scalarF32(val: 21.1)).toStringAsFixed(1),
        equals('21.1'));
    expect(await accounts.scalarF64(val: 11.1), equals(11.1));
    expect(await accounts.scalarString(val: "hello world / ダミーテキスト"),
        equals("hello world / ダミーテキスト"));
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

  test('test that a vec with a large number of elements is handled', () async {
    final accounts = AccountsApi();
    final elements = List.filled(3000, 1.0);
    expect((await accounts.vec(v: VecWrapper(data: elements))).data, elements);
  });

  test(
      'test that UTF-8, u8, u32, u64, u128, i8, i32, i64, and i128 types are supported',
      () async {
    final accounts = AccountsApi();
    final types = MoreTypes(
        string: "hello world / ダミーテキスト",
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
        float64: 1.7976931348623157e+308,
        blob: Bytes(Uint8List.fromList(
            [104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100])));

    final returned = await accounts.moreTypes(types: types);
    expect(returned.toString(), types.toString());
  });

  test('can pass a vec of structs', () async {
    final accounts = AccountsApi();
    expect(
        (await accounts.vecStruct(values: [
          Contact(id: 1, fullName: 'Alice Smith', status: Status.pending),
          Contact(id: 2, fullName: 'John Smith', status: Status.active)
        ])),
        equals([
          Contact(id: 1, fullName: 'Alice Smith', status: Status.pending),
          Contact(id: 2, fullName: 'John Smith', status: Status.active)
        ]));
  });

  test('can handle a vec of strings', () async {
    final accounts = AccountsApi();
    expect((await accounts.vecString(values: ["hello", "world"])),
        equals(["hello", "world"]));
  });

  test('can handle a vec of booleans', () async {
    final accounts = AccountsApi();
    expect(
        (await accounts.vecBool(values: [true, false])), equals([true, false]));
  });

  test('can handle a vec of integers', () async {
    final accounts = AccountsApi();
    expect((await accounts.vecInt(values: [1, 2])), equals([1, 2]));
  });

  test('can handle a vec of floats', () async {
    final accounts = AccountsApi();
    expect((await accounts.vecFloat(values: [1.0, 2.1])), equals([1.0, 2.1]));
  });

  test('can handle a vec of vecs', () async {
    final accounts = AccountsApi();
    expect(
        (await accounts.vecVec(values: [
          [1, 2],
          [3, 4]
        ])),
        equals([
          [1, 2],
          [3, 4]
        ]));
  });

  test('can handle a vec of optional nullable vecs', () async {
    final accounts = AccountsApi();
    expect(
        (await accounts.vecVecOption(values: [
          [
            [1, null],
            [3, 4]
          ],
          [
            null,
            [5, 6]
          ]
        ])),
        equals([
          [
            [1, null],
            [3, 4]
          ],
          [
            null,
            [5, 6]
          ]
        ]));
  });

  test('can pass a vec of optional structs', () async {
    final accounts = AccountsApi();
    expect(
        (await accounts.vecOptionStruct(values: [
          null,
          Contact(id: 2, fullName: 'John Smith', status: Status.active)
        ])),
        equals([
          null,
          Contact(id: 2, fullName: 'John Smith', status: Status.active)
        ]));
  });

  test('can handle a vec of optional strings', () async {
    final accounts = AccountsApi();
    expect((await accounts.vecOptionString(values: [null, "hello", "world"])),
        equals([null, "hello", "world"]));
  });

  test('can handle a vec of optional booleans', () async {
    final accounts = AccountsApi();
    expect((await accounts.vecOptionBool(values: [false, null, true])),
        equals([false, null, true]));
  });

  test('can handle a vec of optional integers', () async {
    final accounts = AccountsApi();
    expect((await accounts.vecOptionInt(values: [1, null, 2])),
        equals([1, null, 2]));
  });

  test('can handle a vec of optional floats', () async {
    final accounts = AccountsApi();
    expect((await accounts.vecOptionFloat(values: [1.0, 2.1, null])),
        equals([1.0, 2.1, null]));
  });

  test('can handle an optional vec arg with optional return value', () async {
    final accounts = AccountsApi();
    expect((await accounts.optionalVecArg(values: [1.0, 2.1])),
        equals([1.0, 2.1]));

    expect((await accounts.optionalVecArg()), equals(null));
  });

  test('can handle an optional float arg with optional return value', () async {
    final accounts = AccountsApi();
    expect((await accounts.optionalFloatArg(value: 1.0)), equals(1.0));

    expect((await accounts.optionalFloatArg()), equals(null));
  });

  test('can pass a tuple arg containing a vec of structs', () async {
    final accounts = AccountsApi();
    expect(
        (await accounts.filterArg(
            filter:
                Filter(value: [Match(field: 'name', value: 'Alice Smith')]))),
        equals(Contacts(data: [
          Contact(id: 1, fullName: 'Alice Smith', status: Status.pending)
        ], count: 1, total: 1)));
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

  test('can receive a complex data enum', () async {
    final accounts = AccountsApi();
    expect(
        (await accounts.enumData()),
        equals(ReportsReportsItem(
            value: ReportsNameItem(value: "Example Report"))));
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

  test('calling a function emits a log event', () async {
    var logs = [];
    Logger.root.level = Level.ALL;
    Logger('membrane').onRecord.listen((event) {
      logs.add(event.toString());
    });

    final locations = LocationsApi();
    await locations.getLocation(id: 1);

    expect(logs.first, contains('[FINE] membrane.locations:'));
  });

  test('calling a function with its logging disabled does not emit a log event',
      () async {
    var logs = [];
    Logger.root.level = Level.ALL;
    Logger('membrane').onRecord.listen((event) {
      logs.add(event.toString());
    });

    final accounts = AccountsApi();
    await accounts.scalarEmpty();

    expect(logs, equals([]));
  });

  test(
      'test that a slow function will throw a timeout at its #[async_dart(timeout=100)] configured duration',
      () async {
    final accounts = AccountsApi();
    try {
      await accounts.slowFunction(sleepFor: 150);
    } on TimeoutException catch (err) {
      expect(err.duration?.inMilliseconds, 100);
      expect(err.message, "Future not completed");
    }
  });

  test(
      'test that a slow function will throw a timeout at its globally configured (200) timeout duration',
      () async {
    final accounts = AccountsApi();
    try {
      await accounts.slowFunctionTwo(sleepFor: 250);
    } on TimeoutException catch (err) {
      expect(err.duration?.inMilliseconds, 200);
      expect(err.message, "Future not completed");
    }
  });

  test(
      'test that a stream with a timeout will disconnect if the time between events exceeds set timeout',
      () async {
    final accounts = AccountsApi();
    try {
      await accounts.slowStream(sleepFor: 100).take(2).toList();
    } on TimeoutException catch (err) {
      expect(err.duration?.inMilliseconds, 50);
      expect(err.message, "No stream event");
    }
  });

  test(
      'test that panics in async code are handled gracefully and merely time out',
      () async {
    final accounts = AccountsApi();
    try {
      await accounts.contactPanic();
    } on TimeoutException catch (err) {
      expect(err.duration?.inMilliseconds, 200);
      expect(err.message, "Future not completed");
    }
  });

  test(
      'test that panics in stream code are handled gracefully and merely time out',
      () async {
    final accounts = AccountsApi();
    try {
      await accounts.contactStreamPanic().take(1).toList();
    } on TimeoutException catch (err) {
      expect(err.duration?.inMilliseconds, 20);
      expect(err.message, "No stream event");
    }
  });

  test('test that panics in sync code are handled gracefully', () {
    final accounts = AccountsApi();
    try {
      accounts.contactSyncPanic();
    } on AccountsApiError catch (err) {
      expect(err.e, "The sync rust code panicked");
    }
  });
}
