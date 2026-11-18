// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appTitle => 'Pulsys';

  @override
  String get wordmark => 'PULSYS';

  @override
  String get pulseScreenTitle => 'Pulse';

  @override
  String get pulseSectionSubtitle =>
      'Record a heart rate reading in bpm. Saved on this device.';

  @override
  String get pulseBpmLabel => 'Pulse (bpm)';

  @override
  String get pulseBpmHint => 'e.g. 72';

  @override
  String get pulseBpmUnit => 'bpm';

  @override
  String get pulseTimeLabel => 'Measurement time (UTC, optional)';

  @override
  String get pulseTimeHint => 'YYYY-MM-DD HH:MM';

  @override
  String get pulseTimeHelper => 'Leave blank to use the current UTC time.';

  @override
  String get pulseNoteLabel => 'Note (optional)';

  @override
  String get pulseNoteHint => 'Add context such as rest or post-exercise.';

  @override
  String get pulseSaveAction => 'Save reading';

  @override
  String get pulseNewReadingAction => 'New reading';

  @override
  String get pulseSavingAction => 'Saving…';

  @override
  String get pulseSaveRealDataAction => 'Save real data';

  @override
  String get pulseSavedLocallyHeading => 'Saved locally';

  @override
  String pulseSavedReceipt(String bpm, String measuredAtUtc) {
    return '$bpm bpm · Measured $measuredAtUtc';
  }

  @override
  String get pulseSyncPending =>
      'Synchronization pending. Ground receipt is not confirmed.';

  @override
  String get pulseSyncLocalOnly => 'Saved on this device only.';

  @override
  String get pulseHistoryHeading => 'Recorded pulse history';

  @override
  String get pulseHistoryLoading => 'Loading pulse history…';

  @override
  String get pulseHistoryUnavailable => 'History unavailable. Try again.';

  @override
  String get pulseHistoryRetry => 'Retry';

  @override
  String get pulseHistoryEmptyTitle => 'No data';

  @override
  String get pulseHistoryEmptyBody => 'No pulse readings recorded yet.';

  @override
  String get pulseCorrectedLabel => 'Corrected';

  @override
  String get pulseDemoTag => 'DEMO';

  @override
  String get pulseSourceForm => 'Form';

  @override
  String get pulseSourceText => 'Text';

  @override
  String get pulseSourceVoice => 'Voice';

  @override
  String get pulseSourceEarth => 'Ground';

  @override
  String get pulseSourceGarmin => 'Garmin';

  @override
  String get pulseStatusHeading => 'Current pulse status';

  @override
  String get statusStable => 'Stable';

  @override
  String get statusChange => 'Change detected';

  @override
  String get statusAlarm => 'Alarm';

  @override
  String get statusNotEvaluated => 'Not evaluated';

  @override
  String get statusNoData => 'No data';

  @override
  String get statusStale => 'Stale reading';

  @override
  String statusInsufficientHistory(int count, int needed) {
    return 'Insufficient history ($count/$needed)';
  }

  @override
  String get statusContextOnly => 'Context only';

  @override
  String get statusIncreasing => 'Increasing';

  @override
  String get statusDecreasing => 'Decreasing';

  @override
  String get statusPersistent => 'Persistent';

  @override
  String zonesSummaryLabel(int count) {
    return 'Body zones evaluated: $count';
  }

  @override
  String get confirmDialogTitle => 'Confirm pulse reading?';

  @override
  String get confirmDialogDemoTitle => 'Demo data will be removed';

  @override
  String get confirmDialogDemoBody =>
      'Saving real data will remove the demo data.';

  @override
  String get confirmDialogWarningsIntro =>
      'Review these warnings before saving:';

  @override
  String get confirmDialogLocalNote =>
      'This reading will be saved locally on this device.';

  @override
  String get confirmDialogCancel => 'Cancel';

  @override
  String get confirmDialogConfirm => 'Confirm and save';

  @override
  String get warningUnusualValue =>
      'This pulse value is outside the typical range.';

  @override
  String get warningFutureTimestamp => 'The measurement time is in the future.';

  @override
  String get warningOldMeasurement =>
      'The measurement time is more than seven days old.';

  @override
  String warningFallback(String code) {
    return 'Review this reading before saving ($code).';
  }

  @override
  String get errorBpmRequired => 'Enter a pulse in bpm.';

  @override
  String get errorBpmInvalid => 'Enter a positive number in bpm.';

  @override
  String get errorTimeInvalid => 'Enter a valid UTC time (YYYY-MM-DD HH:MM).';

  @override
  String get errorNoteTooLong => 'Note must contain at most 2,000 characters.';

  @override
  String get errorIdempotencyConflict =>
      'This submission conflicts with a previously saved reading. Edit a field and try again.';

  @override
  String get errorActionExpired =>
      'The target profile changed before saving. Review and try again.';

  @override
  String get errorLocalServiceUnavailable =>
      'The local service could not be reached. This action was not confirmed.';

  @override
  String get errorStorageBusy =>
      'The local store is busy. Try again; your entry has been kept.';

  @override
  String get errorNotFoundReplaces =>
      'The original reading to correct was not found.';

  @override
  String get errorGenericSave =>
      'The reading could not be saved. Your entry has been kept.';

  @override
  String get semanticsBpmField => 'Pulse reading in beats per minute';

  @override
  String get semanticsTimeField => 'Optional measurement time in UTC';

  @override
  String get semanticsNoteField => 'Optional note for the pulse reading';

  @override
  String get semanticsSaveButton => 'Save pulse reading';

  @override
  String get startupStorageError => 'Pulsys could not open local storage.';

  @override
  String get pulseStatusLastAvailable => 'Last available pulse status';

  @override
  String get pulseStatusUnavailable => 'Pulse status could not be refreshed.';

  @override
  String get pulseStatusRetry => 'Retry pulse status';

  @override
  String get pulseHistoryLastAvailable => 'Last available pulse history';

  @override
  String get pulseRecoveryNewerVersion =>
      'A pending reading requires a newer app version. It has been kept on this device.';

  @override
  String get pulseRecoveryUnavailable =>
      'A pending reading could not be recovered. It has been kept on this device.';

  @override
  String get pulseRecoveryRetry => 'Retry recovery';

  @override
  String get pulseCorrectionHeading => 'Correcting a previous reading';

  @override
  String get pulseCorrectAction => 'Correct reading';

  @override
  String pulseDestinationLabel(String target) {
    return 'Save for $target';
  }

  @override
  String get pulseLocalProfile => 'Local profile';
}
