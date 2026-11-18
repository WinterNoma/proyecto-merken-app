import 'package:flutter/foundation.dart';

@immutable
final class Bpm {
  const Bpm._(this.value);

  factory Bpm(double value) {
    if (!value.isFinite || value <= 0) {
      throw ArgumentError.value(
        value,
        'value',
        'Bpm must be finite and greater than zero.',
      );
    }
    return Bpm._(value);
  }

  static const String unit = 'bpm';

  final double value;

  static Bpm? tryFrom(double value) {
    if (!value.isFinite || value <= 0) {
      return null;
    }
    return Bpm._(value);
  }

  String format() {
    final text = value.toString();
    return text.endsWith('.0') ? text.substring(0, text.length - 2) : text;
  }

  @override
  bool operator ==(Object other) => other is Bpm && other.value == value;

  @override
  int get hashCode => value.hashCode;

  @override
  String toString() => '${format()} $unit';
}

enum RecordSource { form, text, voice, earth, garmin }

enum PulseOutboxState { pending, localOnly }

enum EvaluationStatus { notEvaluated, stable, change, alarm }

enum NotEvaluatedReason { noData, tooFewData, stale, notApplicable }

enum TrendDirection { up, down }

enum EscalationLevel { continueReading, selfCare, flagForGround, urgent }

enum SourceKind { published, approvedProtocol, demoHeuristic }

enum BodyZone {
  head,
  neck,
  chest,
  abdomen,
  back,
  leftArm,
  rightArm,
  leftHand,
  rightHand,
  leftLeg,
  rightLeg,
  leftFoot,
  rightFoot,
  other,
}

@immutable
final class DemoTarget {
  const DemoTarget({this.astronautId, required this.generation});

  final String? astronautId;
  final String generation;

  @override
  bool operator ==(Object other) =>
      other is DemoTarget &&
      other.astronautId == astronautId &&
      other.generation == generation;

  @override
  int get hashCode => Object.hash(astronautId, generation);
}

@immutable
sealed class PulseTarget {
  const PulseTarget();
}

final class LocalPulseTarget extends PulseTarget {
  const LocalPulseTarget({required this.storeId, this.astronautId});

  final String storeId;
  final String? astronautId;

  @override
  bool operator ==(Object other) =>
      other is LocalPulseTarget &&
      other.storeId == storeId &&
      other.astronautId == astronautId;

  @override
  int get hashCode => Object.hash(storeId, astronautId);
}

final class DemoPulseTarget extends PulseTarget {
  const DemoPulseTarget({required this.demo});

  final DemoTarget demo;

  @override
  bool operator ==(Object other) =>
      other is DemoPulseTarget && other.demo == demo;

  @override
  int get hashCode => demo.hashCode;
}

@immutable
final class PulseWarning {
  const PulseWarning({required this.code, required this.ruleId});

  static const String unusualValue = 'unusual_value';
  static const String futureTimestamp = 'future_timestamp';
  static const String oldMeasurement = 'old_measurement';

  final String code;
  final String ruleId;

  @override
  bool operator ==(Object other) =>
      other is PulseWarning && other.code == code && other.ruleId == ruleId;

  @override
  int get hashCode => Object.hash(code, ruleId);
}

@immutable
final class PulseInput {
  PulseInput({
    required this.key,
    required this.bpm,
    DateTime? measuredAtUtc,
    this.note,
    this.replaces,
  }) : measuredAtUtc = measuredAtUtc?.toUtc();

  final String key;
  final Bpm bpm;
  final DateTime? measuredAtUtc;
  final String? note;
  final String? replaces;

  @override
  bool operator ==(Object other) =>
      other is PulseInput &&
      other.key == key &&
      other.bpm == bpm &&
      other.measuredAtUtc == measuredAtUtc &&
      other.note == note &&
      other.replaces == replaces;

  @override
  int get hashCode => Object.hash(key, bpm, measuredAtUtc, note, replaces);
}

enum PulseConfirmation { notRequired, requiredReview, approved }

@immutable
final class PreparedPulse {
  PreparedPulse({
    required this.key,
    required this.bpm,
    required DateTime measuredAtUtc,
    this.note,
    this.replaces,
    required this.target,
    required List<PulseWarning> warnings,
    this.demoToReplace,
    bool approved = false,
    PulseConfirmation? confirmation,
  }) : confirmation =
           confirmation ??
           (approved
               ? PulseConfirmation.approved
               : warnings.isNotEmpty || demoToReplace != null
               ? PulseConfirmation.requiredReview
               : PulseConfirmation.notRequired),
       measuredAtUtc = measuredAtUtc.toUtc(),
       warnings = List<PulseWarning>.unmodifiable(warnings);

  final String key;
  final Bpm bpm;
  final DateTime measuredAtUtc;
  final String? note;
  final String? replaces;
  final PulseTarget target;
  final List<PulseWarning> warnings;
  final DemoTarget? demoToReplace;
  final PulseConfirmation confirmation;

  bool get approved => confirmation == PulseConfirmation.approved;

  bool get requiresConfirmation =>
      confirmation == PulseConfirmation.requiredReview;

  PreparedPulse approve() => PreparedPulse(
    key: key,
    bpm: bpm,
    measuredAtUtc: measuredAtUtc,
    note: note,
    replaces: replaces,
    target: target,
    warnings: warnings,
    demoToReplace: demoToReplace,
    approved: true,
  );
}

@immutable
sealed class SaveOutcome {
  const SaveOutcome();
}

final class NeedsConfirmation extends SaveOutcome {
  const NeedsConfirmation(this.prepared);

  final PreparedPulse prepared;
}

final class Saved extends SaveOutcome {
  const Saved(this.receipt);

  final PulseReceipt receipt;
}

@immutable
final class PulseReceipt {
  PulseReceipt({
    required this.key,
    required this.recordId,
    required this.rootId,
    this.replaces,
    required this.bpm,
    required this.source,
    required DateTime measuredAtUtc,
    required DateTime loggedAtUtc,
    required this.isDemo,
    required this.outboxState,
  }) : measuredAtUtc = measuredAtUtc.toUtc(),
       loggedAtUtc = loggedAtUtc.toUtc();

  final String key;
  final String recordId;
  final String rootId;
  final String? replaces;
  final Bpm bpm;
  final RecordSource source;
  final DateTime measuredAtUtc;
  final DateTime loggedAtUtc;
  final bool isDemo;
  final PulseOutboxState outboxState;

  String get unit => Bpm.unit;
}

@immutable
final class HistoryQuery {
  HistoryQuery({DateTime? fromUtc, DateTime? toUtc, this.limit})
    : fromUtc = fromUtc?.toUtc(),
      toUtc = toUtc?.toUtc();

  final DateTime? fromUtc;
  final DateTime? toUtc;
  final int? limit;
}

@immutable
final class PulsePoint {
  PulsePoint({
    required this.recordId,
    required this.rootId,
    this.replaces,
    required this.bpm,
    required this.source,
    required DateTime measuredAtUtc,
    required DateTime loggedAtUtc,
    required this.isDemo,
  }) : measuredAtUtc = measuredAtUtc.toUtc(),
       loggedAtUtc = loggedAtUtc.toUtc();

  final String recordId;
  final String rootId;
  final String? replaces;
  final Bpm bpm;
  final RecordSource source;
  final DateTime measuredAtUtc;
  final DateTime loggedAtUtc;
  final bool isDemo;

  String get unit => Bpm.unit;
}

@immutable
final class SymptomEvaluation {
  SymptomEvaluation({
    required this.status,
    this.reason,
    required DateTime evaluatedAtUtc,
    required this.ruleId,
    required this.ruleVersion,
    required this.sourceKind,
    this.sourceRef,
    required List<String> inputRecordIds,
    required List<String> inputEventIds,
  }) : evaluatedAtUtc = evaluatedAtUtc.toUtc(),
       inputRecordIds = List<String>.unmodifiable(inputRecordIds),
       inputEventIds = List<String>.unmodifiable(inputEventIds);

  final EvaluationStatus status;
  final NotEvaluatedReason? reason;
  final DateTime evaluatedAtUtc;
  final String ruleId;
  final String ruleVersion;
  final SourceKind sourceKind;
  final String? sourceRef;
  final List<String> inputRecordIds;
  final List<String> inputEventIds;
}

@immutable
final class ZoneState {
  ZoneState({
    required this.zoneId,
    required this.status,
    required List<SymptomEvaluation> symptoms,
  }) : symptoms = List<SymptomEvaluation>.unmodifiable(symptoms);

  final BodyZone zoneId;
  final EvaluationStatus status;
  final List<SymptomEvaluation> symptoms;
}

@immutable
final class PulseEvaluation {
  PulseEvaluation({
    required this.status,
    this.reason,
    this.direction,
    required this.persistent,
    required this.escalation,
    required DateTime evaluatedAtUtc,
    DateTime? latestMeasuredAtUtc,
    required this.ruleId,
    required this.ruleVersion,
    required this.sourceKind,
    this.sourceRef,
    required List<String> inputRecordIds,
    required this.historyCount,
    required this.historyNeeded,
    this.baselineBpm,
  }) : evaluatedAtUtc = evaluatedAtUtc.toUtc(),
       latestMeasuredAtUtc = latestMeasuredAtUtc?.toUtc(),
       inputRecordIds = List<String>.unmodifiable(inputRecordIds);

  final EvaluationStatus status;
  final NotEvaluatedReason? reason;
  final TrendDirection? direction;
  final bool persistent;
  final EscalationLevel escalation;
  final DateTime evaluatedAtUtc;
  final DateTime? latestMeasuredAtUtc;
  final String ruleId;
  final String ruleVersion;
  final SourceKind sourceKind;
  final String? sourceRef;
  final List<String> inputRecordIds;
  final int historyCount;
  final int historyNeeded;
  final Bpm? baselineBpm;

  // Rust orders the evaluated record before the antecedents in this evidence.
  String? get evaluatedRecordId => inputRecordIds.firstOrNull;
}

@immutable
final class ZoneSnapshot {
  ZoneSnapshot({
    required DateTime evaluatedAtUtc,
    required this.isDemo,
    required this.isStoreEmpty,
    required List<ZoneState> zones,
    required this.pulse,
  }) : evaluatedAtUtc = evaluatedAtUtc.toUtc(),
       zones = List<ZoneState>.unmodifiable(zones);

  final DateTime evaluatedAtUtc;
  final bool isDemo;
  final bool isStoreEmpty;
  final List<ZoneState> zones;
  final PulseEvaluation pulse;
}

abstract final class ApiErrorCodes {
  static const String invalidValue = 'invalid_value';
  static const String notFound = 'not_found';
  static const String idempotencyConflict = 'idempotency_conflict';
  static const String actionExpired = 'action_expired';
  static const String invalidSchema = 'invalid_schema';
  static const String unsupportedActionVersion = 'unsupported_action_version';
  static const String storageError = 'storage_error';
  static const String closed = 'closed';
  static const String busy = 'busy';
  static const String cancelled = 'cancelled';
  static const String deadlineExceeded = 'deadline_exceeded';
}

abstract final class ApiFields {
  static const String bpm = 'value.bpm';
  static const String measuredAtMs = 'measured_at_ms';
  static const String note = 'note';
  static const String replaces = 'replaces';
  static const String key = 'key';
}

@immutable
final class ApiError implements Exception {
  const ApiError({required this.code, this.field, required this.message});

  final String code;
  final String? field;
  final String message;

  @override
  String toString() =>
      'ApiError($code${field != null ? ', $field' : ''}): $message';
}

sealed class PulseRecovery {
  const PulseRecovery();
}

final class NoPendingPulse extends PulseRecovery {
  const NoPendingPulse();
}

final class RecoveredPulse extends PulseRecovery {
  const RecoveredPulse(this.prepared);

  final PreparedPulse prepared;
}

final class RecoveredReceipt extends PulseRecovery {
  const RecoveredReceipt(this.receipt);

  final PulseReceipt receipt;
}

final class PulseRecoveryBlocked extends PulseRecovery {
  const PulseRecoveryBlocked(this.error);

  final ApiError error;
}

abstract interface class PulsysApi {
  Future<PulseRecovery> recoverPulse();

  Future<void> cancelPendingPulse(PreparedPulse action);

  Future<PreparedPulse> preparePulse(PulseInput input);

  Future<SaveOutcome> confirmPulse(PreparedPulse action);

  Future<List<PulsePoint>> effectivePulseHistory([HistoryQuery? query]);

  Future<ZoneSnapshot> zoneStatus();

  Future<void> close();
}
