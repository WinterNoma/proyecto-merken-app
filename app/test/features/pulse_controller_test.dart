import 'package:flutter_test/flutter_test.dart';
import 'package:pulsys/features/pulse/pulse_controller.dart';
import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';

void main() {
  PulseController controller(FakePulsysApi api) {
    var next = 0;
    final result = PulseController(
      api: api,
      keyGenerator: () => 'intent-${next++}',
    );
    addTearDown(result.dispose);
    result.updateBpm('73.5');
    return result;
  }

  test('completed intent stays locked until the content changes', () async {
    final api = FakePulsysApi();
    final pulse = controller(api);
    final key = pulse.currentIntentKey;
    await pulse.submit();
    await pulse.submit();

    expect(pulse.currentIntentKey, key);
    expect(pulse.phase, PulseFormPhase.saved);
    expect(pulse.canSubmit, isFalse);
    expect(api.confirmedActions, hasLength(1));
    expect(api.historySnapshot, hasLength(1));

    pulse.updateBpm('74');
    expect(pulse.currentIntentKey, isNot(key));
    expect(pulse.canSubmit, isTrue);
    await pulse.submit();
    expect(api.historySnapshot, hasLength(2));
  });

  test('expired intent is prepared for the new target and reviewed', () async {
    final api = FakePulsysApi(astronautId: 'previous');
    api.enqueueConfirmError(
      const ApiError(
        code: ApiErrorCodes.actionExpired,
        field: ApiFields.key,
        message: 'Expired target',
      ),
    );
    final pulse = controller(api);
    await pulse.submit();
    final old = api.confirmedActions.single;
    expect(pulse.bpmText, '73.5');
    api.astronautId = 'current';
    await pulse.submit();

    expect(api.preparedInputs, hasLength(2));
    expect(api.confirmedActions, hasLength(1));
    expect(pulse.currentIntentKey, isNot(old.key));
    expect(
      pulse.pendingConfirmation?.target,
      const LocalPulseTarget(storeId: 'fake-store-1', astronautId: 'current'),
    );
    expect(api.historySnapshot, isEmpty);
    await pulse.confirmPending();
    expect(api.historySnapshot, hasLength(1));
    expect(api.confirmedActions.last.approved, isTrue);
  });

  test('transient failure retries the identical prepared action', () async {
    final api = FakePulsysApi();
    api.enqueueConfirmError(
      const ApiError(
        code: ApiErrorCodes.storageError,
        message: 'Temporary failure',
      ),
    );
    final pulse = controller(api);
    await pulse.submit();
    final first = api.confirmedActions.single;
    await pulse.submit();

    expect(api.preparedInputs, hasLength(1));
    expect(identical(api.confirmedActions.last, first), isTrue);
    expect(api.historySnapshot, hasLength(1));
  });

  test('recovered intent reaches confirmation and cancellation', () async {
    final api = FakePulsysApi();
    final action = await api.preparePulse(
      PulseInput(key: 'recovered', bpm: Bpm(201)),
    );
    api.recovery = RecoveredPulse(action);
    final pulse = controller(api);
    await pulse.loadInitial();

    expect(pulse.phase, PulseFormPhase.confirming);
    expect(pulse.bpmText, '201');
    expect(pulse.currentIntentKey, 'recovered');
    expect(api.historySnapshot, isEmpty);
    await pulse.cancelConfirmation();
    expect(api.cancelledActions.single.key, 'recovered');
    expect(pulse.phase, PulseFormPhase.editing);
    expect(api.historySnapshot, isEmpty);
  });

  test(
    'history and status retry independently while receipt stays visible',
    () async {
      final api = FakePulsysApi();
      final pulse = controller(api);
      await pulse.submit();
      final receipt = pulse.lastReceipt;
      final snapshot = pulse.zoneSnapshot;
      api.enqueueHistoryError(
        const ApiError(code: 'storage_error', message: 'History failed'),
      );
      api.enqueueZoneStatusError(
        const ApiError(code: 'storage_error', message: 'Status failed'),
      );
      await pulse.refreshOverview();

      expect(pulse.historyLoadFailed, isTrue);
      expect(pulse.zonesLoadFailed, isTrue);
      expect(pulse.zoneSnapshot, same(snapshot));
      expect(pulse.lastReceipt, same(receipt));
      final statusCalls = api.zoneStatusCalls;
      await pulse.refreshHistory();
      expect(pulse.historyLoadFailed, isFalse);
      expect(pulse.zonesLoadFailed, isTrue);
      expect(api.zoneStatusCalls, statusCalls);
      final historyCalls = api.historyQueries.length;
      await pulse.refreshZones();
      expect(pulse.zonesLoadFailed, isFalse);
      expect(api.historyQueries, hasLength(historyCalls));
      expect(pulse.lastReceipt, same(receipt));
    },
  );
  test(
    'transient failure keeps the exact explicitly approved action',
    () async {
      final api = FakePulsysApi();
      api.enqueuePrepareWarnings(const <PulseWarning>[
        PulseWarning(code: PulseWarning.unusualValue, ruleId: 'fixture'),
      ]);
      final pulse = controller(api);
      await pulse.submit();
      api.enqueueConfirmError(
        const ApiError(
          code: ApiErrorCodes.storageError,
          message: 'Temporary failure after approval',
        ),
      );
      await pulse.confirmPending();
      final approved = api.confirmedActions.last;
      expect(approved.approved, isTrue);
      await pulse.submit();
      expect(api.confirmedActions.last, same(approved));
      expect(api.preparedInputs, hasLength(1));
      expect(api.historySnapshot, hasLength(1));
    },
  );
  test('new reading cannot discard a pending submission', () async {
    final api = FakePulsysApi();
    api.enqueueConfirmError(
      const ApiError(
        code: ApiErrorCodes.storageError,
        message: 'Temporary failure',
      ),
    );
    final pulse = controller(api);
    await pulse.submit();
    final key = pulse.currentIntentKey;
    final prepared = api.confirmedActions.single;
    expect(pulse.canStartNewReading, isFalse);
    pulse.startNewReading();
    expect(pulse.currentIntentKey, key);
    expect(pulse.bpmText, '73.5');
    await pulse.submit();
    expect(api.confirmedActions.last, same(prepared));
    expect(pulse.canSubmit, isFalse);
    await pulse.submit();
    expect(api.historySnapshot, hasLength(1));
    expect(api.confirmedActions, hasLength(2));
    pulse.startNewReading();
    expect(pulse.currentIntentKey, isNot(key));
    expect(pulse.phase, PulseFormPhase.editing);
    expect(pulse.bpmText, isEmpty);
    expect(pulse.measuredAtText, isEmpty);
    expect(pulse.noteText, isEmpty);
    expect(pulse.replaces, isNull);
    expect(pulse.lastReceipt, isNull);
  });

  test('bpm-only correction preserves the exact measured instant', () async {
    final measuredAtUtc = DateTime.utc(2026, 11, 18, 12, 34, 56, 789);
    final api = FakePulsysApi(clock: () => measuredAtUtc);
    final pulse = controller(api);
    await pulse.submit();
    final original = pulse.history.single;
    pulse.startCorrection(original);
    pulse.updateBpm('74');
    await pulse.submit();
    expect(api.preparedInputs.last.measuredAtUtc, measuredAtUtc);
    expect(pulse.lastReceipt?.measuredAtUtc, measuredAtUtc);
    expect(pulse.history.single.measuredAtUtc, measuredAtUtc);
    expect(pulse.history.single.replaces, original.recordId);
    expect(pulse.history.single.rootId, original.rootId);
  });

  test('editing correction time replaces the preserved instant', () async {
    final measuredAtUtc = DateTime.utc(2026, 11, 18, 12, 34, 56, 789);
    final api = FakePulsysApi(clock: () => measuredAtUtc);
    final pulse = controller(api);
    await pulse.submit();
    pulse.startCorrection(pulse.history.single);
    pulse.updateBpm('74');
    pulse.updateMeasuredAt('2026-11-18 12:35');
    await pulse.submit();
    final editedAtUtc = DateTime.utc(2026, 11, 18, 12, 35);
    expect(api.preparedInputs.last.measuredAtUtc, editedAtUtc);
    expect(pulse.lastReceipt?.measuredAtUtc, editedAtUtc);
  });

  test('invalid or cleared time never reuses the preserved instant', () async {
    var nowUtc = DateTime.utc(2026, 11, 18, 12, 34, 56, 789);
    final originalAtUtc = nowUtc;
    final api = FakePulsysApi(clock: () => nowUtc);
    final pulse = controller(api);
    await pulse.submit();
    pulse.startCorrection(pulse.history.single);
    pulse.updateBpm('74');
    pulse.updateMeasuredAt('invalid');
    await pulse.submit();
    expect(pulse.measuredAtError, TimeValidationError.invalidFormat);
    expect(api.preparedInputs, hasLength(1));
    nowUtc = DateTime.utc(2026, 11, 18, 13, 0, 1, 123);
    pulse.updateMeasuredAt('');
    await pulse.submit();
    expect(api.preparedInputs.last.measuredAtUtc, isNull);
    expect(pulse.lastReceipt?.measuredAtUtc, nowUtc);
    expect(pulse.lastReceipt?.measuredAtUtc, isNot(originalAtUtc));
  });
}
