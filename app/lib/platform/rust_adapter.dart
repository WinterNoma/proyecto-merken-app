import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart' show listEquals;
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart' show FrbException;

import 'device/pending_pulse_file.dart';
import 'device/rust_session.dart';
import 'generated/api.dart' as ffi;
import 'pulsys_api.dart';

final class RustAdapter implements PulsysApi {
  RustAdapter._(this._session, this._pendingFile);

  static const _closed = ApiError(
    code: ApiErrorCodes.closed,
    message: 'The store session is closed.',
  );
  static const _busy = ApiError(
    code: ApiErrorCodes.busy,
    message: 'A pulse confirmation is already in progress.',
  );
  static const _expired = ApiError(
    code: ApiErrorCodes.actionExpired,
    field: ApiFields.key,
    message: 'The pulse must be prepared again.',
  );

  final RustSession _session;
  PulseRecovery _recovery = const NoPendingPulse();
  final PendingPulseFile _pendingFile;
  // PreparedPulse.approve() keeps the same target instance.
  final _preparedByTarget = Expando<ffi.PreparedPulse>();
  ffi.PreparedPulse? _pendingAction;
  Completer<void>? _confirming;
  Future<void>? _closing;

  static Future<RustAdapter> open({
    required Directory privateDirectory,
    required String appVersion,
  }) async {
    final session = await _call(
      () => NativeRustSession.open(
        privateDirectory: privateDirectory,
        appVersion: appVersion,
      ),
    );
    return connect(session: session, privateDirectory: privateDirectory);
  }

  static Future<RustAdapter> connect({
    required RustSession session,
    required Directory privateDirectory,
  }) async {
    final adapter = RustAdapter._(session, PendingPulseFile(privateDirectory));
    await adapter._restorePending();
    return adapter;
  }

  @override
  Future<PulseRecovery> recoverPulse() async {
    _ensureOpen();
    if (_confirming != null) {
      throw _busy;
    }
    if (_recovery is PulseRecoveryBlocked) {
      await _restorePending();
    }
    return _recovery;
  }

  @override
  Future<void> cancelPendingPulse(PreparedPulse action) async {
    _ensureOpen();
    if (_confirming != null) {
      throw _busy;
    }
    if (_recovery case PulseRecoveryBlocked(:final error)) {
      throw error;
    }
    final pending = _pendingAction;
    if (pending == null) {
      return;
    }
    final original = _preparedByTarget[action.target];
    if (original == null || !_sameIntent(original, pending)) {
      throw _expired;
    }
    await _clearPending();
    _recovery = const NoPendingPulse();
  }

  @override
  Future<PreparedPulse> preparePulse(PulseInput input) async {
    _ensureOpen();
    final action = await _call(
      () => _session.preparePulse(
        ffi.PulseInput(
          key: input.key,
          bpm: input.bpm.value,
          measuredAtMs: input.measuredAtUtc?.millisecondsSinceEpoch,
          note: input.note,
          replaces: input.replaces,
        ),
      ),
    );
    return _prepared(action);
  }

  @override
  Future<SaveOutcome> confirmPulse(PreparedPulse action) async {
    _ensureOpen();
    if (_confirming != null) {
      throw _busy;
    }
    final original = _preparedByTarget[action.target];
    if (original == null || !_matchesPresentation(action, original)) {
      throw _expired;
    }

    final complete = Completer<void>();
    _confirming = complete;
    try {
      final outcome = await _confirm(_withApproval(original, action: action));
      return switch (outcome) {
        ffi.SaveOutcome_NeedsConfirmation(:final prepared) => NeedsConfirmation(
          _prepared(prepared),
        ),
        ffi.SaveOutcome_Saved(:final receipt) => Saved(_receipt(receipt)),
      };
    } finally {
      _confirming = null;
      complete.complete();
    }
  }

  Future<ffi.SaveOutcome> _confirm(ffi.PreparedPulse action) async {
    if (_recovery case PulseRecoveryBlocked(:final error)) {
      throw error;
    }
    final pending = _pendingAction;
    if (pending != null && pending.key != action.key) {
      return ffi.SaveOutcome.needsConfirmation(prepared: pending);
    }
    if (pending != null && !_sameIntent(action, pending)) {
      throw const ApiError(
        code: ApiErrorCodes.idempotencyConflict,
        field: ApiFields.key,
        message: 'The key belongs to another pending pulse.',
      );
    }
    if (pending == null &&
        action.confirmation == ffi.PulseConfirmation.required_) {
      return _submit(action);
    }

    await _stagePending(action);
    return _submitPending(action);
  }

  Future<void> _stagePending(ffi.PreparedPulse action) async {
    final bytes = await _call(() => _session.encodePreparedPulse(action));
    await _pendingFile.stage(bytes);
    _pendingAction = action;
    _recovery = RecoveredPulse(_prepared(action));
  }

  Future<ffi.SaveOutcome> _submit(ffi.PreparedPulse action) =>
      _call(() => _session.confirmPulse(action));

  Future<ffi.SaveOutcome> _submitPending(ffi.PreparedPulse action) async {
    final ffi.SaveOutcome outcome;
    try {
      outcome = await _submit(action);
    } on ApiError catch (error) {
      if (_isDefinitive(error)) {
        await _clearPending();
        _recovery = const NoPendingPulse();
      }
      rethrow;
    }
    switch (outcome) {
      case ffi.SaveOutcome_NeedsConfirmation(:final prepared):
        await _stagePending(prepared);
      case ffi.SaveOutcome_Saved(:final receipt):
        await _clearPending();
        _recovery = RecoveredReceipt(_receipt(receipt));
    }
    return outcome;
  }

  Future<void> _restorePending() async {
    try {
      final bytes = await _pendingFile.read();
      if (bytes == null) {
        _recovery = const NoPendingPulse();
        return;
      }
      final action = await _call(() => _session.decodePreparedPulse(bytes));
      _pendingAction = action;
      if (action.confirmation == ffi.PulseConfirmation.required_) {
        _recovery = RecoveredPulse(_prepared(action));
        return;
      }
      await _submitPending(action);
    } on ApiError catch (error) {
      _recovery = PulseRecoveryBlocked(error);
    }
  }

  Future<void> _clearPending() async {
    await _pendingFile.clear();
    _pendingAction = null;
  }

  @override
  Future<List<PulsePoint>> effectivePulseHistory([HistoryQuery? query]) async {
    _ensureOpen();
    final points = await _call(
      () => _session.effectivePulseHistory(
        ffi.HistoryQuery(
          fromMs: query?.fromUtc?.millisecondsSinceEpoch,
          toMs: query?.toUtc?.millisecondsSinceEpoch,
          limit: query?.limit,
        ),
      ),
    );
    return List<PulsePoint>.unmodifiable(points.map(_point));
  }

  @override
  Future<ZoneSnapshot> zoneStatus() async {
    _ensureOpen();
    final snapshot = await _call(_session.zoneStatus);
    return ZoneSnapshot(
      evaluatedAtUtc: _utc(snapshot.evaluatedAtMs),
      isDemo: snapshot.isDemo,
      isStoreEmpty: snapshot.isStoreEmpty,
      zones: snapshot.zones.map(_zone).toList(),
      pulse: _evaluation(snapshot.pulse),
    );
  }

  @override
  Future<void> close() => _closing ??= _close();

  Future<void> _close() async {
    final confirming = _confirming;
    if (confirming != null) {
      await confirming.future;
    }
    await _call(_session.close);
  }

  void _ensureOpen() {
    if (_closing != null) {
      throw _closed;
    }
  }

  PreparedPulse _prepared(ffi.PreparedPulse action) {
    final target = switch (action.context.target) {
      ffi.PulseTarget_Local(:final storeId, :final astronautId) =>
        LocalPulseTarget(storeId: storeId, astronautId: astronautId),
      ffi.PulseTarget_Demo(:final field0) => DemoPulseTarget(
        demo: _demo(field0),
      ),
    };
    _preparedByTarget[target] = action;
    return PreparedPulse(
      key: action.key,
      bpm: Bpm(action.bpm),
      measuredAtUtc: _utc(action.measuredAtMs),
      note: action.note,
      replaces: action.replaces,
      target: target,
      warnings: action.warnings.map(_warning).toList(),
      demoToReplace: _optional(action.context.demoToReplace, _demo),
      confirmation: switch (action.confirmation) {
        ffi.PulseConfirmation.notRequired => PulseConfirmation.notRequired,
        ffi.PulseConfirmation.required_ => PulseConfirmation.requiredReview,
        ffi.PulseConfirmation.approved => PulseConfirmation.approved,
      },
    );
  }
}

Future<T> _call<T>(Future<T> Function() operation) async {
  try {
    return await operation();
  } on ffi.ApiError catch (error) {
    throw ApiError(
      code: error.code,
      field: error.field,
      message: error.message,
    );
  } on FrbException {
    throw const ApiError(
      code: ApiErrorCodes.storageError,
      message: 'The native store operation failed.',
    );
  }
}

bool _isDefinitive(ApiError error) => switch (error.code) {
  ApiErrorCodes.actionExpired ||
  ApiErrorCodes.idempotencyConflict ||
  ApiErrorCodes.invalidValue ||
  ApiErrorCodes.notFound ||
  'unresolved_root' => true,
  _ => false,
};

bool _matchesPresentation(PreparedPulse action, ffi.PreparedPulse original) =>
    action.key == original.key &&
    action.bpm.value == original.bpm &&
    action.measuredAtUtc == _utc(original.measuredAtMs) &&
    action.note == original.note &&
    action.replaces == original.replaces &&
    action.demoToReplace == _optional(original.context.demoToReplace, _demo) &&
    listEquals(action.warnings, original.warnings.map(_warning).toList());

bool _sameIntent(ffi.PreparedPulse action, ffi.PreparedPulse pending) =>
    action.key == pending.key &&
    action.bpm == pending.bpm &&
    action.measuredAtMs == pending.measuredAtMs &&
    action.note == pending.note &&
    action.replaces == pending.replaces &&
    action.context.target == pending.context.target &&
    action.context.correctionRootId == pending.context.correctionRootId;

ffi.PreparedPulse _withApproval(
  ffi.PreparedPulse original, {
  required PreparedPulse action,
}) {
  if (!action.approved) {
    return original;
  }
  return ffi.PreparedPulse(
    key: original.key,
    bpm: original.bpm,
    measuredAtMs: original.measuredAtMs,
    note: original.note,
    replaces: original.replaces,
    context: original.context,
    warnings: original.warnings,
    confirmation: ffi.PulseConfirmation.approved,
  );
}

DateTime _utc(int ms) => DateTime.fromMillisecondsSinceEpoch(ms, isUtc: true);

R? _optional<T, R>(T? value, R? Function(T) convert) =>
    value == null ? null : convert(value);

DemoTarget _demo(ffi.DemoTarget value) =>
    DemoTarget(astronautId: value.astronautId, generation: value.generation);

PulseWarning _warning(ffi.PulseWarning value) =>
    PulseWarning(code: value.code, ruleId: value.ruleId);

PulseReceipt _receipt(ffi.PulseReceipt value) => PulseReceipt(
  key: value.key,
  recordId: value.recordId,
  rootId: value.rootId,
  replaces: value.replaces,
  bpm: _bpm(value.bpm, value.unit),
  source: _recordSource(value.source),
  measuredAtUtc: _utc(value.measuredAtMs),
  loggedAtUtc: _utc(value.loggedAtMs),
  isDemo: value.isDemo,
  outboxState: switch (value.outboxState) {
    ffi.PulseOutboxState.pending => PulseOutboxState.pending,
    ffi.PulseOutboxState.localOnly => PulseOutboxState.localOnly,
  },
);

PulsePoint _point(ffi.PulsePoint value) => PulsePoint(
  recordId: value.recordId,
  rootId: value.rootId,
  replaces: value.replaces,
  bpm: _bpm(value.bpm, value.unit),
  source: _recordSource(value.source),
  measuredAtUtc: _utc(value.measuredAtMs),
  loggedAtUtc: _utc(value.loggedAtMs),
  isDemo: value.isDemo,
);

ZoneState _zone(ffi.ZoneState value) => ZoneState(
  zoneId: _bodyZone(value.zoneId),
  status: _status(value.status),
  symptoms: value.symptoms.map(_symptom).toList(),
);

SymptomEvaluation _symptom(ffi.SymptomEvaluation value) => SymptomEvaluation(
  status: _status(value.status),
  reason: _optional(value.reason, _reason),
  evaluatedAtUtc: _utc(value.evaluatedAtMs),
  ruleId: value.ruleId,
  ruleVersion: value.ruleVersion,
  sourceKind: _sourceKind(value.sourceKind),
  sourceRef: value.sourceRef,
  inputRecordIds: value.inputRecordIds,
  inputEventIds: value.inputEventIds,
);

PulseEvaluation _evaluation(ffi.PulseEvaluation value) => PulseEvaluation(
  status: _status(value.status),
  reason: _optional(value.reason, _reason),
  direction: _optional(value.direction, _direction),
  persistent: value.persistent,
  escalation: _escalation(value.escalation),
  evaluatedAtUtc: _utc(value.evaluatedAtMs),
  latestMeasuredAtUtc: _optional(value.latestMeasuredAtMs, _utc),
  ruleId: value.ruleId,
  ruleVersion: value.ruleVersion,
  sourceKind: _sourceKind(value.sourceKind),
  sourceRef: value.sourceRef,
  inputRecordIds: value.inputRecordIds,
  historyCount: value.historyCount,
  historyNeeded: value.historyNeeded,
  baselineBpm: _optional(value.baselineBpm, Bpm.tryFrom),
);

BodyZone _bodyZone(ffi.BodyZone value) => switch (value) {
  ffi.BodyZone.head => BodyZone.head,
  ffi.BodyZone.neck => BodyZone.neck,
  ffi.BodyZone.chest => BodyZone.chest,
  ffi.BodyZone.abdomen => BodyZone.abdomen,
  ffi.BodyZone.back => BodyZone.back,
  ffi.BodyZone.leftArm => BodyZone.leftArm,
  ffi.BodyZone.rightArm => BodyZone.rightArm,
  ffi.BodyZone.leftHand => BodyZone.leftHand,
  ffi.BodyZone.rightHand => BodyZone.rightHand,
  ffi.BodyZone.leftLeg => BodyZone.leftLeg,
  ffi.BodyZone.rightLeg => BodyZone.rightLeg,
  ffi.BodyZone.leftFoot => BodyZone.leftFoot,
  ffi.BodyZone.rightFoot => BodyZone.rightFoot,
  ffi.BodyZone.other => BodyZone.other,
};

Bpm _bpm(double value, String unit) {
  final bpm = Bpm.tryFrom(value);
  if (unit != Bpm.unit || bpm == null) {
    throw const ApiError(
      code: ApiErrorCodes.invalidSchema,
      field: ApiFields.bpm,
      message: 'The store returned an invalid pulse measurement.',
    );
  }
  return bpm;
}

RecordSource _recordSource(ffi.RecordSource value) => switch (value) {
  ffi.RecordSource.form => RecordSource.form,
  ffi.RecordSource.text => RecordSource.text,
  ffi.RecordSource.voice => RecordSource.voice,
  ffi.RecordSource.earth => RecordSource.earth,
  ffi.RecordSource.garmin => RecordSource.garmin,
};

EvaluationStatus _status(ffi.EvaluationStatus value) => switch (value) {
  ffi.EvaluationStatus.notEvaluated => EvaluationStatus.notEvaluated,
  ffi.EvaluationStatus.stable => EvaluationStatus.stable,
  ffi.EvaluationStatus.change => EvaluationStatus.change,
  ffi.EvaluationStatus.alarm => EvaluationStatus.alarm,
};

NotEvaluatedReason _reason(ffi.NotEvaluatedReason value) => switch (value) {
  ffi.NotEvaluatedReason.noData => NotEvaluatedReason.noData,
  ffi.NotEvaluatedReason.tooFewData => NotEvaluatedReason.tooFewData,
  ffi.NotEvaluatedReason.stale => NotEvaluatedReason.stale,
  ffi.NotEvaluatedReason.notApplicable => NotEvaluatedReason.notApplicable,
};

TrendDirection _direction(ffi.TrendDirection value) => switch (value) {
  ffi.TrendDirection.up => TrendDirection.up,
  ffi.TrendDirection.down => TrendDirection.down,
};

EscalationLevel _escalation(ffi.EscalationLevel value) => switch (value) {
  ffi.EscalationLevel.continueReading => EscalationLevel.continueReading,
  ffi.EscalationLevel.selfCare => EscalationLevel.selfCare,
  ffi.EscalationLevel.flagForGround => EscalationLevel.flagForGround,
  ffi.EscalationLevel.urgent => EscalationLevel.urgent,
};

SourceKind _sourceKind(ffi.SourceKind value) => switch (value) {
  ffi.SourceKind.published => SourceKind.published,
  ffi.SourceKind.approvedProtocol => SourceKind.approvedProtocol,
  ffi.SourceKind.demoHeuristic => SourceKind.demoHeuristic,
};
