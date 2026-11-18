import 'package:material_ui/material_ui.dart';

import '../../design/tokens/spacing.dart';
import '../../design/tokens/typography.dart';
import '../../l10n/generated/app_localizations.dart';
import '../../platform/pulsys_api.dart';
import '../pulse/pulse_format.dart';
import 'overview_controller.dart';

class StatusSummary extends StatelessWidget {
  const StatusSummary({super.key, required this.snapshot, this.notice});

  final ZoneSnapshot snapshot;
  final String? notice;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final evaluated = snapshot.zones
        .where((zone) => zone.status != EvaluationStatus.notEvaluated)
        .length;
    return Text(
      <String>[
        ?notice,
        l10n.navPulseStatus(
          PulseFormat.pulseStatus(l10n, snapshot.pulse).label,
        ),
        l10n.zonesSummaryLabel(evaluated),
        PulseFormat.utcTimestamp(snapshot.evaluatedAtUtc),
        if (snapshot.isDemo) l10n.pulseDemoTag,
      ].join(' · '),
      style: PulsysTypography.caption,
    );
  }
}

class ProfileSummary extends StatelessWidget {
  const ProfileSummary({super.key, required this.snapshot, this.notice});

  final ZoneSnapshot snapshot;
  final String? notice;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(PulsysSpacing.s4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              l10n.pulseLocalProfile,
              style: PulsysTypography.componentHeading,
            ),
            StatusSummary(snapshot: snapshot, notice: notice),
            Text(
              l10n.historyEscalation(switch (snapshot.pulse.escalation) {
                EscalationLevel.continueReading => l10n.historyContinueReading,
                EscalationLevel.selfCare => l10n.historySelfCare,
                EscalationLevel.flagForGround => l10n.historyFlagForGround,
                EscalationLevel.urgent => l10n.historyUrgent,
              }),
            ),
            Text(l10n.historySyncUnavailable),
          ],
        ),
      ),
    );
  }
}

String? overviewResultNotice<T>(
  AppLocalizations l10n,
  OverviewResult<T> result,
  String label, {
  DateTime? previousAtUtc,
}) => switch (result) {
  OverviewCurrent<T>() => null,
  OverviewPrevious<T>(:final loadedAtUtc) => l10n.navPreviousResult(
    label,
    PulseFormat.utcTimestamp(previousAtUtc ?? loadedAtUtc),
  ),
  OverviewFailure<T>() => l10n.navResultFailed(label),
  OverviewLoading<T>() => l10n.navResultLoading(label),
};

String evaluationSource(
  AppLocalizations l10n, {
  required SourceKind kind,
  String? reference,
  required String ruleId,
  required String version,
}) => <String>[
  switch (kind) {
    SourceKind.published => l10n.bodyPublished,
    SourceKind.approvedProtocol => l10n.bodyApprovedProtocol,
    SourceKind.demoHeuristic => l10n.bodyDemoHeuristic,
  },
  reference ?? l10n.bodyMissing,
  ruleId,
  version,
].join(' · ');

class PersistentPulseAlert extends StatelessWidget {
  const PersistentPulseAlert({super.key, required this.controller});
  final OverviewController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final pulse = controller.snapshot!.pulse;
    final notice = overviewResultNotice(
      l10n,
      controller.status,
      l10n.navEvaluation,
      previousAtUtc: controller.snapshot!.evaluatedAtUtc,
    );
    return Card(
      key: const ValueKey('history-alerts'),
      child: Padding(
        padding: const EdgeInsets.all(PulsysSpacing.s4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              l10n.historyPersistentAlert,
              style: PulsysTypography.bodyStrong,
            ),
            if (notice != null) Text(notice),
            Text(PulseFormat.pulseStatus(l10n, pulse).label),
            Text(PulseFormat.utcTimestamp(pulse.evaluatedAtUtc)),
            Text(
              evaluationSource(
                l10n,
                kind: pulse.sourceKind,
                reference: pulse.sourceRef,
                ruleId: pulse.ruleId,
                version: pulse.ruleVersion,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
