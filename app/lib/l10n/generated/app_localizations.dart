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
