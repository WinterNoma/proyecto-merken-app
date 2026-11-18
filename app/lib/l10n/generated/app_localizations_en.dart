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
  String get navHistory => 'History';

  @override
  String get navBody => 'Body';

  @override
  String get navToday => 'Today';

  @override
  String get navRecommendations => 'Recommendations';

  @override
  String get navNeedHelp => 'Need help';

  @override
  String get navHelpStage4 =>
      'Help request recording will be available in stage 4.';

  @override
  String get navLoadingStatus => 'Loading current status…';

  @override
  String get navRetryStatus => 'Status could not be refreshed. Retry';

  @override
  String navPulseStatus(String status) {
    return 'Pulse · $status';
  }

  @override
  String navZoneStatus(String zone, String status) {
    return '$zone · $status';
  }

  @override
  String get navPressure => 'Pressure';

  @override
  String get navOxygen => 'SpO₂';

  @override
  String get navMood => 'Mood';

  @override
  String get navTemperature => 'Temperature';

  @override
  String get navGlucose => 'Glucose';

  @override
  String get navSleep => 'Sleep';

  @override
  String get navNutrition => 'Nutrition';

  @override
  String get navRadiation => 'Radiation';

  @override
  String get navDistance => 'Distance';

  @override
  String get navSynchronization => 'Synchronization';

  @override
  String navContextValue(String name) {
    return '$name · No data';
  }

  @override
  String get navComingSoon => 'Coming soon';

  @override
  String get navHoldToTalk => 'Hold to talk';

  @override
  String get navAssistantHint => 'Tell me what happened…';

  @override
  String get navContactFlightSurgeons => 'Contact flight surgeons.';

  @override
  String get navGuidanceDisclaimer => 'This is guidance, not a diagnosis.';

  @override
  String get navSuggestionComingSoon => 'Suggested next step · Coming soon';

  @override
  String get historyRange6Hours => '6 h';

  @override
  String get historyRange24Hours => '24 h';

  @override
  String get historyRange7Days => '7 days';

  @override
  String get historyIndicators => 'Indicators';

  @override
  String get historyExplainComingSoon => 'Explain · Coming soon';

  @override
  String historyEscalation(String level) {
    return 'Escalation · $level';
  }

  @override
  String get historyContinueReading => 'Continue reading';

  @override
  String get historySelfCare => 'Self-care';

  @override
  String get historyFlagForGround => 'Flag for Ground';

  @override
  String get historyUrgent => 'Urgent';

  @override
  String get historySyncUnavailable => 'Synchronization · No data';

  @override
  String get navBodyRightMarker => 'R';

  @override
  String get navBodyLeftMarker => 'L';

  @override
  String get navBodyRightLabel => 'Astronaut’s right';

  @override
  String get navBodyLeftLabel => 'Astronaut’s left';

  @override
  String get historyTrendTitle => 'Pulse trend';

  @override
  String get historyDemoNotice => 'Includes demo readings.';

  @override
  String get historyUnavailable => 'Pulse history could not be loaded.';

  @override
  String historySampleSummary(int shown, int total) {
    final intl.NumberFormat shownNumberFormat =
        intl.NumberFormat.decimalPattern(localeName);
    final String shownString = shownNumberFormat.format(shown);
    final intl.NumberFormat totalNumberFormat =
        intl.NumberFormat.decimalPattern(localeName);
    final String totalString = totalNumberFormat.format(total);

    return 'Showing $shownString of the latest $totalString readings.';
  }

  @override
  String historyPlotRange(String minimum, String maximum) {
    return 'Scale $minimum–$maximum bpm';
  }

  @override
  String get historyTimeAxis => 'Time (UTC)';

  @override
  String get historyReadings => 'Readings';

  @override
  String get historyMeasuredColumn => 'Measured (UTC)';

  @override
  String get historyPulseColumn => 'Pulse';

  @override
  String historyMeasuredLabel(String time) {
    return 'Measured $time UTC';
  }

  @override
  String historyPulseLabel(String bpm) {
    return 'Pulse $bpm beats per minute';
  }

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

  @override
  String get bodyLegendTitle => 'Zone status';

  @override
  String get bodyTitle => 'Body zones';

  @override
  String get bodyFront => 'Front view';

  @override
  String get bodyList => 'Zone list';

  @override
  String get bodyShowList => 'Show zone list';

  @override
  String get bodyShowFigure => 'Show body figure';

  @override
  String get bodyDemo => 'DEMO';

  @override
  String get bodyLoading => 'Loading body zones…';

  @override
  String get bodyLoadError => 'Body zones could not be loaded.';

  @override
  String get bodyRetry => 'Try again';

  @override
  String get bodyClose => 'Close';

  @override
  String get bodyStable => 'Stable';

  @override
  String get bodyChange => 'Change';

  @override
  String get bodyAlarm => 'Alarm';

  @override
  String get bodyNotEvaluated => 'Not evaluated';

  @override
  String get bodyStatusUnavailable => 'Status unavailable';

  @override
  String get bodySnapshotTime => 'Snapshot time (UTC)';

  @override
  String get bodyEvaluation => 'Symptom evaluation';

  @override
  String get bodyReason => 'Reason';

  @override
  String get bodyNoData => 'No data';

  @override
  String get bodyStale => 'Stale reading';

  @override
  String get bodyTooFewData => 'Insufficient history';

  @override
  String get bodyNotApplicable => 'Context only';

  @override
  String get bodyMissing => 'Not provided';

  @override
  String get bodyEvaluationTime => 'Evaluation time (UTC)';

  @override
  String get bodySourceKind => 'Source type';

  @override
  String get bodySourceRef => 'Source reference';

  @override
  String get bodyPublished => 'Published source';

  @override
  String get bodyApprovedProtocol => 'Approved protocol';

  @override
  String get bodyDemoHeuristic => 'Demo rule, not medical advice.';

  @override
  String get bodyRuleId => 'Rule ID';

  @override
  String get bodyRuleVersion => 'Rule version';

  @override
  String get bodyNoEvidence =>
      'No symptom evaluation was provided for this zone.';

  @override
  String get bodyHead => 'Head';

  @override
  String get bodyNeck => 'Neck';

  @override
  String get bodyChest => 'Chest';

  @override
  String get bodyAbdomen => 'Abdomen';

  @override
  String get bodyRightArm => 'Right arm';

  @override
  String get bodyLeftArm => 'Left arm';

  @override
  String get bodyRightHand => 'Right hand';

  @override
  String get bodyLeftHand => 'Left hand';

  @override
  String get bodyRightLeg => 'Right leg';

  @override
  String get bodyLeftLeg => 'Left leg';

  @override
  String get bodyRightFoot => 'Right foot';

  @override
  String get bodyLeftFoot => 'Left foot';

  @override
  String get bodyBack => 'Back';

  @override
  String get bodyOther => 'Other';

  @override
  String get navEvaluation => 'Evaluation';

  @override
  String get navReadings => 'Readings';

  @override
  String navPreviousResult(String result, String time) {
    return '$result could not be refreshed. Previous data from $time.';
  }

  @override
  String navResultFailed(String result) {
    return '$result could not be loaded.';
  }

  @override
  String navResultLoading(String result) {
    return '$result is loading.';
  }

  @override
  String get navEvaluationNotForReading =>
      'Evaluation unavailable for this reading.';

  @override
  String get navReason => 'Reason and evaluation';

  @override
  String get navSources => 'Sources';

  @override
  String get navMeasurements => 'Measurements used';

  @override
  String navEvidenceRecords(String ids) {
    return 'Evaluation record IDs: $ids';
  }

  @override
  String get navLeisureComingSoon => 'Leisure preference · Coming soon';

  @override
  String get navExerciseComingSoon =>
      'Mission exercise guidelines · Coming soon';

  @override
  String get historyPersistentAlert => 'Persistent pulse change';

  @override
  String historyRangeEmpty(String range) {
    return 'No pulse readings in the selected range ($range).';
  }

  @override
  String get historyDemoUnavailable =>
      'DEMO scenarios are not available in this version.';

  @override
  String navEvaluatedRecord(String id) {
    return 'Evaluated record: $id';
  }

  @override
  String navEvidenceEvents(String ids) {
    return 'Evaluation event IDs: $ids';
  }

  @override
  String get navLatestPulse => 'Latest pulse reading';
}
