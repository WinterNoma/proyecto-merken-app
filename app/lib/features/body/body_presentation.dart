import 'package:material_ui/material_ui.dart';

import '../../design/theme.dart';
import '../../design/tokens/spacing.dart';
import '../../design/tokens/typography.dart';
import '../../l10n/generated/app_localizations.dart';
import '../../platform/pulsys_api.dart';

extension BodyZoneLabel on BodyZone {
  (String, String) _identity(AppLocalizations l10n) => switch (this) {
    BodyZone.head => ('head', l10n.bodyHead),
    BodyZone.neck => ('neck', l10n.bodyNeck),
    BodyZone.chest => ('chest', l10n.bodyChest),
    BodyZone.abdomen => ('abdomen', l10n.bodyAbdomen),
    BodyZone.rightArm => ('right_arm', l10n.bodyRightArm),
    BodyZone.leftArm => ('left_arm', l10n.bodyLeftArm),
    BodyZone.rightHand => ('right_hand', l10n.bodyRightHand),
    BodyZone.leftHand => ('left_hand', l10n.bodyLeftHand),
    BodyZone.rightLeg => ('right_leg', l10n.bodyRightLeg),
    BodyZone.leftLeg => ('left_leg', l10n.bodyLeftLeg),
    BodyZone.rightFoot => ('right_foot', l10n.bodyRightFoot),
    BodyZone.leftFoot => ('left_foot', l10n.bodyLeftFoot),
    BodyZone.back => ('back', l10n.bodyBack),
    BodyZone.other => ('other', l10n.bodyOther),
  };

  String id(AppLocalizations l10n) => _identity(l10n).$1;

  String label(AppLocalizations l10n) => _identity(l10n).$2;
}

enum BodyStatus {
  stable(PulsysTone.ok, Icons.check_circle_outline),
  change(PulsysTone.warn, Icons.warning_amber_rounded),
  alarm(PulsysTone.alarm, Icons.report_gmailerrorred_outlined),
  notEvaluated(PulsysTone.neutral, Icons.remove_circle_outline),
  unavailable(PulsysTone.neutral, Icons.help_outline);

  const BodyStatus(this.tone, this.icon);

  factory BodyStatus.from(EvaluationStatus? status) => switch (status) {
    EvaluationStatus.stable => stable,
    EvaluationStatus.change => change,
    EvaluationStatus.alarm => alarm,
    EvaluationStatus.notEvaluated => notEvaluated,
    null => unavailable,
  };

  final PulsysTone tone;
  final IconData icon;

  PulsysToneColors get colors => PulsysToneColors.forTone(tone);

  String label(AppLocalizations l10n) => switch (this) {
    BodyStatus.stable => l10n.bodyStable,
    BodyStatus.change => l10n.bodyChange,
    BodyStatus.alarm => l10n.bodyAlarm,
    BodyStatus.notEvaluated => l10n.bodyNotEvaluated,
    BodyStatus.unavailable => l10n.bodyStatusUnavailable,
  };
}

class BodyStatusLabel extends StatelessWidget {
  const BodyStatusLabel(this.status, {super.key});

  final BodyStatus status;

  @override
  Widget build(BuildContext context) {
    final colors = status.colors;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(
          status.icon,
          size: PulsysSpacing.iconSize,
          color: colors.foreground,
        ),
        const SizedBox(width: PulsysSpacing.s2),
        Flexible(
          child: Text(
            status.label(AppLocalizations.of(context)),
            style: PulsysTypography.label.copyWith(color: colors.foreground),
          ),
        ),
      ],
    );
  }
}

class BodyZoneButton extends StatelessWidget {
  const BodyZoneButton({
    super.key,
    required this.zone,
    required this.status,
    required this.selected,
    required this.onSelect,
    this.marker = false,
  });

  final BodyZone zone;
  final BodyStatus status;
  final bool selected;
  final ValueChanged<BodyZone> onSelect;
  final bool marker;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final name = zone.label(l10n);
    void activate() => onSelect(zone);
    return Semantics(
      container: true,
      button: true,
      selected: selected,
      label: '$name, ${status.label(l10n)}',
      onTap: activate,
      excludeSemantics: true,
      child: marker
          ? IconButton(
              onPressed: activate,
              tooltip: '$name, ${status.label(l10n)}',
              icon: DecoratedBox(
                decoration: BoxDecoration(
                  color: status.colors.background,
                  shape: BoxShape.circle,
                  border: selected
                      ? Border.all(color: Theme.of(context).colorScheme.primary)
                      : null,
                ),
                child: Padding(
                  padding: const EdgeInsets.all(2),
                  child: Icon(
                    status.icon,
                    color: status.colors.foreground,
                    size: PulsysSpacing.iconSize,
                  ),
                ),
              ),
            )
          : OutlinedButton(
              onPressed: activate,
              child: Align(
                alignment: AlignmentDirectional.centerStart,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(name, style: PulsysTypography.bodyStrong),
                    BodyStatusLabel(status),
                  ],
                ),
              ),
            ),
    );
  }
}
