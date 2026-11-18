import 'package:material_ui/material_ui.dart';
import 'package:intl/intl.dart';

import '../../design/theme.dart';
import '../../l10n/generated/app_localizations.dart';
import '../../platform/pulsys_api.dart';
import 'pulse_controller.dart';

@immutable
final class StatusPresentation {
  const StatusPresentation({
    required this.tone,
    required this.icon,
    required this.label,
  });

  final PulsysTone tone;
  final IconData icon;
  final String label;
}

abstract final class PulseFormat {
  static final DateFormat _utcFormatter = DateFormat('dd MMM yyyy, HH:mm');

  static String utcTimestamp(DateTime instant) {
    return '${_utcFormatter.format(instant.toUtc())} UTC';
  }

  static String recordSource(AppLocalizations l10n, RecordSource source) {
    return switch (source) {
      RecordSource.form => l10n.pulseSourceForm,
      RecordSource.text => l10n.pulseSourceText,
      RecordSource.voice => l10n.pulseSourceVoice,
      RecordSource.earth => l10n.pulseSourceEarth,
      RecordSource.garmin => l10n.pulseSourceGarmin,
    };
  }

  static StatusPresentation pulseStatus(
    AppLocalizations l10n,
    PulseEvaluation evaluation,
  ) {
    switch (evaluation.status) {
      case EvaluationStatus.stable:
        return StatusPresentation(
          tone: PulsysTone.ok,
          icon: Icons.check_circle_outline,
          label: l10n.statusStable,
        );
      case EvaluationStatus.change:
        final parts = <String>[l10n.statusChange];
        if (evaluation.direction == TrendDirection.up) {
          parts.add(l10n.statusIncreasing);
        } else if (evaluation.direction == TrendDirection.down) {
          parts.add(l10n.statusDecreasing);
        }
        if (evaluation.persistent) {
          parts.add(l10n.statusPersistent);
        }
        return StatusPresentation(
          tone: PulsysTone.warn,
          icon: Icons.warning_amber_rounded,
          label: parts.join(' · '),
        );
      case EvaluationStatus.alarm:
        return StatusPresentation(
          tone: PulsysTone.alarm,
          icon: Icons.error_outline,
          label: l10n.statusAlarm,
        );
      case EvaluationStatus.notEvaluated:
        return switch (evaluation.reason) {
          NotEvaluatedReason.noData => StatusPresentation(
            tone: PulsysTone.neutral,
            icon: Icons.remove_circle_outline,
            label: l10n.statusNoData,
          ),
          NotEvaluatedReason.stale => StatusPresentation(
            tone: PulsysTone.neutral,
            icon: Icons.schedule,
            label: l10n.statusStale,
          ),
          NotEvaluatedReason.tooFewData => StatusPresentation(
            tone: PulsysTone.neutral,
            icon: Icons.bar_chart,
            label: l10n.statusInsufficientHistory(
              evaluation.historyCount,
              evaluation.historyNeeded,
            ),
          ),
          NotEvaluatedReason.notApplicable => StatusPresentation(
            tone: PulsysTone.neutral,
            icon: Icons.info_outline,
            label: l10n.statusContextOnly,
          ),
          null => StatusPresentation(
            tone: PulsysTone.neutral,
            icon: Icons.remove_circle_outline,
            label: l10n.statusNotEvaluated,
          ),
        };
    }
  }

  static String warningMessage(AppLocalizations l10n, PulseWarning warning) {
    return switch (warning.code) {
      PulseWarning.unusualValue => l10n.warningUnusualValue,
      PulseWarning.futureTimestamp => l10n.warningFutureTimestamp,
      PulseWarning.oldMeasurement => l10n.warningOldMeasurement,
      _ => l10n.warningFallback(warning.code),
    };
  }

  static String? bpmErrorText(
    AppLocalizations l10n,
    BpmValidationError? error,
  ) {
    return switch (error) {
      BpmValidationError.required => l10n.errorBpmRequired,
      BpmValidationError.invalid => l10n.errorBpmInvalid,
      null => null,
    };
  }

  static String? timeErrorText(
    AppLocalizations l10n,
    TimeValidationError? error,
  ) {
    return switch (error) {
      TimeValidationError.invalidFormat => l10n.errorTimeInvalid,
      null => null,
    };
  }

  static String? noteErrorText(
    AppLocalizations l10n,
    NoteValidationError? error,
  ) {
    return switch (error) {
      NoteValidationError.tooLong => l10n.errorNoteTooLong,
      null => null,
    };
  }

  static String submitErrorText(AppLocalizations l10n, PulseSubmitError error) {
    return switch (error) {
      PulseSubmitError.idempotencyConflict => l10n.errorIdempotencyConflict,
      PulseSubmitError.actionExpired => l10n.errorActionExpired,
      PulseSubmitError.localServiceUnavailable =>
        l10n.errorLocalServiceUnavailable,
      PulseSubmitError.storageBusy => l10n.errorStorageBusy,
      PulseSubmitError.notFoundReplaces => l10n.errorNotFoundReplaces,
      PulseSubmitError.genericSave => l10n.errorGenericSave,
    };
  }
}
