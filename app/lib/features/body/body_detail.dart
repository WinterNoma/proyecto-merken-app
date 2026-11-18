import 'package:intl/intl.dart';
import 'package:material_ui/material_ui.dart';

import '../../design/tokens/spacing.dart';
import '../../design/tokens/typography.dart';
import '../../l10n/generated/app_localizations.dart';
import '../../platform/pulsys_api.dart';
import 'body_presentation.dart';

class BodyDetail extends StatelessWidget {
  const BodyDetail({
    super.key,
    required this.zone,
    required this.state,
    required this.snapshot,
  });

  final BodyZone zone;
  final ZoneState? state;
  final ZoneSnapshot snapshot;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final utcFormat = DateFormat('dd MMM yyyy, HH:mm:ss', l10n.localeName);
    final symptoms = state?.symptoms ?? const <SymptomEvaluation>[];
    return AlertDialog(
      scrollable: true,
      title: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (snapshot.isDemo)
            Text(l10n.bodyDemo, style: PulsysTypography.label),
          Text(zone.label(l10n)),
        ],
      ),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          BodyStatusLabel(BodyStatus.from(state?.status)),
          _Field(
            l10n.bodySnapshotTime,
            utcFormat.format(snapshot.evaluatedAtUtc),
          ),
          if (symptoms.isEmpty) ...[
            const Divider(),
            Text(l10n.bodyNoEvidence),
            _Field(l10n.bodyReason, l10n.bodyMissing),
            _Field(l10n.bodyEvaluationTime, l10n.bodyMissing),
            _Field(l10n.bodySourceRef, l10n.bodyMissing),
            _Field(l10n.bodyRuleId, l10n.bodyMissing),
          ],
          for (final symptom in symptoms) ...[
            const Divider(),
            Semantics(
              header: true,
              child: Text(
                l10n.bodyEvaluation,
                style: PulsysTypography.bodyStrong,
              ),
            ),
            BodyStatusLabel(BodyStatus.from(symptom.status)),
            _Field(l10n.bodyReason, switch (symptom.reason) {
              NotEvaluatedReason.noData => l10n.bodyNoData,
              NotEvaluatedReason.stale => l10n.bodyStale,
              NotEvaluatedReason.tooFewData => l10n.bodyTooFewData,
              NotEvaluatedReason.notApplicable => l10n.bodyNotApplicable,
              null => l10n.bodyMissing,
            }),
            _Field(
              l10n.bodyEvaluationTime,
              utcFormat.format(symptom.evaluatedAtUtc),
            ),
            _Field(l10n.bodySourceKind, switch (symptom.sourceKind) {
              SourceKind.published => l10n.bodyPublished,
              SourceKind.approvedProtocol => l10n.bodyApprovedProtocol,
              SourceKind.demoHeuristic => l10n.bodyDemoHeuristic,
            }),
            _Field(l10n.bodySourceRef, symptom.sourceRef),
            _Field(l10n.bodyRuleId, symptom.ruleId),
            _Field(l10n.bodyRuleVersion, symptom.ruleVersion),
          ],
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.bodyClose),
        ),
      ],
    );
  }
}

class _Field extends StatelessWidget {
  const _Field(this.label, this.value);

  final String label;
  final String? value;

  @override
  Widget build(BuildContext context) {
    final text = value;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: PulsysSpacing.s1),
      child: MergeSemantics(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(label, style: PulsysTypography.caption),
            Text(
              text == null || text.trim().isEmpty
                  ? AppLocalizations.of(context).bodyMissing
                  : text,
              style: PulsysTypography.body,
            ),
          ],
        ),
      ),
    );
  }
}
