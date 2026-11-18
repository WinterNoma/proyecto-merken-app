import 'dart:math';

import 'package:flutter/foundation.dart';

import '../../platform/pulsys_api.dart';

enum PulseFormPhase { editing, confirming, submitting, saved, error }

enum BpmValidationError { required, invalid }

enum TimeValidationError { invalidFormat }

enum NoteValidationError { tooLong }

enum PulseSubmitError {
  idempotencyConflict,
  actionExpired,
  localServiceUnavailable,
  storageBusy,
  notFoundReplaces,
  genericSave,
}

final class PulseController extends ChangeNotifier {
  PulseController({required this.api, String Function()? keyGenerator})
    : _keyGenerator = keyGenerator ?? _generateUuidV4 {
    _intentKey = _keyGenerator();
  }

  static const int maxNoteLength = 2000;
  static const int defaultHistoryLimit = 50;

  final PulsysApi api;
  final String Function() _keyGenerator;

  String _bpmText = '';
  String _measuredAtText = '';
  DateTime? _preservedMeasuredAtUtc;
  String _noteText = '';
  late String _intentKey;
  bool _intentSubmitted = false;
  PreparedPulse? _pendingPrepared;
  String? _replaces;
  bool _reviewDestination = false;
  bool _recovering = false;
  ApiError? _recoveryError;

  PulseFormPhase _phase = PulseFormPhase.editing;
  BpmValidationError? _bpmError;
  TimeValidationError? _measuredAtError;
  NoteValidationError? _noteError;
  PulseSubmitError? _generalError;
  PulseReceipt? _lastReceipt;

  List<PulsePoint> _history = const <PulsePoint>[];
  bool _historyLoading = false;
  bool _historyLoadFailed = false;
  ZoneSnapshot? _zoneSnapshot;
  bool _zonesLoadFailed = false;
  bool _disposed = false;

  PulseFormPhase get phase => _phase;
  bool get isSubmitting => _phase == PulseFormPhase.submitting;
  BpmValidationError? get bpmError => _bpmError;
  TimeValidationError? get measuredAtError => _measuredAtError;
  NoteValidationError? get noteError => _noteError;
  PulseSubmitError? get generalError => _generalError;
  PulseReceipt? get lastReceipt => _lastReceipt;
  PreparedPulse? get pendingConfirmation =>
      _phase == PulseFormPhase.confirming ? _pendingPrepared : null;
  List<PulsePoint> get history => _history;
  bool get historyLoading => _historyLoading;
  bool get historyLoadFailed => _historyLoadFailed;
  ZoneSnapshot? get zoneSnapshot => _zoneSnapshot;
  bool get zonesLoadFailed => _zonesLoadFailed;
  String get currentIntentKey => _intentKey;
  String get bpmText => _bpmText;
  String get measuredAtText => _measuredAtText;
  String get noteText => _noteText;
  String? get replaces => _replaces;
  ApiError? get recoveryError => _recoveryError;
  bool get canEdit =>
      !isSubmitting &&
      !_recovering &&
      _recoveryError == null &&
      _phase != PulseFormPhase.confirming;
  bool get canSubmit => canEdit && _phase != PulseFormPhase.saved;
  bool get canStartNewReading => canEdit && _pendingPrepared == null;

  Future<void> loadInitial() async {
    await recoverPendingPulse();
    await refreshOverview();
  }

  Future<void> recoverPendingPulse() async {
    if (isSubmitting || _recovering) {
      return;
    }
    _recovering = true;
    _notifySafely();
    try {
      final recovery = await api.recoverPulse();
      _recoveryError = null;
      switch (recovery) {
        case NoPendingPulse():
          break;
        case RecoveredPulse(:final prepared):
          _adoptPrepared(prepared);
          _phase = PulseFormPhase.confirming;
        case RecoveredReceipt(:final receipt):
          _lastReceipt = receipt;
          _intentKey = receipt.key;
          _intentSubmitted = true;
          _phase = PulseFormPhase.saved;
        case PulseRecoveryBlocked(:final error):
          _recoveryError = error;
      }
    } on ApiError catch (error) {
      _recoveryError = error;
    } finally {
      _recovering = false;
      _notifySafely();
    }
  }

  Future<void> refreshOverview() async {
    await Future.wait<void>(<Future<void>>[refreshHistory(), refreshZones()]);
  }

  Future<void> refreshHistory() async {
    _historyLoading = true;
    _notifySafely();
    try {
      _history = await api.effectivePulseHistory(
        HistoryQuery(limit: defaultHistoryLimit),
      );
      _historyLoadFailed = false;
    } on ApiError {
      _historyLoadFailed = true;
    } finally {
      _historyLoading = false;
      _notifySafely();
    }
  }

  Future<void> refreshZones() async {
    try {
      _zoneSnapshot = await api.zoneStatus();
      _zonesLoadFailed = false;
    } on ApiError {
      _zonesLoadFailed = true;
    }
    _notifySafely();
  }

  void startNewReading() {
    if (!canStartNewReading) {
      return;
    }
    _startEditing();
  }

  void startCorrection(PulsePoint point) {
    if (!canEdit) {
      return;
    }
    _startEditing(correction: point);
  }

  void _startEditing({PulsePoint? correction}) {
    _intentKey = _keyGenerator();
    _intentSubmitted = false;
    _pendingPrepared = null;
    _replaces = correction?.recordId;
    _bpmText = correction?.bpm.format() ?? '';
    final measuredAtUtc = correction?.measuredAtUtc.toUtc();
    _preservedMeasuredAtUtc = measuredAtUtc;
    _measuredAtText = measuredAtUtc == null ? '' : _inputTime(measuredAtUtc);
    _noteText = '';
    _lastReceipt = null;
    _generalError = null;
    _bpmError = null;
    _measuredAtError = null;
    _noteError = null;
    _reviewDestination = false;
    _phase = PulseFormPhase.editing;
    _notifySafely();
  }

  void _adoptPrepared(PreparedPulse prepared) {
    _pendingPrepared = prepared;
    _intentKey = prepared.key;
    _intentSubmitted = true;
    _replaces = prepared.replaces;
    _bpmText = prepared.bpm.format();
    _preservedMeasuredAtUtc = prepared.measuredAtUtc;
    _measuredAtText = _inputTime(prepared.measuredAtUtc);
    _noteText = prepared.note ?? '';
  }

  static String _inputTime(DateTime value) =>
      value.toUtc().toIso8601String().substring(0, 16).replaceFirst('T', ' ');

  void updateBpm(String value) {
    if (!canEdit) {
      return;
    }
    if (_bpmText == value) {
      return;
    }
    _bpmText = value;
    _bpmError = null;
    _onContentEdited();
  }

  void updateMeasuredAt(String value) {
    if (!canEdit) {
      return;
    }
    if (_measuredAtText == value) {
      return;
    }
    _measuredAtText = value;
    _preservedMeasuredAtUtc = null;
    _measuredAtError = null;
    _onContentEdited();
  }

  void updateNote(String value) {
    if (!canEdit) {
      return;
    }
    if (_noteText == value) {
      return;
    }
    _noteText = value;
    _noteError = null;
    _onContentEdited();
  }

  void _onContentEdited() {
    _generalError = null;
    if (_intentSubmitted) {
      _intentKey = _keyGenerator();
      _intentSubmitted = false;
      _pendingPrepared = null;
    }
    if (_phase == PulseFormPhase.saved || _phase == PulseFormPhase.error) {
      _phase = PulseFormPhase.editing;
    }
    _notifySafely();
  }

  Future<void> submit() async {
    if (!canSubmit) {
      return;
    }
    _bpmError = null;
    _measuredAtError = null;
    _noteError = null;
    _generalError = null;
    _lastReceipt = null;

    PulseInput? input;
    if (_pendingPrepared == null) {
      final bpm = _validateBpm(_bpmText);
      final hasMeasuredAt = _measuredAtText.trim().isNotEmpty;
      final measuredAtUtc =
          _preservedMeasuredAtUtc ??
          (hasMeasuredAt ? _parseOptionalUtc(_measuredAtText) : null);
      if (hasMeasuredAt && measuredAtUtc == null) {
        _measuredAtError = TimeValidationError.invalidFormat;
      }
      final note = _validateNote(_noteText);
      if (_bpmError != null || _measuredAtError != null || _noteError != null) {
        _phase = PulseFormPhase.error;
        _notifySafely();
        return;
      }
      input = PulseInput(
        key: _intentKey,
        bpm: bpm!,
        measuredAtUtc: measuredAtUtc,
        note: note,
        replaces: _replaces,
      );
    }

    _phase = PulseFormPhase.submitting;
    _intentSubmitted = true;
    _notifySafely();
    try {
      final prepared = _pendingPrepared ?? await api.preparePulse(input!);
      _pendingPrepared = prepared;
      if (_reviewDestination) {
        _adoptPrepared(prepared);
        _phase = PulseFormPhase.confirming;
        _notifySafely();
        return;
      }
      await _handleOutcome(await api.confirmPulse(prepared));
    } on ApiError catch (error) {
      _handleApiError(error);
    }
  }

  Future<void> confirmPending() async {
    if (_phase != PulseFormPhase.confirming || _recoveryError != null) {
      return;
    }
    final pending = _pendingPrepared;
    if (pending == null) {
      return;
    }

    final approved = pending.approve();
    _pendingPrepared = approved;
    _generalError = null;
    _phase = PulseFormPhase.submitting;
    _notifySafely();

    try {
      final outcome = await api.confirmPulse(approved);
      await _handleOutcome(outcome);
    } on ApiError catch (error) {
      _handleApiError(error);
    }
  }

  Future<void> cancelConfirmation() async {
    final pending = pendingConfirmation;
    if (pending == null) {
      return;
    }
    _phase = PulseFormPhase.submitting;
    _notifySafely();
    try {
      await api.cancelPendingPulse(pending);
      _pendingPrepared = null;
      _intentKey = _keyGenerator();
      _intentSubmitted = false;
      _reviewDestination = false;
      _phase = PulseFormPhase.editing;
    } on ApiError catch (error) {
      _recoveryError = error;
      _phase = PulseFormPhase.error;
    }
    _notifySafely();
  }

  Future<void> _handleOutcome(SaveOutcome outcome) async {
    switch (outcome) {
      case NeedsConfirmation(:final prepared):
        _adoptPrepared(prepared);
        _phase = PulseFormPhase.confirming;
        _notifySafely();
      case Saved(:final receipt):
        _lastReceipt = receipt;
        _pendingPrepared = null;
        _phase = PulseFormPhase.saved;
        _reviewDestination = false;
        _intentSubmitted = true;
        _notifySafely();

        await Future.wait<void>(<Future<void>>[
          refreshHistory(),
          refreshZones(),
        ]);
        _notifySafely();
    }
  }

  void _handleApiError(ApiError error) {
    _phase = PulseFormPhase.error;
    if (error.code == ApiErrorCodes.actionExpired) {
      _pendingPrepared = null;
      _intentKey = _keyGenerator();
      _intentSubmitted = false;
      _reviewDestination = true;
    }
    switch (error.field) {
      case ApiFields.bpm:
        _pendingPrepared = null;
        _bpmError = BpmValidationError.invalid;
      case ApiFields.measuredAtMs:
        _pendingPrepared = null;
        _measuredAtError = TimeValidationError.invalidFormat;
      case ApiFields.note:
        _pendingPrepared = null;
        _noteError = NoteValidationError.tooLong;
      case ApiFields.replaces:
        _pendingPrepared = null;
        _generalError = PulseSubmitError.notFoundReplaces;
      default:
        _generalError = _mapGeneralError(error.code);
    }
    _notifySafely();
  }

  PulseSubmitError _mapGeneralError(String code) {
    return switch (code) {
      ApiErrorCodes.idempotencyConflict => PulseSubmitError.idempotencyConflict,
      ApiErrorCodes.actionExpired => PulseSubmitError.actionExpired,
      ApiErrorCodes.closed => PulseSubmitError.localServiceUnavailable,
      ApiErrorCodes.busy ||
      ApiErrorCodes.storageError ||
      ApiErrorCodes.deadlineExceeded => PulseSubmitError.storageBusy,
      ApiErrorCodes.notFound => PulseSubmitError.notFoundReplaces,
      _ => PulseSubmitError.genericSave,
    };
  }

  Bpm? _validateBpm(String raw) {
    final trimmed = raw.trim();
    if (trimmed.isEmpty) {
      _bpmError = BpmValidationError.required;
      return null;
    }
    final normalized = trimmed.contains(',') && !trimmed.contains('.')
        ? trimmed.replaceFirst(',', '.')
        : trimmed;
    final parsed = double.tryParse(normalized);
    final bpm = parsed == null ? null : Bpm.tryFrom(parsed);
    if (bpm == null) {
      _bpmError = BpmValidationError.invalid;
      return null;
    }
    return bpm;
  }

  String? _validateNote(String raw) {
    final trimmed = raw.trim();
    if (trimmed.isEmpty) {
      return null;
    }
    if (trimmed.runes.length > maxNoteLength) {
      _noteError = NoteValidationError.tooLong;
      return null;
    }
    return trimmed;
  }

  static DateTime? _parseOptionalUtc(String raw) {
    final trimmed = raw.trim();
    if (trimmed.isEmpty) {
      return null;
    }
    final match = RegExp(
      r'^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})(?::(\d{2}))?Z?$',
      caseSensitive: false,
    ).firstMatch(trimmed);
    if (match == null) {
      return null;
    }
    final year = int.parse(match.group(1)!);
    final month = int.parse(match.group(2)!);
    final day = int.parse(match.group(3)!);
    final hour = int.parse(match.group(4)!);
    final minute = int.parse(match.group(5)!);
    final second = match.group(6) != null ? int.parse(match.group(6)!) : 0;

    final candidate = DateTime.utc(year, month, day, hour, minute, second);
    if (candidate.year != year ||
        candidate.month != month ||
        candidate.day != day ||
        candidate.hour != hour ||
        candidate.minute != minute ||
        candidate.second != second) {
      return null;
    }
    return candidate;
  }

  static String _generateUuidV4() {
    final random = Random.secure();
    final bytes = List<int>.generate(16, (_) => random.nextInt(256));
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    String hex(int value) => value.toRadixString(16).padLeft(2, '0');
    final buffer = StringBuffer();
    for (var i = 0; i < 16; i++) {
      if (i == 4 || i == 6 || i == 8 || i == 10) {
        buffer.write('-');
      }
      buffer.write(hex(bytes[i]));
    }
    return buffer.toString();
  }

  void _notifySafely() {
    if (!_disposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
