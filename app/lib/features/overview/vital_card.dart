import 'package:material_ui/material_ui.dart';

import '../../design/tokens/spacing.dart';
import '../../design/tokens/typography.dart';
import '../../l10n/generated/app_localizations.dart';
import '../pulse/pulse_format.dart';
import 'overview_controller.dart';
import 'status_summary.dart';

enum VitalIndicator {
  pulse,
  pressure,
  oxygen,
  mood,
  temperature,
  glucose,
  sleep,
  nutrition;

  String label(AppLocalizations l10n) => switch (this) {
    VitalIndicator.pulse => l10n.pulseScreenTitle,
    VitalIndicator.pressure => l10n.navPressure,
    VitalIndicator.oxygen => l10n.navOxygen,
    VitalIndicator.mood => l10n.navMood,
    VitalIndicator.temperature => l10n.navTemperature,
    VitalIndicator.glucose => l10n.navGlucose,
    VitalIndicator.sleep => l10n.navSleep,
    VitalIndicator.nutrition => l10n.navNutrition,
  };
}

class VitalCard extends StatelessWidget {
  const VitalCard({
    super.key,
    required this.indicator,
    required this.controller,
    this.onOpenPulse,
    this.compact = false,
  });

  final VitalIndicator indicator;
  final OverviewController controller;
  final VoidCallback? onOpenPulse;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final name = indicator.label(l10n);
    final pulse = indicator == VitalIndicator.pulse;
    final reading = pulse ? controller.latestPulse : null;
    final readingNotice = pulse
        ? overviewResultNotice(
            l10n,
            controller.readings,
            l10n.navReadings,
            previousAtUtc: reading?.measuredAtUtc,
          )
        : null;
    final evaluation = pulse ? controller.currentPulseEvaluation : null;
    final evaluationNotice = pulse
        ? overviewResultNotice(
            l10n,
            controller.status,
            l10n.navEvaluation,
            previousAtUtc: controller.snapshot?.evaluatedAtUtc,
          )
        : null;
    final value = reading == null
        ? readingNotice ?? l10n.statusNoData
        : '${reading.bpm.format()} ${l10n.pulseBpmUnit}';
    final state = pulse && evaluation != null
        ? PulseFormat.pulseStatus(l10n, evaluation).label
        : pulse
        ? evaluationNotice ?? l10n.navEvaluationNotForReading
        : null;
    final content = Padding(
      padding: const EdgeInsets.all(PulsysSpacing.s2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(
            name,
            style: PulsysTypography.label,
            maxLines: compact ? 1 : null,
            overflow: compact ? TextOverflow.ellipsis : null,
          ),
          if (compact)
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: FittedBox(
                fit: BoxFit.scaleDown,
                child: Text(value, style: PulsysTypography.dataCaption),
              ),
            )
          else
            Text(value, style: PulsysTypography.dataCaption),
          if (reading != null && readingNotice != null)
            Text(
              readingNotice,
              style: PulsysTypography.caption,
              maxLines: compact ? 1 : null,
              overflow: compact ? TextOverflow.ellipsis : null,
            ),
          if (state != null)
            Text(
              state,
              style: PulsysTypography.caption,
              maxLines: compact ? 1 : null,
              overflow: compact ? TextOverflow.ellipsis : null,
            ),
        ],
      ),
    );
    if (!pulse) {
      return Semantics(
        label: '$name, $value',
        excludeSemantics: true,
        child: Card(child: content),
      );
    }
    return Semantics(
      button: true,
      label: <String>[
        name,
        value,
        if (reading != null) ?readingNotice,
        ?state,
      ].join(', '),
      excludeSemantics: true,
      onTap: onOpenPulse,
      child: OutlinedButton(
        onPressed: onOpenPulse,
        style: OutlinedButton.styleFrom(
          padding: EdgeInsets.zero,
          alignment: AlignmentDirectional.centerStart,
        ),
        child: content,
      ),
    );
  }
}

class IndicatorList extends StatelessWidget {
  const IndicatorList({
    super.key,
    required this.controller,
    required this.onOpenPulse,
  });

  final OverviewController controller;
  final VoidCallback onOpenPulse;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Column(
      key: const ValueKey<String>('history-indicators'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(l10n.historyIndicators, style: PulsysTypography.componentHeading),
        for (final indicator in VitalIndicator.values) ...<Widget>[
          const SizedBox(height: PulsysSpacing.s2),
          VitalCard(
            indicator: indicator,
            key: ValueKey('history-${indicator.name}-card'),
            controller: controller,
            onOpenPulse: onOpenPulse,
          ),
          if (indicator == VitalIndicator.pulse &&
              controller.latestPulse != null)
            Text(
              PulseFormat.recordSource(l10n, controller.latestPulse!.source),
              style: PulsysTypography.caption,
            ),
          OutlinedButton(
            onPressed: null,
            child: Text(l10n.historyExplainComingSoon),
          ),
        ],
      ],
    );
  }
}
