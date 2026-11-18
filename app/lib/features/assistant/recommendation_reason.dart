import '../../platform/pulsys_api.dart';

sealed class RecommendationReason {
  const RecommendationReason();

  static RecommendationReason fromSnapshot(ZoneSnapshot snapshot) {
    final reasons = <RecommendationReason>[
      PulseRecommendationReason(snapshot.pulse),
      for (final zone in snapshot.zones) ZoneRecommendationReason(zone),
    ];
    for (final status in <EvaluationStatus>[
      EvaluationStatus.alarm,
      EvaluationStatus.change,
    ]) {
      for (final reason in reasons) {
        if (reason.status == status) return reason;
      }
    }
    return PulseRecommendationReason(snapshot.pulse);
  }

  EvaluationStatus get status;
}

final class PulseRecommendationReason extends RecommendationReason {
  const PulseRecommendationReason(this.evaluation);
  final PulseEvaluation evaluation;

  @override
  EvaluationStatus get status => evaluation.status;
}

final class ZoneRecommendationReason extends RecommendationReason {
  const ZoneRecommendationReason(this.zone);
  final ZoneState zone;

  @override
  EvaluationStatus get status => zone.status;
}
