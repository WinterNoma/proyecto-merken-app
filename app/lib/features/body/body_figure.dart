import 'package:material_ui/material_ui.dart';

import '../../design/tokens/colors.dart';
import '../../design/tokens/spacing.dart';
import '../../design/tokens/typography.dart';
import '../../l10n/generated/app_localizations.dart';
import '../../platform/pulsys_api.dart';
import 'body_geometry.dart';
import 'body_presentation.dart';

const _scale = 1.28;
const _origin = Offset(12.8, 16);
const _diagramSize = Size(256, 584);

Offset _markerCenter(BodyRegion region, double width) {
  final factor = width / _diagramSize.width;
  final original = (_origin + region.marker * _scale) * factor;
  final center = width / 2;
  const gap = PulsysSpacing.minTarget;
  const radius = gap / 2;
  final x = switch (region.zone) {
    BodyZone.neck ||
    BodyZone.leftArm ||
    BodyZone.leftHand => original.dx.clamp(center + gap, width - radius),
    BodyZone.rightArm ||
    BodyZone.rightHand => original.dx.clamp(radius, center - gap),
    BodyZone.rightLeg ||
    BodyZone.rightFoot => original.dx.clamp(radius, center - radius),
    BodyZone.leftLeg ||
    BodyZone.leftFoot => original.dx.clamp(center + radius, width - radius),
    _ => original.dx,
  };
  return Offset(x.toDouble(), original.dy);
}

class BodyFigure extends StatelessWidget {
  const BodyFigure({
    super.key,
    required this.zones,
    required this.selected,
    required this.onSelect,
  });

  final Map<BodyZone, ZoneState> zones;
  final BodyZone? selected;
  final ValueChanged<BodyZone> onSelect;

  void _selectPath(TapUpDetails tap, double factor) {
    final point = (tap.localPosition / factor - _origin) / _scale;
    for (final region in bodyRegions) {
      if (region.path.contains(point)) {
        onSelect(region.zone);
        return;
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return LayoutBuilder(
      builder: (context, constraints) {
        final width = constraints.maxWidth.clamp(160.0, _diagramSize.width);
        final factor = width / _diagramSize.width;
        return SizedBox(
          width: width,
          height: _diagramSize.height * factor,
          child: Stack(
            children: <Widget>[
              Positioned.fill(
                child: GestureDetector(
                  key: const ValueKey('body-canvas'),
                  excludeFromSemantics: true,
                  behavior: HitTestBehavior.opaque,
                  onTapUp: (tap) => _selectPath(tap, factor),
                  child: CustomPaint(
                    painter: _BodyPainter(zones: zones, selected: selected),
                  ),
                ),
              ),
              Positioned(
                top: PulsysSpacing.s2,
                left: PulsysSpacing.s2,
                child: Text(
                  l10n.navBodyRightMarker,
                  semanticsLabel: l10n.navBodyRightLabel,
                  style: PulsysTypography.label,
                ),
              ),
              Positioned(
                top: PulsysSpacing.s2,
                right: PulsysSpacing.s2,
                child: Text(
                  l10n.navBodyLeftMarker,
                  semanticsLabel: l10n.navBodyLeftLabel,
                  style: PulsysTypography.label,
                ),
              ),
              for (final region in bodyRegions)
                Positioned.fromRect(
                  rect: Rect.fromCenter(
                    center: _markerCenter(region, width),
                    width: PulsysSpacing.minTarget,
                    height: PulsysSpacing.minTarget,
                  ),
                  child: BodyZoneButton(
                    key: ValueKey('figure-${region.zone.id(l10n)}'),
                    zone: region.zone,
                    status: BodyStatus.from(zones[region.zone]?.status),
                    selected: selected == region.zone,
                    onSelect: onSelect,
                    marker: true,
                  ),
                ),
            ],
          ),
        );
      },
    );
  }
}

class _BodyPainter extends CustomPainter {
  const _BodyPainter({required this.zones, required this.selected});

  final Map<BodyZone, ZoneState> zones;
  final BodyZone? selected;

  @override
  void paint(Canvas canvas, Size size) {
    final factor = size.width / _diagramSize.width;
    canvas.save();
    canvas.scale(factor);
    canvas.translate(_origin.dx, _origin.dy);
    canvas.scale(_scale);
    final fill = Paint();
    final edge = Paint()
      ..style = PaintingStyle.stroke
      ..strokeJoin = StrokeJoin.round;
    canvas.drawPath(bodyOutline, fill..color = PulsysColors.surfaceRaised);
    canvas.drawPath(
      bodyOutline,
      edge
        ..color = PulsysColors.border
        ..strokeWidth = 1.25,
    );
    for (final region in bodyRegions) {
      final colors = BodyStatus.from(zones[region.zone]?.status).colors;
      final active = region.zone == selected;
      fill.color = colors.background;
      edge
        ..color = active ? PulsysColors.focus : colors.foreground
        ..strokeWidth = active ? 2.5 : 1.25;
      canvas.drawPath(region.path, fill);
      canvas.drawPath(region.path, edge);
      final target =
          (_markerCenter(region, size.width) / factor - _origin) / _scale;
      if (region.zone == BodyZone.neck) {
        canvas.drawLine(const Offset(103, 68), target, edge);
      } else if ((target - region.marker).distance > .01) {
        canvas.drawLine(region.marker, target, edge);
      }
    }
    canvas.restore();
  }

  @override
  bool shouldRepaint(_BodyPainter oldDelegate) =>
      zones != oldDelegate.zones || selected != oldDelegate.selected;
}
