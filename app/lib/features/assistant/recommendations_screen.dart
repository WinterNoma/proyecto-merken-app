import 'package:material_ui/material_ui.dart';

import '../../design/tokens/spacing.dart';
import '../../design/tokens/typography.dart';
import '../../l10n/generated/app_localizations.dart';
import '../../platform/pulsys_api.dart';
import '../body/body_presentation.dart';
import '../overview/overview_controller.dart';
import '../overview/status_summary.dart';
import '../pulse/pulse_format.dart';
import 'recommendation_reason.dart';

class RecommendationsScreen extends StatelessWidget {
  const RecommendationsScreen({super.key, required this.controller});

  final OverviewController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final snapshot = controller.snapshot;
    final reason = snapshot == null
        ? null
        : RecommendationReason.fromSnapshot(snapshot);
    final usedPoint =
        reason is PulseRecommendationReason &&
            controller.currentPulseEvaluation != null
        ? controller.latestPulse
        : null;
    final notice = overviewResultNotice(
      l10n,
      controller.status,
      l10n.navEvaluation,
      previousAtUtc: snapshot?.evaluatedAtUtc,
    );
    return SingleChildScrollView(
      key: const PageStorageKey<String>('recommendations-scroll'),
      padding: const EdgeInsets.all(PulsysSpacing.s4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(l10n.navRecommendations, style: PulsysTypography.sectionTitle),
          const SizedBox(height: PulsysSpacing.s3),
          Card(
            key: const ValueKey('recommendation-suggestion'),
            child: Padding(
              padding: const EdgeInsets.all(PulsysSpacing.s4),
              child: Text(l10n.navSuggestionComingSoon),
            ),
          ),
          Column(
            key: const ValueKey('recommendation-reason'),
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(l10n.navReason, style: PulsysTypography.componentHeading),
              if (notice != null) Text(notice),
              if (reason != null)
                Text(
                  _reasonLabel(l10n, reason),
                  key: const ValueKey('recommendation-principal-reason'),
                ),
              if (snapshot != null)
                Text(PulseFormat.utcTimestamp(snapshot.evaluatedAtUtc)),
            ],
          ),
          if (reason?.status == EvaluationStatus.alarm)
            Semantics(
              key: const ValueKey('recommendation-action'),
              liveRegion: true,
              child: Text(
                l10n.navContactFlightSurgeons,
                style: PulsysTypography.bodyStrong,
              ),
            ),
          const SizedBox(height: PulsysSpacing.s3),
          Column(
            key: const ValueKey('recommendation-sources'),
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(l10n.navSources, style: PulsysTypography.componentHeading),
              ..._sources(l10n, reason),
            ],
          ),
          const SizedBox(height: PulsysSpacing.s3),
          Column(
            key: const ValueKey('recommendation-measurements'),
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                l10n.navMeasurements,
                style: PulsysTypography.componentHeading,
              ),
              if (reason case PulseRecommendationReason(:final evaluation))
                _PulseEvidence(evaluation: evaluation, point: usedPoint),
              if (reason case ZoneRecommendationReason(:final zone))
                _ZoneEvidence(zone: zone),
              if (snapshot != null)
                for (final zone in snapshot.zones)
                  if (reason is! ZoneRecommendationReason ||
                      zone != reason.zone)
                    _ZoneEvidence(zone: zone),
            ],
          ),
          if (usedPoint == null) ...<Widget>[
            const SizedBox(height: PulsysSpacing.s3),
            _LatestPulse(controller: controller),
          ],
          const SizedBox(height: PulsysSpacing.s4),
          Text(
            l10n.navGuidanceDisclaimer,
            key: const ValueKey('recommendation-disclaimer'),
          ),
          const SizedBox(height: PulsysSpacing.s4),
          Text(
            l10n.navLeisureComingSoon,
            key: const ValueKey('recommendation-leisure'),
          ),
          const SizedBox(height: PulsysSpacing.s4),
          Text(
            l10n.navExerciseComingSoon,
            key: const ValueKey('recommendation-exercise'),
          ),
        ],
      ),
    );
  }
}

String _reasonLabel(AppLocalizations l10n, RecommendationReason reason) =>
    switch (reason) {
      PulseRecommendationReason(:final evaluation) => l10n.navPulseStatus(
        PulseFormat.pulseStatus(l10n, evaluation).label,
      ),
      ZoneRecommendationReason(:final zone) => l10n.navZoneStatus(
        zone.zoneId.label(l10n),
        BodyStatus.from(zone.status).label(l10n),
      ),
    };

List<Widget> _sources(AppLocalizations l10n, RecommendationReason? reason) =>
    switch (reason) {
      PulseRecommendationReason(:final evaluation) => <Widget>[
        Text(
          evaluationSource(
            l10n,
            kind: evaluation.sourceKind,
            reference: evaluation.sourceRef,
            ruleId: evaluation.ruleId,
            version: evaluation.ruleVersion,
          ),
        ),
      ],
      ZoneRecommendationReason(:final zone) => <Widget>[
        if (zone.symptoms.isEmpty) Text(l10n.bodyNoEvidence),
        for (final symptom in zone.symptoms)
          Text(
            evaluationSource(
              l10n,
              kind: symptom.sourceKind,
              reference: symptom.sourceRef,
              ruleId: symptom.ruleId,
              version: symptom.ruleVersion,
            ),
          ),
      ],
      null => <Widget>[Text(l10n.bodyMissing)],
    };

class _PulseEvidence extends StatelessWidget {
  const _PulseEvidence({required this.evaluation, required this.point});
  final PulseEvaluation evaluation;
  final PulsePoint? point;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final reading = point;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          l10n.navEvaluatedRecord(
            evaluation.evaluatedRecordId ?? l10n.bodyMissing,
          ),
        ),
        if (reading != null) ...<Widget>[
          Text('${reading.bpm.format()} ${l10n.pulseBpmUnit}'),
          Text(PulseFormat.utcTimestamp(reading.measuredAtUtc)),
          Text(PulseFormat.recordSource(l10n, reading.source)),
        ],
        _RecordEvidence(
          recordIds: evaluation.inputRecordIds,
          eventIds: const <String>[],
          evaluatedAtUtc: evaluation.evaluatedAtUtc,
        ),
      ],
    );
  }
}

class _ZoneEvidence extends StatelessWidget {
  const _ZoneEvidence({required this.zone});
  final ZoneState zone;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Padding(
      key: ValueKey('recommendation-zone-${zone.zoneId.id(l10n)}'),
      padding: const EdgeInsets.symmetric(vertical: PulsysSpacing.s1),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(
            l10n.navZoneStatus(
              zone.zoneId.label(l10n),
              BodyStatus.from(zone.status).label(l10n),
            ),
          ),
          for (final symptom in zone.symptoms) ...<Widget>[
            Text(BodyStatus.from(symptom.status).label(l10n)),
            _RecordEvidence(
              recordIds: symptom.inputRecordIds,
              eventIds: symptom.inputEventIds,
              evaluatedAtUtc: symptom.evaluatedAtUtc,
            ),
            Text(
              evaluationSource(
                l10n,
                kind: symptom.sourceKind,
                reference: symptom.sourceRef,
                ruleId: symptom.ruleId,
                version: symptom.ruleVersion,
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _RecordEvidence extends StatelessWidget {
  const _RecordEvidence({
    required this.recordIds,
    required this.eventIds,
    required this.evaluatedAtUtc,
  });
  final List<String> recordIds;
  final List<String> eventIds;
  final DateTime evaluatedAtUtc;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          '${l10n.bodyEvaluationTime} · ${PulseFormat.utcTimestamp(evaluatedAtUtc)}',
        ),
        if (recordIds.isNotEmpty)
          Text(l10n.navEvidenceRecords(recordIds.join(', '))),
        if (eventIds.isNotEmpty)
          Text(l10n.navEvidenceEvents(eventIds.join(', '))),
      ],
    );
  }
}

class _LatestPulse extends StatelessWidget {
  const _LatestPulse({required this.controller});
  final OverviewController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final point = controller.latestPulse;
    final notice = overviewResultNotice(
      l10n,
      controller.readings,
      l10n.navReadings,
      previousAtUtc: point?.measuredAtUtc,
    );
    return Column(
      key: const ValueKey('recommendation-latest-pulse'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(l10n.navLatestPulse, style: PulsysTypography.componentHeading),
        if (notice != null) Text(notice),
        if (point != null) ...<Widget>[
          Text('${point.bpm.format()} ${l10n.pulseBpmUnit}'),
          Text(PulseFormat.utcTimestamp(point.measuredAtUtc)),
          Text(PulseFormat.recordSource(l10n, point.source)),
          if (controller.currentPulseEvaluation == null)
            Text(l10n.navEvaluationNotForReading),
        ] else if (notice == null)
          Text(l10n.statusNoData),
      ],
    );
  }
}
