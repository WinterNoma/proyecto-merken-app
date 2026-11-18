import 'package:flutter/widgets.dart';

import '../../design/tokens/colors.dart';
import '../../design/tokens/spacing.dart';
import 'pulse_trend.dart';

class PulseTrendPainter extends CustomPainter {
  const PulseTrendPainter({required this.trend});

  static const double _markerRadius = 3;

  final PulseTrend trend;

  @override
  void paint(Canvas canvas, Size size) {
    if (size.shortestSide <= 2 * _markerRadius) {
      return;
    }

    final plot = (Offset.zero & size).deflate(_markerRadius);
    final guide = Paint()
      ..color = PulsysColors.divider
      ..strokeWidth = PulsysBorders.control;
    canvas.drawLine(plot.centerLeft, plot.centerRight, guide);
    if (trend.minimumBpm != trend.maximumBpm) {
      canvas.drawLine(plot.topLeft, plot.topRight, guide);
      canvas.drawLine(plot.bottomLeft, plot.bottomRight, guide);
    }

    final startMs = trend.startUtc.millisecondsSinceEpoch;
    final durationMs = trend.endUtc.millisecondsSinceEpoch - startMs;
    final minimumBpm = trend.minimumBpm.value;
    final bpmRange = trend.maximumBpm.value - minimumBpm;
    final markers = trend.points
        .map((point) {
          final elapsedMs =
              point.measuredAtUtc.millisecondsSinceEpoch - startMs;
          final x = durationMs == 0 ? 0.5 : elapsedMs / durationMs;
          final y = bpmRange == 0
              ? 0.5
              : (point.bpm.value - minimumBpm) / bpmRange;
          return Offset(
            plot.left + plot.width * x,
            plot.bottom - plot.height * y,
          );
        })
        .toList(growable: false);

    if (markers.length > 1) {
      final line = Path()..addPolygon(markers, false);
      canvas.drawPath(
        line,
        Paint()
          ..color = PulsysColors.accent
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2
          ..strokeJoin = StrokeJoin.round,
      );
    }
    final markerPaint = Paint()..color = PulsysColors.accent;
    for (final marker in markers) {
      canvas.drawCircle(marker, _markerRadius, markerPaint);
    }
  }

  @override
  bool shouldRepaint(covariant PulseTrendPainter oldDelegate) =>
      oldDelegate.trend != trend;
}
