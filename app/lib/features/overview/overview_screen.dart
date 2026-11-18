import 'package:material_ui/material_ui.dart';

import '../../design/tokens/spacing.dart';
import '../../l10n/generated/app_localizations.dart';
import '../body/body_screen.dart';
import 'overview_controller.dart';
import 'status_summary.dart';
import 'vital_card.dart';

class OverviewScreen extends StatelessWidget {
  const OverviewScreen({
    super.key,
    required this.controller,
    required this.onOpenPulse,
  });

  final OverviewController controller;
  final VoidCallback onOpenPulse;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final snapshot = controller.snapshot;
    return SingleChildScrollView(
      key: const PageStorageKey<String>('today-scroll'),
      padding: const EdgeInsets.all(PulsysSpacing.s4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (snapshot != null)
            StatusSummary(
              snapshot: snapshot,
              notice: overviewResultNotice(
                l10n,
                controller.status,
                l10n.navEvaluation,
                previousAtUtc: snapshot.evaluatedAtUtc,
              ),
            )
          else
            Text(
              overviewResultNotice(
                    l10n,
                    controller.status,
                    l10n.navEvaluation,
                  ) ??
                  l10n.navLoadingStatus,
            ),
          if (snapshot != null) ...<Widget>[
            const SizedBox(height: PulsysSpacing.s3),
            BodyPanel(
              snapshot: snapshot,
              leading: _vitals(const <VitalIndicator>[
                VitalIndicator.pulse,
                VitalIndicator.pressure,
                VitalIndicator.oxygen,
                VitalIndicator.mood,
              ]),
              trailing: _vitals(const <VitalIndicator>[
                VitalIndicator.temperature,
                VitalIndicator.glucose,
                VitalIndicator.sleep,
                VitalIndicator.nutrition,
              ]),
            ),
          ] else
            BodyStage(
              leading: _vitals(const <VitalIndicator>[
                VitalIndicator.pulse,
                VitalIndicator.pressure,
                VitalIndicator.oxygen,
                VitalIndicator.mood,
              ]),
              trailing: _vitals(const <VitalIndicator>[
                VitalIndicator.temperature,
                VitalIndicator.glucose,
                VitalIndicator.sleep,
                VitalIndicator.nutrition,
              ]),
              child: Text(
                controller.statusFailed ? l10n.bodyLoadError : l10n.bodyLoading,
              ),
            ),
          const SizedBox(height: PulsysSpacing.s4),
          Wrap(
            key: const ValueKey<String>('overview-context'),
            spacing: PulsysSpacing.s3,
            runSpacing: PulsysSpacing.s2,
            children: <Widget>[
              Text(l10n.navContextValue(l10n.navRadiation)),
              Text(l10n.navContextValue(l10n.navDistance)),
              Text(l10n.navContextValue(l10n.navSynchronization)),
            ],
          ),
          const SizedBox(height: PulsysSpacing.s4),
          Column(
            key: const ValueKey<String>('overview-assistant'),
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  IconButton(
                    onPressed: null,
                    tooltip: l10n.navHoldToTalk,
                    icon: const Icon(Icons.mic_none),
                  ),
                  const SizedBox(width: PulsysSpacing.s2),
                  Expanded(
                    child: TextField(
                      enabled: false,
                      minLines: 1,
                      maxLines: 3,
                      decoration: InputDecoration(
                        labelText: l10n.navAssistantHint,
                        helperText: l10n.navComingSoon,
                      ),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _vitals(List<VitalIndicator> indicators) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: <Widget>[
      for (final indicator in indicators) ...<Widget>[
        VitalCard(
          key: ValueKey<String>('overview-${indicator.name}-card'),
          indicator: indicator,
          compact: true,
          controller: controller,
          onOpenPulse: onOpenPulse,
        ),
        const SizedBox(height: PulsysSpacing.s3),
      ],
    ],
  );
}
