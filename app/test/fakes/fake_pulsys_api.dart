import 'dart:async';
import 'dart:collection';

import 'package:pulsys/platform/pulsys_api.dart';

typedef _PulseFingerprint = (PulseTarget, String?, double, String?, DateTime);

final class _StoredOperation {
  const _StoredOperation({required this.fingerprint, required this.receipt});

  final _PulseFingerprint fingerprint;
  final PulseReceipt receipt;
}

final class _PendingConfirmError {
  const _PendingConfirmError({
    required this.error,
    required this.saveBeforeThrowing,
  });

  final ApiError error;
  final bool saveBeforeThrowing;
}

final class FakePulsysApi implements PulsysApi {
  FakePulsysApi({
    DateTime Function()? clock,
    List<PulsePoint> initialHistory = const <PulsePoint>[],
    List<ZoneState>? initialZones,
    PulseEvaluation? initialPulseEvaluation,
    this.demoTarget,
    this.storeId = 'fake-store-1',
    this.astronautId,
    this.latency,
  }) : _clock = clock ?? (() => DateTime.utc(2026, 11, 18, 12, 0)),
       _zones = initialZones ?? _defaultZones(clock),
       _pulseEvaluation = initialPulseEvaluation {
    _history.addAll(initialHistory);
  }

  final DateTime Function() _clock;
  String storeId;
  String? astronautId;
  final Duration? latency;

  final List<PulseInput> preparedInputs = <PulseInput>[];
  final List<PreparedPulse> confirmedActions = <PreparedPulse>[];
  final List<HistoryQuery> historyQueries = <HistoryQuery>[];
  int zoneStatusCalls = 0;
  bool closed = false;

  Completer<void>? confirmGate;

  final Queue<List<PulseWarning>> _prepareWarnings =
      Queue<List<PulseWarning>>();
  final Queue<ApiError> _prepareErrors = Queue<ApiError>();
  final Queue<_PendingConfirmError> _confirmErrors =
      Queue<_PendingConfirmError>();
  final Queue<ApiError> _historyErrors = Queue<ApiError>();
  final Queue<ApiError> _zoneStatusErrors = Queue<ApiError>();

  final Map<String, _StoredOperation> _operationsByKey =
      <String, _StoredOperation>{};
  final List<PulsePoint> _history = <PulsePoint>[];
  DemoTarget? demoTarget;
  final List<ZoneState> _zones;
  final PulseEvaluation? _pulseEvaluation;
  int _nextRecordSeq = 1;

  PulseRecovery recovery = const NoPendingPulse();
  final List<PreparedPulse> cancelledActions = <PreparedPulse>[];
  final Queue<PreparedPulse> _confirmations = Queue<PreparedPulse>();

  void enqueueConfirmation(PreparedPulse prepared) {
    _confirmations.add(prepared);
  }

  @override
  Future<PulseRecovery> recoverPulse() async {
    _ensureOpen();
    return recovery;
  }

  @override
  Future<void> cancelPendingPulse(PreparedPulse action) async {
    _ensureOpen();
    cancelledActions.add(action);
    recovery = const NoPendingPulse();
  }

  List<PulsePoint> get historySnapshot =>
      List<PulsePoint>.unmodifiable(_history);

  void enqueuePrepareWarnings(List<PulseWarning> warnings) {
    _prepareWarnings.add(List<PulseWarning>.unmodifiable(warnings));
  }

  void enqueuePrepareError(ApiError error) {
    _prepareErrors.add(error);
  }

  void enqueueConfirmError(ApiError error, {bool saveBeforeThrowing = false}) {
    _confirmErrors.add(
      _PendingConfirmError(
        error: error,
        saveBeforeThrowing: saveBeforeThrowing,
      ),
    );
  }

  void enqueueHistoryError(ApiError error) {
    _historyErrors.add(error);
  }

  void enqueueZoneStatusError(ApiError error) {
    _zoneStatusErrors.add(error);
  }

  static List<ZoneState> _defaultZones(DateTime Function()? clock) {
    final now = (clock ?? () => DateTime.utc(2026, 11, 18, 12, 0))().toUtc();
    return BodyZone.values
        .map(
          (BodyZone zone) => ZoneState(
            zoneId: zone,
            status: EvaluationStatus.notEvaluated,
            symptoms: <SymptomEvaluation>[
              SymptomEvaluation(
                status: EvaluationStatus.notEvaluated,
                reason: NotEvaluatedReason.noData,
                evaluatedAtUtc: now,
                ruleId: 'symptom.no_data',
                ruleVersion: 'rules-2026.11.14',
                sourceKind: SourceKind.demoHeuristic,
                inputRecordIds: const <String>[],
                inputEventIds: const <String>[],
              ),
            ],
          ),
        )
        .toList(growable: false);
  }

  void _ensureOpen() {
    if (closed) {
      throw const ApiError(
        code: ApiErrorCodes.closed,
        message: 'The store session has been closed.',
      );
    }
  }

  Future<void> _waitIfNeeded() async {
    final delay = latency;
    if (delay != null) {
      await Future<void>.delayed(delay);
    }
  }

  @override
  Future<PreparedPulse> preparePulse(PulseInput input) async {
    _ensureOpen();
    preparedInputs.add(input);
    await _waitIfNeeded();

    if (_prepareErrors.isNotEmpty) {
      throw _prepareErrors.removeFirst();
    }

    final note = input.note;
    if (note != null && note.runes.length > 2000) {
      throw const ApiError(
        code: ApiErrorCodes.invalidValue,
        field: ApiFields.note,
        message: 'Text must contain at most 2000 characters',
      );
    }

    final measuredAt = (input.measuredAtUtc ?? _clock()).toUtc();
    final warnings = _prepareWarnings.isNotEmpty
        ? _prepareWarnings.removeFirst()
        : const <PulseWarning>[];

    final target = LocalPulseTarget(storeId: storeId, astronautId: astronautId);

    return PreparedPulse(
      key: input.key,
      bpm: input.bpm,
      measuredAtUtc: measuredAt,
      note: note,
      replaces: input.replaces,
      target: target,
      warnings: warnings,
      demoToReplace: demoTarget,
    );
  }

  @override
  Future<SaveOutcome> confirmPulse(PreparedPulse action) async {
    _ensureOpen();
    confirmedActions.add(action);
    await _waitIfNeeded();

    final gate = confirmGate;
    if (gate != null && !gate.isCompleted) {
      await gate.future;
    }

    final pendingError = _confirmErrors.isNotEmpty
        ? _confirmErrors.removeFirst()
        : null;
    if (pendingError != null && !pendingError.saveBeforeThrowing) {
      throw pendingError.error;
    }

    final fp = _fingerprint(action);
    final existing = _operationsByKey[action.key];
    if (existing != null) {
      if (existing.fingerprint != fp) {
        throw const ApiError(
          code: ApiErrorCodes.idempotencyConflict,
          field: ApiFields.key,
          message: 'The key already belongs to a different pulse',
        );
      }
      if (pendingError != null) {
        throw pendingError.error;
      }
      return Saved(existing.receipt);
    }

    if (_confirmations.isNotEmpty) {
      return NeedsConfirmation(_confirmations.removeFirst());
    }

    if (action.requiresConfirmation) {
      if (pendingError != null) {
        throw pendingError.error;
      }
      return NeedsConfirmation(action);
    }

    final now = _clock().toUtc();
    final isDemo = action.target is DemoPulseTarget;
    if (!isDemo && action.demoToReplace != null) {
      _history.removeWhere((PulsePoint p) => p.isDemo);
      demoTarget = null;
    }

    final recordId = 'rec-${_nextRecordSeq++}';
    final replaced = _history
        .where((point) => point.recordId == action.replaces)
        .firstOrNull;
    final rootId = replaced?.rootId ?? recordId;
    final receipt = PulseReceipt(
      key: action.key,
      recordId: recordId,
      rootId: rootId,
      replaces: action.replaces,
      bpm: action.bpm,
      source: RecordSource.form,
      measuredAtUtc: action.measuredAtUtc,
      loggedAtUtc: now,
      isDemo: isDemo,
      outboxState: isDemo
          ? PulseOutboxState.localOnly
          : PulseOutboxState.pending,
    );

    _history.add(
      PulsePoint(
        recordId: recordId,
        rootId: rootId,
        replaces: action.replaces,
        bpm: action.bpm,
        source: RecordSource.form,
        measuredAtUtc: action.measuredAtUtc,
        loggedAtUtc: now,
        isDemo: isDemo,
      ),
    );
    _operationsByKey[action.key] = _StoredOperation(
      fingerprint: fp,
      receipt: receipt,
    );

    if (pendingError != null) {
      throw pendingError.error;
    }
    return Saved(receipt);
  }

  @override
  Future<List<PulsePoint>> effectivePulseHistory([HistoryQuery? query]) async {
    _ensureOpen();
    final resolved = query ?? HistoryQuery();
    historyQueries.add(resolved);
    await _waitIfNeeded();

    if (_historyErrors.isNotEmpty) {
      throw _historyErrors.removeFirst();
    }

    final from = resolved.fromUtc;
    final to = resolved.toUtc;
    final limit = (resolved.limit ?? 100).clamp(1, 500);

    final replacedIds = _history
        .map((PulsePoint p) => p.replaces)
        .whereType<String>()
        .toSet();

    final filtered =
        _history
            .where((PulsePoint p) => !replacedIds.contains(p.recordId))
            .where((PulsePoint p) {
              if (from != null && p.measuredAtUtc.isBefore(from)) {
                return false;
              }
              if (to != null && p.measuredAtUtc.isAfter(to)) {
                return false;
              }
              return true;
            })
            .toList()
          ..sort((PulsePoint a, PulsePoint b) {
            final cmp = b.measuredAtUtc.compareTo(a.measuredAtUtc);
            if (cmp != 0) {
              return cmp;
            }
            return b.recordId.compareTo(a.recordId);
          });

    if (filtered.length > limit) {
      return List<PulsePoint>.unmodifiable(filtered.sublist(0, limit));
    }
    return List<PulsePoint>.unmodifiable(filtered);
  }

  @override
  Future<ZoneSnapshot> zoneStatus() async {
    _ensureOpen();
    zoneStatusCalls++;
    await _waitIfNeeded();

    if (_zoneStatusErrors.isNotEmpty) {
      throw _zoneStatusErrors.removeFirst();
    }

    final now = _clock().toUtc();
    final pulse = _pulseEvaluation ?? _noDataEvaluation(now);
    return ZoneSnapshot(
      evaluatedAtUtc: now,
      isDemo: demoTarget != null,
      zones: _zones,
      pulse: pulse,
    );
  }

  static PulseEvaluation _noDataEvaluation(DateTime now) => PulseEvaluation(
    status: EvaluationStatus.notEvaluated,
    reason: NotEvaluatedReason.noData,
    persistent: false,
    escalation: EscalationLevel.continueReading,
    evaluatedAtUtc: now,
    ruleId: 'fixture.pulse',
    ruleVersion: 'fixture-v1',
    sourceKind: SourceKind.demoHeuristic,
    inputRecordIds: const <String>[],
    historyCount: 0,
    historyNeeded: 0,
  );

  _PulseFingerprint _fingerprint(PreparedPulse action) => (
    action.target,
    action.replaces,
    action.bpm.value,
    action.note,
    action.measuredAtUtc,
  );

  @override
  Future<void> close() async {
    closed = true;
  }
}
