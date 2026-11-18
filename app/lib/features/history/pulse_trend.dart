import 'package:flutter/foundation.dart';

import '../../platform/pulsys_api.dart';

@immutable
final class PulseTrend {
  const PulseTrend._({
    required this.points,
    required this.sourceCount,
    required this.minimumBpm,
    required this.maximumBpm,
    required this.hasDemoReadings,
  });

  static const int pointLimit = 30;

  final List<PulsePoint> points;
  final int sourceCount;
  final Bpm minimumBpm;
  final Bpm maximumBpm;
  final bool hasDemoReadings;

  DateTime get startUtc => points.first.measuredAtUtc;
  DateTime get endUtc => points.last.measuredAtUtc;

  static PulseTrend? fromEffectiveHistory(List<PulsePoint> newestFirst) {
    if (newestFirst.isEmpty) {
      return null;
    }

    final chronological = newestFirst.reversed.toList(growable: false);
    final points = _sample(chronological);
    var minimumBpm = points.first.bpm;
    var maximumBpm = minimumBpm;
    for (final point in points) {
      final bpm = point.bpm;
      if (bpm.value < minimumBpm.value) {
        minimumBpm = bpm;
      }
      if (bpm.value > maximumBpm.value) {
        maximumBpm = bpm;
      }
    }

    return PulseTrend._(
      points: List<PulsePoint>.unmodifiable(points),
      sourceCount: newestFirst.length,
      minimumBpm: minimumBpm,
      maximumBpm: maximumBpm,
      hasDemoReadings: newestFirst.any((point) => point.isDemo),
    );
  }

  static List<PulsePoint> _sample(List<PulsePoint> chronological) {
    if (chronological.length <= pointLimit) {
      return chronological;
    }

    final interiorCount = chronological.length - 2;
    const bucketCount = (pointLimit - 2) ~/ 2;
    final sampled = <PulsePoint>[chronological.first];
    for (var bucket = 0; bucket < bucketCount; bucket++) {
      final start = 1 + bucket * interiorCount ~/ bucketCount;
      final end = 1 + (bucket + 1) * interiorCount ~/ bucketCount;
      var minimum = start;
      var maximum = start;
      for (var index = start + 1; index < end; index++) {
        final bpm = chronological[index].bpm.value;
        if (bpm < chronological[minimum].bpm.value) {
          minimum = index;
        }
        // Opposite tie breaks keep two distinct readings in a flat bucket.
        if (bpm >= chronological[maximum].bpm.value) {
          maximum = index;
        }
      }
      if (minimum < maximum) {
        sampled.add(chronological[minimum]);
        sampled.add(chronological[maximum]);
      } else {
        sampled.add(chronological[maximum]);
        sampled.add(chronological[minimum]);
      }
    }
    sampled.add(chronological.last);
    return sampled;
  }
}
