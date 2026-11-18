import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:pulsys/features/pulse/pulse_controller.dart';
import 'package:pulsys/platform/generated/api.dart' as ffi;
import 'package:pulsys/platform/pulsys_api.dart';
import 'package:pulsys/platform/rust_adapter.dart';

import '../fakes/fake_rust_session.dart';

void main() {
  late Directory directory;
  late File pending;
  late RustStoreFixture store;

  setUp(() async {
    directory = await Directory.systemTemp.createTemp('pulsys-recovery-');
    pending = File('${directory.path}${Platform.pathSeparator}pulse-pending');
    store = RustStoreFixture();
  });
  tearDown(() => directory.delete(recursive: true));

  Future<RustAdapter> open() async {
    final adapter = await RustAdapter.connect(
      session: FakeRustSession(store),
      privateDirectory: directory,
    );
    addTearDown(adapter.close);
    return adapter;
  }

  Future<PreparedPulse> interrupt(RustAdapter adapter) async {
    final action = await adapter.preparePulse(
      PulseInput(
        key: 'intent',
        bpm: Bpm(73.5),
        measuredAtUtc: DateTime.fromMillisecondsSinceEpoch(
          1800000000000,
          isUtc: true,
        ),
      ),
    );
    store.confirmError = const ffi.ApiError(
      code: 'storage_error',
      message: 'Interrupted',
    );
    await expectLater(adapter.confirmPulse(action), throwsA(isA<ApiError>()));
    expect(await pending.exists(), isTrue);
    return action;
  }

  ffi.PreparedPulse renewed({
    List<ffi.PulseWarning> warnings = const <ffi.PulseWarning>[],
  }) => ffi.PreparedPulse(
    key: 'intent',
    bpm: 73.5,
    measuredAtMs: 1800000000000,
    context: const ffi.PulseContext(
      target: ffi.PulseTarget.local(storeId: 'store'),
    ),
    warnings: warnings,
    confirmation: ffi.PulseConfirmation.required_,
  );

  test(
    'reopening preserves changed warnings until the controller approves',
    () async {
      final first = await open();
      await interrupt(first);
      await first.close();
      store.nextConfirmation = renewed(
        warnings: const <ffi.PulseWarning>[
          ffi.PulseWarning(code: 'old_measurement', ruleId: 'fixture'),
        ],
      );
      final reopened = await open();
      final recovered = await reopened.recoverPulse();
      expect(recovered, isA<RecoveredPulse>());
      final action = (recovered as RecoveredPulse).prepared;
      expect(action.key, 'intent');
      expect(action.requiresConfirmation, isTrue);
      expect(action.warnings.single.code, 'old_measurement');
      expect(store.receipts, isEmpty);
      final bytes = await pending.readAsBytes();
      final onDisk = await FakeRustSession(store).decodePreparedPulse(bytes);
      expect(onDisk.confirmation, ffi.PulseConfirmation.required_);

      await reopened.close();
      final restoredAgain = await open();
      final controller = PulseController(api: restoredAgain);
      addTearDown(controller.dispose);
      await controller.loadInitial();
      expect(controller.phase, PulseFormPhase.confirming);
      expect(controller.currentIntentKey, 'intent');
      expect(store.receipts, isEmpty);
      await controller.confirmPending();
      expect(controller.phase, PulseFormPhase.saved);
      expect(store.receipts, hasLength(1));
      expect(await pending.exists(), isFalse);
      await controller.submit();
      expect(store.receipts, hasLength(1));
    },
  );

  test(
    'cancelling a recovered confirmation removes only its pending action',
    () async {
      final first = await open();
      await interrupt(first);
      await first.close();
      store.nextConfirmation = renewed();
      final reopened = await open();
      final action = (await reopened.recoverPulse() as RecoveredPulse).prepared;
      expect(action.requiresConfirmation, isTrue);
      await reopened.cancelPendingPulse(action);
      expect(await pending.exists(), isFalse);
      expect(store.receipts, isEmpty);
      expect(await reopened.recoverPulse(), isA<NoPendingPulse>());
    },
  );

  test(
    'another key surfaces the existing confirmation and keeps its file',
    () async {
      final first = await open();
      await interrupt(first);
      await first.close();
      store.nextConfirmation = renewed();
      final reopened = await open();
      final bytes = await pending.readAsBytes();
      final other = await reopened.preparePulse(
        PulseInput(key: 'other', bpm: Bpm(74)),
      );
      final outcome = await reopened.confirmPulse(other);
      expect(outcome, isA<NeedsConfirmation>());
      expect((outcome as NeedsConfirmation).prepared.key, 'intent');
      expect(await pending.readAsBytes(), orderedEquals(bytes));
      expect(store.receipts, isEmpty);
    },
  );

  test(
    'future durable version survives opening, retrying and attempted saving',
    () async {
      final bytes = Uint8List.fromList(
        utf8.encode('{"version":"2","action":{}}'),
      );
      await pending.writeAsBytes(bytes, flush: true);
      store.decodeError = const ffi.ApiError(
        code: ApiErrorCodes.unsupportedActionVersion,
        field: 'action.version',
        message: 'Future version',
      );
      final adapter = await open();
      for (var attempt = 0; attempt < 2; attempt++) {
        final recovery = await adapter.recoverPulse();
        expect(recovery, isA<PulseRecoveryBlocked>());
        expect(
          (recovery as PulseRecoveryBlocked).error.code,
          ApiErrorCodes.unsupportedActionVersion,
        );
        expect(await pending.readAsBytes(), orderedEquals(bytes));
      }
      final other = await adapter.preparePulse(
        PulseInput(key: 'other', bpm: Bpm(74)),
      );
      await expectLater(
        adapter.confirmPulse(other),
        throwsA(
          isA<ApiError>().having(
            (error) => error.code,
            'code',
            ApiErrorCodes.unsupportedActionVersion,
          ),
        ),
      );
      expect(await pending.readAsBytes(), orderedEquals(bytes));
      expect(store.receipts, isEmpty);
    },
  );
}
