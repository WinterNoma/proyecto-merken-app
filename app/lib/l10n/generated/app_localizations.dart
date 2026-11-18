import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'generated/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[Locale('en')];

  /// No description provided for @appTitle.
  ///
  /// In en, this message translates to:
  /// **'Pulsys'**
  String get appTitle;

  /// No description provided for @wordmark.
  ///
  /// In en, this message translates to:
  /// **'PULSYS'**
  String get wordmark;

  /// No description provided for @navHistory.
  ///
  /// In en, this message translates to:
  /// **'History'**
  String get navHistory;

  /// No description provided for @navBody.
  ///
  /// In en, this message translates to:
  /// **'Body'**
  String get navBody;

  /// No description provided for @navToday.
  ///
  /// In en, this message translates to:
  /// **'Today'**
  String get navToday;

  /// No description provided for @navRecommendations.
  ///
  /// In en, this message translates to:
  /// **'Recommendations'**
  String get navRecommendations;

  /// No description provided for @navNeedHelp.
  ///
  /// In en, this message translates to:
  /// **'Need help'**
  String get navNeedHelp;

  /// No description provided for @navHelpStage4.
  ///
  /// In en, this message translates to:
  /// **'Help request recording will be available in stage 4.'**
  String get navHelpStage4;

  /// No description provided for @navLoadingStatus.
  ///
  /// In en, this message translates to:
  /// **'Loading current status…'**
  String get navLoadingStatus;

  /// No description provided for @navRetryStatus.
  ///
  /// In en, this message translates to:
  /// **'Status could not be refreshed. Retry'**
  String get navRetryStatus;

  /// No description provided for @navPulseStatus.
  ///
  /// In en, this message translates to:
  /// **'Pulse · {status}'**
  String navPulseStatus(String status);

  /// No description provided for @navZoneStatus.
  ///
  /// In en, this message translates to:
  /// **'{zone} · {status}'**
  String navZoneStatus(String zone, String status);

  /// No description provided for @navPressure.
  ///
  /// In en, this message translates to:
  /// **'Pressure'**
  String get navPressure;

  /// No description provided for @navOxygen.
  ///
  /// In en, this message translates to:
  /// **'SpO₂'**
  String get navOxygen;

  /// No description provided for @navMood.
  ///
  /// In en, this message translates to:
  /// **'Mood'**
  String get navMood;

  /// No description provided for @navTemperature.
  ///
  /// In en, this message translates to:
  /// **'Temperature'**
  String get navTemperature;

  /// No description provided for @navGlucose.
  ///
  /// In en, this message translates to:
  /// **'Glucose'**
  String get navGlucose;

  /// No description provided for @navSleep.
  ///
  /// In en, this message translates to:
  /// **'Sleep'**
  String get navSleep;

  /// No description provided for @navNutrition.
  ///
  /// In en, this message translates to:
  /// **'Nutrition'**
  String get navNutrition;

  /// No description provided for @navRadiation.
  ///
  /// In en, this message translates to:
  /// **'Radiation'**
  String get navRadiation;

  /// No description provided for @navDistance.
  ///
  /// In en, this message translates to:
  /// **'Distance'**
  String get navDistance;

  /// No description provided for @navSynchronization.
  ///
  /// In en, this message translates to:
  /// **'Synchronization'**
  String get navSynchronization;

  /// No description provided for @navContextValue.
  ///
  /// In en, this message translates to:
  /// **'{name} · No data'**
  String navContextValue(String name);

  /// No description provided for @navComingSoon.
  ///
  /// In en, this message translates to:
  /// **'Coming soon'**
  String get navComingSoon;

  /// No description provided for @navHoldToTalk.
  ///
  /// In en, this message translates to:
  /// **'Hold to talk'**
  String get navHoldToTalk;

  /// No description provided for @navAssistantHint.
  ///
  /// In en, this message translates to:
  /// **'Tell me what happened…'**
  String get navAssistantHint;

  /// No description provided for @navContactFlightSurgeons.
  ///
  /// In en, this message translates to:
  /// **'Contact flight surgeons.'**
  String get navContactFlightSurgeons;

  /// No description provided for @navGuidanceDisclaimer.
  ///
  /// In en, this message translates to:
  /// **'This is guidance, not a diagnosis.'**
  String get navGuidanceDisclaimer;

  /// No description provided for @navSuggestionComingSoon.
  ///
  /// In en, this message translates to:
  /// **'Suggested next step · Coming soon'**
  String get navSuggestionComingSoon;

  /// No description provided for @historyRange6Hours.
  ///
  /// In en, this message translates to:
  /// **'6 h'**
  String get historyRange6Hours;

  /// No description provided for @historyRange24Hours.
  ///
  /// In en, this message translates to:
  /// **'24 h'**
  String get historyRange24Hours;

  /// No description provided for @historyRange7Days.
  ///
  /// In en, this message translates to:
  /// **'7 days'**
  String get historyRange7Days;

  /// No description provided for @historyIndicators.
  ///
  /// In en, this message translates to:
  /// **'Indicators'**
  String get historyIndicators;

  /// No description provided for @historyExplainComingSoon.
  ///
  /// In en, this message translates to:
  /// **'Explain · Coming soon'**
  String get historyExplainComingSoon;

  /// No description provided for @historyEscalation.
  ///
  /// In en, this message translates to:
  /// **'Escalation · {level}'**
  String historyEscalation(String level);

  /// No description provided for @historyContinueReading.
  ///
  /// In en, this message translates to:
  /// **'Continue reading'**
  String get historyContinueReading;

  /// No description provided for @historySelfCare.
  ///
  /// In en, this message translates to:
  /// **'Self-care'**
  String get historySelfCare;

  /// No description provided for @historyFlagForGround.
  ///
  /// In en, this message translates to:
  /// **'Flag for Ground'**
  String get historyFlagForGround;

  /// No description provided for @historyUrgent.
  ///
  /// In en, this message translates to:
  /// **'Urgent'**
  String get historyUrgent;

  /// No description provided for @historySyncUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Synchronization · No data'**
  String get historySyncUnavailable;

  /// No description provided for @navBodyRightMarker.
  ///
  /// In en, this message translates to:
  /// **'R'**
  String get navBodyRightMarker;

  /// No description provided for @navBodyLeftMarker.
  ///
  /// In en, this message translates to:
  /// **'L'**
  String get navBodyLeftMarker;

  /// No description provided for @navBodyRightLabel.
  ///
  /// In en, this message translates to:
  /// **'Astronaut’s right'**
  String get navBodyRightLabel;

  /// No description provided for @navBodyLeftLabel.
  ///
  /// In en, this message translates to:
  /// **'Astronaut’s left'**
  String get navBodyLeftLabel;

  /// No description provided for @historyTrendTitle.
  ///
  /// In en, this message translates to:
  /// **'Pulse trend'**
  String get historyTrendTitle;

  /// No description provided for @historyDemoNotice.
  ///
  /// In en, this message translates to:
  /// **'Includes demo readings.'**
  String get historyDemoNotice;

  /// No description provided for @historyUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Pulse history could not be loaded.'**
  String get historyUnavailable;

  /// No description provided for @historySampleSummary.
  ///
  /// In en, this message translates to:
  /// **'Showing {shown} of the latest {total} readings.'**
  String historySampleSummary(int shown, int total);

  /// No description provided for @historyPlotRange.
  ///
  /// In en, this message translates to:
  /// **'Scale {minimum}–{maximum} bpm'**
  String historyPlotRange(String minimum, String maximum);

  /// No description provided for @historyTimeAxis.
  ///
  /// In en, this message translates to:
  /// **'Time (UTC)'**
  String get historyTimeAxis;

  /// No description provided for @historyReadings.
  ///
  /// In en, this message translates to:
  /// **'Readings'**
  String get historyReadings;

  /// No description provided for @historyMeasuredColumn.
  ///
  /// In en, this message translates to:
  /// **'Measured (UTC)'**
  String get historyMeasuredColumn;

  /// No description provided for @historyPulseColumn.
  ///
  /// In en, this message translates to:
  /// **'Pulse'**
  String get historyPulseColumn;

  /// No description provided for @historyMeasuredLabel.
  ///
  /// In en, this message translates to:
  /// **'Measured {time} UTC'**
  String historyMeasuredLabel(String time);

  /// No description provided for @historyPulseLabel.
  ///
  /// In en, this message translates to:
  /// **'Pulse {bpm} beats per minute'**
  String historyPulseLabel(String bpm);

  /// No description provided for @pulseScreenTitle.
  ///
  /// In en, this message translates to:
  /// **'Pulse'**
  String get pulseScreenTitle;

  /// No description provided for @pulseSectionSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Record a heart rate reading in bpm. Saved on this device.'**
  String get pulseSectionSubtitle;

  /// No description provided for @pulseBpmLabel.
  ///
  /// In en, this message translates to:
  /// **'Pulse (bpm)'**
  String get pulseBpmLabel;

  /// No description provided for @pulseBpmHint.
  ///
  /// In en, this message translates to:
  /// **'e.g. 72'**
  String get pulseBpmHint;

  /// No description provided for @pulseBpmUnit.
  ///
  /// In en, this message translates to:
  /// **'bpm'**
  String get pulseBpmUnit;

  /// No description provided for @pulseTimeLabel.
  ///
  /// In en, this message translates to:
  /// **'Measurement time (UTC, optional)'**
  String get pulseTimeLabel;

  /// No description provided for @pulseTimeHint.
  ///
  /// In en, this message translates to:
  /// **'YYYY-MM-DD HH:MM'**
  String get pulseTimeHint;

  /// No description provided for @pulseTimeHelper.
  ///
  /// In en, this message translates to:
  /// **'Leave blank to use the current UTC time.'**
  String get pulseTimeHelper;

  /// No description provided for @pulseNoteLabel.
  ///
  /// In en, this message translates to:
  /// **'Note (optional)'**
  String get pulseNoteLabel;

  /// No description provided for @pulseNoteHint.
  ///
  /// In en, this message translates to:
  /// **'Add context such as rest or post-exercise.'**
  String get pulseNoteHint;

  /// No description provided for @pulseSaveAction.
  ///
  /// In en, this message translates to:
  /// **'Save reading'**
  String get pulseSaveAction;

  /// No description provided for @pulseNewReadingAction.
  ///
  /// In en, this message translates to:
  /// **'New reading'**
  String get pulseNewReadingAction;

  /// No description provided for @pulseSavingAction.
  ///
  /// In en, this message translates to:
  /// **'Saving…'**
  String get pulseSavingAction;

  /// No description provided for @pulseSaveRealDataAction.
  ///
  /// In en, this message translates to:
  /// **'Save real data'**
  String get pulseSaveRealDataAction;

  /// No description provided for @pulseSavedLocallyHeading.
  ///
  /// In en, this message translates to:
  /// **'Saved locally'**
  String get pulseSavedLocallyHeading;

  /// No description provided for @pulseSavedReceipt.
  ///
  /// In en, this message translates to:
  /// **'{bpm} bpm · Measured {measuredAtUtc}'**
  String pulseSavedReceipt(String bpm, String measuredAtUtc);

  /// No description provided for @pulseSyncPending.
  ///
  /// In en, this message translates to:
  /// **'Synchronization pending. Ground receipt is not confirmed.'**
  String get pulseSyncPending;

  /// No description provided for @pulseSyncLocalOnly.
  ///
  /// In en, this message translates to:
  /// **'Saved on this device only.'**
  String get pulseSyncLocalOnly;

  /// No description provided for @pulseHistoryHeading.
  ///
  /// In en, this message translates to:
  /// **'Recorded pulse history'**
  String get pulseHistoryHeading;

  /// No description provided for @pulseHistoryLoading.
  ///
  /// In en, this message translates to:
  /// **'Loading pulse history…'**
  String get pulseHistoryLoading;

  /// No description provided for @pulseHistoryUnavailable.
  ///
  /// In en, this message translates to:
  /// **'History unavailable. Try again.'**
  String get pulseHistoryUnavailable;

  /// No description provided for @pulseHistoryRetry.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get pulseHistoryRetry;

  /// No description provided for @pulseHistoryEmptyTitle.
  ///
  /// In en, this message translates to:
  /// **'No data'**
  String get pulseHistoryEmptyTitle;

  /// No description provided for @pulseHistoryEmptyBody.
  ///
  /// In en, this message translates to:
  /// **'No pulse readings recorded yet.'**
  String get pulseHistoryEmptyBody;

  /// No description provided for @pulseCorrectedLabel.
  ///
  /// In en, this message translates to:
  /// **'Corrected'**
  String get pulseCorrectedLabel;

  /// No description provided for @pulseDemoTag.
  ///
  /// In en, this message translates to:
  /// **'DEMO'**
  String get pulseDemoTag;

  /// No description provided for @pulseSourceForm.
  ///
  /// In en, this message translates to:
  /// **'Form'**
  String get pulseSourceForm;

  /// No description provided for @pulseSourceText.
  ///
  /// In en, this message translates to:
  /// **'Text'**
  String get pulseSourceText;

  /// No description provided for @pulseSourceVoice.
  ///
  /// In en, this message translates to:
  /// **'Voice'**
  String get pulseSourceVoice;

  /// No description provided for @pulseSourceEarth.
  ///
  /// In en, this message translates to:
  /// **'Ground'**
  String get pulseSourceEarth;

  /// No description provided for @pulseSourceGarmin.
  ///
  /// In en, this message translates to:
  /// **'Garmin'**
  String get pulseSourceGarmin;

  /// No description provided for @pulseStatusHeading.
  ///
  /// In en, this message translates to:
  /// **'Current pulse status'**
  String get pulseStatusHeading;

  /// No description provided for @statusStable.
  ///
  /// In en, this message translates to:
  /// **'Stable'**
  String get statusStable;

  /// No description provided for @statusChange.
  ///
  /// In en, this message translates to:
  /// **'Change detected'**
  String get statusChange;

  /// No description provided for @statusAlarm.
  ///
  /// In en, this message translates to:
  /// **'Alarm'**
  String get statusAlarm;

  /// No description provided for @statusNotEvaluated.
  ///
  /// In en, this message translates to:
  /// **'Not evaluated'**
  String get statusNotEvaluated;

  /// No description provided for @statusNoData.
  ///
  /// In en, this message translates to:
  /// **'No data'**
  String get statusNoData;

  /// No description provided for @statusStale.
  ///
  /// In en, this message translates to:
  /// **'Stale reading'**
  String get statusStale;

  /// No description provided for @statusInsufficientHistory.
  ///
  /// In en, this message translates to:
  /// **'Insufficient history ({count}/{needed})'**
  String statusInsufficientHistory(int count, int needed);

  /// No description provided for @statusContextOnly.
  ///
  /// In en, this message translates to:
  /// **'Context only'**
  String get statusContextOnly;

  /// No description provided for @statusIncreasing.
  ///
  /// In en, this message translates to:
  /// **'Increasing'**
  String get statusIncreasing;

  /// No description provided for @statusDecreasing.
  ///
  /// In en, this message translates to:
  /// **'Decreasing'**
  String get statusDecreasing;

  /// No description provided for @statusPersistent.
  ///
  /// In en, this message translates to:
  /// **'Persistent'**
  String get statusPersistent;

  /// No description provided for @zonesSummaryLabel.
  ///
  /// In en, this message translates to:
  /// **'Body zones evaluated: {count}'**
  String zonesSummaryLabel(int count);

  /// No description provided for @confirmDialogTitle.
  ///
  /// In en, this message translates to:
  /// **'Confirm pulse reading?'**
  String get confirmDialogTitle;

  /// No description provided for @confirmDialogDemoTitle.
  ///
  /// In en, this message translates to:
  /// **'Demo data will be removed'**
  String get confirmDialogDemoTitle;

  /// No description provided for @confirmDialogDemoBody.
  ///
  /// In en, this message translates to:
  /// **'Saving real data will remove the demo data.'**
  String get confirmDialogDemoBody;

  /// No description provided for @confirmDialogWarningsIntro.
  ///
  /// In en, this message translates to:
  /// **'Review these warnings before saving:'**
  String get confirmDialogWarningsIntro;

  /// No description provided for @confirmDialogLocalNote.
  ///
  /// In en, this message translates to:
  /// **'This reading will be saved locally on this device.'**
  String get confirmDialogLocalNote;

  /// No description provided for @confirmDialogCancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get confirmDialogCancel;

  /// No description provided for @confirmDialogConfirm.
  ///
  /// In en, this message translates to:
  /// **'Confirm and save'**
  String get confirmDialogConfirm;

  /// No description provided for @warningUnusualValue.
  ///
  /// In en, this message translates to:
  /// **'This pulse value is outside the typical range.'**
  String get warningUnusualValue;

  /// No description provided for @warningFutureTimestamp.
  ///
  /// In en, this message translates to:
  /// **'The measurement time is in the future.'**
  String get warningFutureTimestamp;

  /// No description provided for @warningOldMeasurement.
  ///
  /// In en, this message translates to:
  /// **'The measurement time is more than seven days old.'**
  String get warningOldMeasurement;

  /// No description provided for @warningFallback.
  ///
  /// In en, this message translates to:
  /// **'Review this reading before saving ({code}).'**
  String warningFallback(String code);

  /// No description provided for @errorBpmRequired.
  ///
  /// In en, this message translates to:
  /// **'Enter a pulse in bpm.'**
  String get errorBpmRequired;

  /// No description provided for @errorBpmInvalid.
  ///
  /// In en, this message translates to:
  /// **'Enter a positive number in bpm.'**
  String get errorBpmInvalid;

  /// No description provided for @errorTimeInvalid.
  ///
  /// In en, this message translates to:
  /// **'Enter a valid UTC time (YYYY-MM-DD HH:MM).'**
  String get errorTimeInvalid;

  /// No description provided for @errorNoteTooLong.
  ///
  /// In en, this message translates to:
  /// **'Note must contain at most 2,000 characters.'**
  String get errorNoteTooLong;

  /// No description provided for @errorIdempotencyConflict.
  ///
  /// In en, this message translates to:
  /// **'This submission conflicts with a previously saved reading. Edit a field and try again.'**
  String get errorIdempotencyConflict;

  /// No description provided for @errorActionExpired.
  ///
  /// In en, this message translates to:
  /// **'The target profile changed before saving. Review and try again.'**
  String get errorActionExpired;

  /// No description provided for @errorLocalServiceUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The local service could not be reached. This action was not confirmed.'**
  String get errorLocalServiceUnavailable;

  /// No description provided for @errorStorageBusy.
  ///
  /// In en, this message translates to:
  /// **'The local store is busy. Try again; your entry has been kept.'**
  String get errorStorageBusy;

  /// No description provided for @errorNotFoundReplaces.
  ///
  /// In en, this message translates to:
  /// **'The original reading to correct was not found.'**
  String get errorNotFoundReplaces;

  /// No description provided for @errorGenericSave.
  ///
  /// In en, this message translates to:
  /// **'The reading could not be saved. Your entry has been kept.'**
  String get errorGenericSave;

  /// No description provided for @semanticsBpmField.
  ///
  /// In en, this message translates to:
  /// **'Pulse reading in beats per minute'**
  String get semanticsBpmField;

  /// No description provided for @semanticsTimeField.
  ///
  /// In en, this message translates to:
  /// **'Optional measurement time in UTC'**
  String get semanticsTimeField;

  /// No description provided for @semanticsNoteField.
  ///
  /// In en, this message translates to:
  /// **'Optional note for the pulse reading'**
  String get semanticsNoteField;

  /// No description provided for @semanticsSaveButton.
  ///
  /// In en, this message translates to:
  /// **'Save pulse reading'**
  String get semanticsSaveButton;

  /// No description provided for @startupStorageError.
  ///
  /// In en, this message translates to:
  /// **'Pulsys could not open local storage.'**
  String get startupStorageError;

  /// No description provided for @pulseStatusLastAvailable.
  ///
  /// In en, this message translates to:
  /// **'Last available pulse status'**
  String get pulseStatusLastAvailable;

  /// No description provided for @pulseStatusUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Pulse status could not be refreshed.'**
  String get pulseStatusUnavailable;

  /// No description provided for @pulseStatusRetry.
  ///
  /// In en, this message translates to:
  /// **'Retry pulse status'**
  String get pulseStatusRetry;

  /// No description provided for @pulseHistoryLastAvailable.
  ///
  /// In en, this message translates to:
  /// **'Last available pulse history'**
  String get pulseHistoryLastAvailable;

  /// No description provided for @pulseRecoveryNewerVersion.
  ///
  /// In en, this message translates to:
  /// **'A pending reading requires a newer app version. It has been kept on this device.'**
  String get pulseRecoveryNewerVersion;

  /// No description provided for @pulseRecoveryUnavailable.
  ///
  /// In en, this message translates to:
  /// **'A pending reading could not be recovered. It has been kept on this device.'**
  String get pulseRecoveryUnavailable;

  /// No description provided for @pulseRecoveryRetry.
  ///
  /// In en, this message translates to:
  /// **'Retry recovery'**
  String get pulseRecoveryRetry;

  /// No description provided for @pulseCorrectionHeading.
  ///
  /// In en, this message translates to:
  /// **'Correcting a previous reading'**
  String get pulseCorrectionHeading;

  /// No description provided for @pulseCorrectAction.
  ///
  /// In en, this message translates to:
  /// **'Correct reading'**
  String get pulseCorrectAction;

  /// No description provided for @pulseDestinationLabel.
  ///
  /// In en, this message translates to:
  /// **'Save for {target}'**
  String pulseDestinationLabel(String target);

  /// No description provided for @pulseLocalProfile.
  ///
  /// In en, this message translates to:
  /// **'Local profile'**
  String get pulseLocalProfile;

  /// No description provided for @bodyLegendTitle.
  ///
  /// In en, this message translates to:
  /// **'Zone status'**
  String get bodyLegendTitle;

  /// Título de pantalla.
  ///
  /// In en, this message translates to:
  /// **'Body zones'**
  String get bodyTitle;

  /// Encabezado de la figura anatómica frontal.
  ///
  /// In en, this message translates to:
  /// **'Front view'**
  String get bodyFront;

  /// Encabezado de la lista equivalente.
  ///
  /// In en, this message translates to:
  /// **'Zone list'**
  String get bodyList;

  /// Abre la lista desde la figura.
  ///
  /// In en, this message translates to:
  /// **'Show zone list'**
  String get bodyShowList;

  /// Vuelve a la figura desde la lista.
  ///
  /// In en, this message translates to:
  /// **'Show body figure'**
  String get bodyShowFigure;

  /// Marca datos simulados en pantalla y ficha.
  ///
  /// In en, this message translates to:
  /// **'DEMO'**
  String get bodyDemo;

  /// Lectura de zonas en curso.
  ///
  /// In en, this message translates to:
  /// **'Loading body zones…'**
  String get bodyLoading;

  /// Fallo de lectura de las zonas.
  ///
  /// In en, this message translates to:
  /// **'Body zones could not be loaded.'**
  String get bodyLoadError;

  /// Reintenta la consulta.
  ///
  /// In en, this message translates to:
  /// **'Try again'**
  String get bodyRetry;

  /// Cierra la ficha de zona.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get bodyClose;

  /// Estado estable devuelto por Rust.
  ///
  /// In en, this message translates to:
  /// **'Stable'**
  String get bodyStable;

  /// Cambio devuelto por Rust.
  ///
  /// In en, this message translates to:
  /// **'Change'**
  String get bodyChange;

  /// Alarma devuelta por Rust.
  ///
  /// In en, this message translates to:
  /// **'Alarm'**
  String get bodyAlarm;

  /// Estado sin evaluar devuelto por Rust.
  ///
  /// In en, this message translates to:
  /// **'Not evaluated'**
  String get bodyNotEvaluated;

  /// La respuesta no contiene esta zona.
  ///
  /// In en, this message translates to:
  /// **'Status unavailable'**
  String get bodyStatusUnavailable;

  /// Fecha del resumen completo.
  ///
  /// In en, this message translates to:
  /// **'Snapshot time (UTC)'**
  String get bodySnapshotTime;

  /// Encabezado de cada evaluación individual.
  ///
  /// In en, this message translates to:
  /// **'Symptom evaluation'**
  String get bodyEvaluation;

  /// Motivo devuelto en la evaluación.
  ///
  /// In en, this message translates to:
  /// **'Reason'**
  String get bodyReason;

  /// Motivo no_data.
  ///
  /// In en, this message translates to:
  /// **'No data'**
  String get bodyNoData;

  /// Motivo stale.
  ///
  /// In en, this message translates to:
  /// **'Stale reading'**
  String get bodyStale;

  /// Motivo too_few_data.
  ///
  /// In en, this message translates to:
  /// **'Insufficient history'**
  String get bodyTooFewData;

  /// Motivo not_applicable.
  ///
  /// In en, this message translates to:
  /// **'Context only'**
  String get bodyNotApplicable;

  /// El campo no aparece en la respuesta.
  ///
  /// In en, this message translates to:
  /// **'Not provided'**
  String get bodyMissing;

  /// Fecha de la evaluación individual.
  ///
  /// In en, this message translates to:
  /// **'Evaluation time (UTC)'**
  String get bodyEvaluationTime;

  /// Tipo de procedencia de la evaluación.
  ///
  /// In en, this message translates to:
  /// **'Source type'**
  String get bodySourceKind;

  /// Referencia de la fuente devuelta por el núcleo.
  ///
  /// In en, this message translates to:
  /// **'Source reference'**
  String get bodySourceRef;

  /// Tipo de fuente published.
  ///
  /// In en, this message translates to:
  /// **'Published source'**
  String get bodyPublished;

  /// Tipo de fuente approved_protocol.
  ///
  /// In en, this message translates to:
  /// **'Approved protocol'**
  String get bodyApprovedProtocol;

  /// Tipo de fuente demo_heuristic.
  ///
  /// In en, this message translates to:
  /// **'Demo rule, not medical advice.'**
  String get bodyDemoHeuristic;

  /// Identificador de regla sin traducir.
  ///
  /// In en, this message translates to:
  /// **'Rule ID'**
  String get bodyRuleId;

  /// Versión de la regla sin traducir.
  ///
  /// In en, this message translates to:
  /// **'Rule version'**
  String get bodyRuleVersion;

  /// La zona carece de evaluaciones individuales y sus metadatos.
  ///
  /// In en, this message translates to:
  /// **'No symptom evaluation was provided for this zone.'**
  String get bodyNoEvidence;

  /// Nombre anatómico de head.
  ///
  /// In en, this message translates to:
  /// **'Head'**
  String get bodyHead;

  /// Nombre anatómico de neck.
  ///
  /// In en, this message translates to:
  /// **'Neck'**
  String get bodyNeck;

  /// Nombre anatómico de chest.
  ///
  /// In en, this message translates to:
  /// **'Chest'**
  String get bodyChest;

  /// Nombre anatómico de abdomen.
  ///
  /// In en, this message translates to:
  /// **'Abdomen'**
  String get bodyAbdomen;

  /// Nombre anatómico de right_arm.
  ///
  /// In en, this message translates to:
  /// **'Right arm'**
  String get bodyRightArm;

  /// Nombre anatómico de left_arm.
  ///
  /// In en, this message translates to:
  /// **'Left arm'**
  String get bodyLeftArm;

  /// Nombre anatómico de right_hand.
  ///
  /// In en, this message translates to:
  /// **'Right hand'**
  String get bodyRightHand;

  /// Nombre anatómico de left_hand.
  ///
  /// In en, this message translates to:
  /// **'Left hand'**
  String get bodyLeftHand;

  /// Nombre anatómico de right_leg.
  ///
  /// In en, this message translates to:
  /// **'Right leg'**
  String get bodyRightLeg;

  /// Nombre anatómico de left_leg.
  ///
  /// In en, this message translates to:
  /// **'Left leg'**
  String get bodyLeftLeg;

  /// Nombre anatómico de right_foot.
  ///
  /// In en, this message translates to:
  /// **'Right foot'**
  String get bodyRightFoot;

  /// Nombre anatómico de left_foot.
  ///
  /// In en, this message translates to:
  /// **'Left foot'**
  String get bodyLeftFoot;

  /// Nombre anatómico de back.
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get bodyBack;

  /// Nombre anatómico de other.
  ///
  /// In en, this message translates to:
  /// **'Other'**
  String get bodyOther;

  /// No description provided for @navEvaluation.
  ///
  /// In en, this message translates to:
  /// **'Evaluation'**
  String get navEvaluation;

  /// No description provided for @navReadings.
  ///
  /// In en, this message translates to:
  /// **'Readings'**
  String get navReadings;

  /// No description provided for @navPreviousResult.
  ///
  /// In en, this message translates to:
  /// **'{result} could not be refreshed. Previous data from {time}.'**
  String navPreviousResult(String result, String time);

  /// No description provided for @navResultFailed.
  ///
  /// In en, this message translates to:
  /// **'{result} could not be loaded.'**
  String navResultFailed(String result);

  /// No description provided for @navResultLoading.
  ///
  /// In en, this message translates to:
  /// **'{result} is loading.'**
  String navResultLoading(String result);

  /// No description provided for @navEvaluationNotForReading.
  ///
  /// In en, this message translates to:
  /// **'Evaluation unavailable for this reading.'**
  String get navEvaluationNotForReading;

  /// No description provided for @navReason.
  ///
  /// In en, this message translates to:
  /// **'Reason and evaluation'**
  String get navReason;

  /// No description provided for @navSources.
  ///
  /// In en, this message translates to:
  /// **'Sources'**
  String get navSources;

  /// No description provided for @navMeasurements.
  ///
  /// In en, this message translates to:
  /// **'Measurements used'**
  String get navMeasurements;

  /// No description provided for @navEvidenceRecords.
  ///
  /// In en, this message translates to:
  /// **'Evaluation record IDs: {ids}'**
  String navEvidenceRecords(String ids);

  /// No description provided for @navLeisureComingSoon.
  ///
  /// In en, this message translates to:
  /// **'Leisure preference · Coming soon'**
  String get navLeisureComingSoon;

  /// No description provided for @navExerciseComingSoon.
  ///
  /// In en, this message translates to:
  /// **'Mission exercise guidelines · Coming soon'**
  String get navExerciseComingSoon;

  /// No description provided for @historyPersistentAlert.
  ///
  /// In en, this message translates to:
  /// **'Persistent pulse change'**
  String get historyPersistentAlert;

  /// No description provided for @historyRangeEmpty.
  ///
  /// In en, this message translates to:
  /// **'No pulse readings in the selected range ({range}).'**
  String historyRangeEmpty(String range);

  /// No description provided for @historyDemoUnavailable.
  ///
  /// In en, this message translates to:
  /// **'DEMO scenarios are not available in this version.'**
  String get historyDemoUnavailable;

  /// No description provided for @navEvaluatedRecord.
  ///
  /// In en, this message translates to:
  /// **'Evaluated record: {id}'**
  String navEvaluatedRecord(String id);

  /// No description provided for @navEvidenceEvents.
  ///
  /// In en, this message translates to:
  /// **'Evaluation event IDs: {ids}'**
  String navEvidenceEvents(String ids);

  /// No description provided for @navLatestPulse.
  ///
  /// In en, this message translates to:
  /// **'Latest pulse reading'**
  String get navLatestPulse;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
