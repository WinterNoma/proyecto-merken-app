import 'dart:typed_data';

import 'package:pulsys/platform/device/rust_session.dart';
import 'package:pulsys/platform/generated/api.dart' as ffi;

final class RustStoreFixture {
  final Map<int, ffi.PreparedPulse> encoded = <int, ffi.PreparedPulse>{};
  final Map<String, ffi.PulseReceipt> receipts = <String, ffi.PulseReceipt>{};
  final List<ffi.PreparedPulse> confirmed = <ffi.PreparedPulse>[];
  ffi.PreparedPulse? nextConfirmation;
  ffi.ApiError? confirmError;
  ffi.ApiError? decodeError;
}

final class FakeRustSession implements RustSession {
  FakeRustSession(this.store);

  final RustStoreFixture store;
  bool closed = false;

  @override
  Future<ffi.PreparedPulse> preparePulse(ffi.PulseInput input) async =>
      ffi.PreparedPulse(
        key: input.key,
        bpm: input.bpm,
        measuredAtMs: input.measuredAtMs ?? 1800000000000,
        note: input.note,
        replaces: input.replaces,
        context: const ffi.PulseContext(
          target: ffi.PulseTarget.local(storeId: 'store'),
        ),
        warnings: const <ffi.PulseWarning>[],
        confirmation: ffi.PulseConfirmation.notRequired,
      );

  @override
  Future<Uint8List> encodePreparedPulse(ffi.PreparedPulse action) async {
    final token = store.encoded.length + 1;
    store.encoded[token] = action;
    return Uint8List.fromList(<int>[token]);
  }

  @override
  Future<ffi.PreparedPulse> decodePreparedPulse(Uint8List bytes) async {
    final error = store.decodeError;
    if (error != null) {
      throw error;
    }
    final action = store.encoded[bytes.single];
    if (action == null) {
      throw const ffi.ApiError(
        code: 'invalid_value',
        field: 'action',
        message: 'Invalid fixture',
      );
    }
    return action;
  }

  @override
  Future<ffi.SaveOutcome> confirmPulse(ffi.PreparedPulse action) async {
    store.confirmed.add(action);
    final error = store.confirmError;
    store.confirmError = null;
    if (error != null) {
      throw error;
    }
    final updated = store.nextConfirmation;
    store.nextConfirmation = null;
    if (updated != null) {
      return ffi.SaveOutcome.needsConfirmation(prepared: updated);
    }
    if (action.confirmation == ffi.PulseConfirmation.required_) {
      return ffi.SaveOutcome.needsConfirmation(prepared: action);
    }
    final receipt = store.receipts.putIfAbsent(
      action.key,
      () => ffi.PulseReceipt(
        key: action.key,
        recordId: 'record-${store.receipts.length + 1}',
        rootId: 'root',
        bpm: action.bpm,
        unit: 'bpm',
        source: ffi.RecordSource.form,
        measuredAtMs: action.measuredAtMs,
        loggedAtMs: 1800000000001,
        isDemo: false,
        outboxState: ffi.PulseOutboxState.pending,
      ),
    );
    return ffi.SaveOutcome.saved(receipt: receipt);
  }

  @override
  Future<List<ffi.PulsePoint>> effectivePulseHistory(
    ffi.HistoryQuery query,
  ) async => store.receipts.values
      .map(
        (receipt) => ffi.PulsePoint(
          recordId: receipt.recordId,
          rootId: receipt.rootId,
          bpm: receipt.bpm,
          unit: receipt.unit,
          source: receipt.source,
          measuredAtMs: receipt.measuredAtMs,
          loggedAtMs: receipt.loggedAtMs,
          isDemo: receipt.isDemo,
        ),
      )
      .toList();

  @override
  Future<ffi.ZoneSnapshot> zoneStatus() async => const ffi.ZoneSnapshot(
    evaluatedAtMs: 1800000000000,
    isDemo: false,
    zones: <ffi.ZoneState>[],
    pulse: ffi.PulseEvaluation(
      status: ffi.EvaluationStatus.notEvaluated,
      reason: ffi.NotEvaluatedReason.noData,
      persistent: false,
      escalation: ffi.EscalationLevel.continueReading,
      evaluatedAtMs: 1800000000000,
      ruleId: 'fixture',
      ruleVersion: 'fixture',
      sourceKind: ffi.SourceKind.demoHeuristic,
      inputRecordIds: <String>[],
      historyCount: 0,
      historyNeeded: 0,
    ),
  );

  @override
  Future<void> close() async {
    closed = true;
  }
}
