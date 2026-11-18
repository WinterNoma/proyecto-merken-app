import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';

DateTime overviewClock() => DateTime.utc(2026, 11, 18, 12);

FakePulsysApi overviewApi({
  EvaluationStatus pulseStatus = EvaluationStatus.stable,
  EvaluationStatus zoneStatus = EvaluationStatus.notEvaluated,
  EscalationLevel escalation = EscalationLevel.continueReading,
  bool persistent = false,
  DateTime Function()? clock,
}) {
  final effectiveClock = clock ?? overviewClock;
  final now = effectiveClock();
  return FakePulsysApi(
    clock: effectiveClock,
    initialHistory: <PulsePoint>[
      PulsePoint(
        recordId: 'overview-reading',
        rootId: 'overview-reading',
        bpm: Bpm(72),
        source: RecordSource.form,
        measuredAtUtc: now.subtract(const Duration(minutes: 15)),
        loggedAtUtc: now.subtract(const Duration(minutes: 14)),
        isDemo: false,
      ),
    ],
    initialPulseEvaluation: PulseEvaluation(
      status: pulseStatus,
      persistent: persistent,
      escalation: escalation,
      evaluatedAtUtc: now,
      ruleId: 'fixture.pulse',
      ruleVersion: 'fixture-v1',
      sourceKind: SourceKind.demoHeuristic,
      inputRecordIds: const <String>['overview-reading'],
      historyCount: 1,
      historyNeeded: 1,
    ),
    initialZones: <ZoneState>[
      for (final zone in BodyZone.values)
        ZoneState(
          zoneId: zone,
          status: zone == BodyZone.chest
              ? zoneStatus
              : EvaluationStatus.notEvaluated,
          symptoms: const <SymptomEvaluation>[],
        ),
    ],
  );
}
