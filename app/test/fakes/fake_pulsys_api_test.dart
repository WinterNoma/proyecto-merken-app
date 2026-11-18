import 'package:flutter_test/flutter_test.dart';
import 'package:pulsys/platform/pulsys_api.dart';

import 'fake_pulsys_api.dart';

void main() {
  test(
    'fake evaluation is an injected fixture regardless of record count',
    () async {
      final fixture = PulseEvaluation(
        status: EvaluationStatus.change,
        persistent: false,
        escalation: EscalationLevel.flagForGround,
        evaluatedAtUtc: DateTime.utc(2026),
        ruleId: 'fixture',
        ruleVersion: 'fixture',
        sourceKind: SourceKind.demoHeuristic,
        inputRecordIds: const <String>[],
        historyCount: 0,
        historyNeeded: 0,
      );
      final api = FakePulsysApi(initialPulseEvaluation: fixture);
      for (var index = 0; index < 5; index++) {
        await api.confirmPulse(
          await api.preparePulse(PulseInput(key: 'key-$index', bpm: Bpm(72))),
        );
      }
      expect((await api.zoneStatus()).pulse, same(fixture));
      expect(
        (await FakePulsysApi().zoneStatus()).pulse.status,
        EvaluationStatus.notEvaluated,
      );
    },
  );

  test(
    'replay fingerprint distinguishes target identity and nullable note',
    () async {
      final api = FakePulsysApi(astronautId: 'first');
      final action = await api.preparePulse(
        PulseInput(key: 'key', bpm: Bpm(72)),
      );
      await api.confirmPulse(action);
      for (final target in <PulseTarget>[
        const LocalPulseTarget(storeId: 'fake-store-1', astronautId: 'second'),
        const LocalPulseTarget(storeId: 'other', astronautId: 'first'),
        const DemoPulseTarget(
          demo: DemoTarget(generation: '1', astronautId: 'first'),
        ),
        const DemoPulseTarget(
          demo: DemoTarget(generation: '2', astronautId: 'first'),
        ),
      ]) {
        final changed = PreparedPulse(
          key: action.key,
          bpm: action.bpm,
          measuredAtUtc: action.measuredAtUtc,
          target: target,
          warnings: const <PulseWarning>[],
        );
        await expectLater(
          api.confirmPulse(changed),
          throwsA(
            isA<ApiError>().having(
              (e) => e.code,
              'code',
              ApiErrorCodes.idempotencyConflict,
            ),
          ),
        );
      }
      final withEmptyNote = PreparedPulse(
        key: action.key,
        bpm: action.bpm,
        measuredAtUtc: action.measuredAtUtc,
        note: '',
        target: action.target,
        warnings: const <PulseWarning>[],
      );
      await expectLater(
        api.confirmPulse(withEmptyNote),
        throwsA(isA<ApiError>()),
      );
      expect(api.historySnapshot, hasLength(1));
    },
  );
  test('demo replay distinguishes generation and astronaut identity', () async {
    final api = FakePulsysApi();
    PreparedPulse action(DemoTarget demo) => PreparedPulse(
      key: 'demo-key',
      bpm: Bpm(72),
      measuredAtUtc: DateTime.utc(2026),
      target: DemoPulseTarget(demo: demo),
      warnings: const <PulseWarning>[],
    );
    await api.confirmPulse(
      action(const DemoTarget(generation: 'first', astronautId: 'crew-1')),
    );
    for (final changed in <DemoTarget>[
      const DemoTarget(generation: 'second', astronautId: 'crew-1'),
      const DemoTarget(generation: 'first', astronautId: 'crew-2'),
    ]) {
      await expectLater(
        api.confirmPulse(action(changed)),
        throwsA(
          isA<ApiError>().having(
            (e) => e.code,
            'code',
            ApiErrorCodes.idempotencyConflict,
          ),
        ),
      );
    }
    expect(api.historySnapshot, hasLength(1));
  });
}
