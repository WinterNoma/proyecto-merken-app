import 'package:material_ui/material_ui.dart';

import '../../design/theme.dart';
import '../../design/tokens/colors.dart';
import '../../design/tokens/spacing.dart';
import '../../design/tokens/typography.dart';
import '../../l10n/generated/app_localizations.dart';
import '../../platform/pulsys_api.dart';
import 'pulse_format.dart';

Future<bool> showPulseConfirmationDialog({
  required BuildContext context,
  required PreparedPulse prepared,
}) async {
  final result = await showDialog<bool>(
    context: context,
    barrierDismissible: false,
    barrierColor: PulsysColors.scrim,
    builder: (BuildContext dialogContext) {
      return _PulseConfirmationDialog(prepared: prepared);
    },
  );
  return result ?? false;
}

class _PulseConfirmationDialog extends StatelessWidget {
  const _PulseConfirmationDialog({required this.prepared});

  final PreparedPulse prepared;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final removesDemo = prepared.demoToReplace != null;
    final confirmLabel = removesDemo
        ? l10n.pulseSaveRealDataAction
        : l10n.confirmDialogConfirm;
    final warnColors = PulsysToneColors.forTone(PulsysTone.warn);

    return AlertDialog(
      title: Text(l10n.confirmDialogTitle),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 440),
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                l10n.pulseSavedReceipt(
                  prepared.bpm.format(),
                  PulseFormat.utcTimestamp(prepared.measuredAtUtc),
                ),
                style: PulsysTypography.dataBody,
              ),
              const SizedBox(height: PulsysSpacing.s2),
              Text(
                l10n.pulseDestinationLabel(switch (prepared.target) {
                  LocalPulseTarget(:final astronautId) =>
                    astronautId ?? l10n.pulseLocalProfile,
                  DemoPulseTarget(:final demo) =>
                    demo.astronautId ?? l10n.pulseDemoTag,
                }),
                style: PulsysTypography.caption,
              ),
              if (prepared.warnings.isNotEmpty) ...<Widget>[
                const SizedBox(height: PulsysSpacing.s3),
                Text(
                  l10n.confirmDialogWarningsIntro,
                  style: PulsysTypography.bodyStrong,
                ),
                const SizedBox(height: PulsysSpacing.s2),
                for (final warning in prepared.warnings) ...<Widget>[
                  Container(
                    width: double.infinity,
                    margin: const EdgeInsets.only(bottom: PulsysSpacing.s2),
                    padding: const EdgeInsets.all(PulsysSpacing.s3),
                    decoration: BoxDecoration(
                      color: warnColors.background,
                      borderRadius: BorderRadius.circular(PulsysRadii.md),
                      border: Border(
                        left: BorderSide(
                          color: warnColors.foreground,
                          width: PulsysBorders.statusRule,
                        ),
                      ),
                    ),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Icon(
                          Icons.warning_amber_rounded,
                          size: PulsysSpacing.iconSize,
                          color: warnColors.foreground,
                        ),
                        const SizedBox(width: PulsysSpacing.s2),
                        Expanded(
                          child: Text(
                            PulseFormat.warningMessage(l10n, warning),
                            style: PulsysTypography.body,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ],
              if (removesDemo) ...<Widget>[
                const SizedBox(height: PulsysSpacing.s2),
                Container(
                  width: double.infinity,
                  padding: const EdgeInsets.all(PulsysSpacing.s3),
                  decoration: BoxDecoration(
                    color: PulsysColors.neutralBg,
                    borderRadius: BorderRadius.circular(PulsysRadii.md),
                    border: const Border(
                      left: BorderSide(
                        color: PulsysColors.neutral,
                        width: PulsysBorders.statusRule,
                      ),
                    ),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        l10n.confirmDialogDemoTitle,
                        style: PulsysTypography.bodyStrong,
                      ),
                      const SizedBox(height: PulsysSpacing.s1),
                      Text(
                        l10n.confirmDialogDemoBody,
                        style: PulsysTypography.caption,
                      ),
                    ],
                  ),
                ),
              ],
              const SizedBox(height: PulsysSpacing.s3),
              Text(
                l10n.confirmDialogLocalNote,
                style: PulsysTypography.caption,
              ),
            ],
          ),
        ),
      ),
      actionsAlignment: MainAxisAlignment.end,
      actions: <Widget>[
        Wrap(
          spacing: PulsysSpacing.actionGap,
          runSpacing: PulsysSpacing.actionGap,
          alignment: WrapAlignment.end,
          children: <Widget>[
            OutlinedButton(
              autofocus: true,
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l10n.confirmDialogCancel),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(confirmLabel),
            ),
          ],
        ),
      ],
    );
  }
}
