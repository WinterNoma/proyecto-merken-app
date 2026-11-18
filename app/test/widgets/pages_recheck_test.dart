import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/app.dart';
import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';
import '../support/overview_fixture.dart';

void main() {
  for (final evaluatedLatest in <bool>[false, true]) {
    testWidgets(
      'multiple evidence IDs distinguish latest pulse as ${evaluatedLatest ? 'evaluated' : 'antecedent'} with both queries successful',
      (tester) async {
        final instant = overviewClock();
        final points = <PulsePoint>[
          for (final (id, value) in <(String, double)>[('A', 72), ('B', 85)])
            PulsePoint(
              recordId: id,
              rootId: id,
              bpm: Bpm(value),
              source: RecordSource.form,
              measuredAtUtc: instant,
              loggedAtUtc: instant,
              isDemo: false,
            ),
        ];
        final evidence = evaluatedLatest
            ? <String>['B', 'A']
            : <String>['A', 'B'];
        final evaluation = PulseEvaluation(
          status: EvaluationStatus.alarm,
          persistent: false,
          escalation: EscalationLevel.urgent,
          evaluatedAtUtc: instant,
          latestMeasuredAtUtc: instant,
          ruleId: 'fixture.pulse',
          ruleVersion: 'fixture-v1',
          sourceKind: SourceKind.demoHeuristic,
          inputRecordIds: evidence,
          historyCount: 1,
          historyNeeded: 3,
        );
        final api = FakePulsysApi(
          clock: overviewClock,
          initialHistory: points,
          initialPulseEvaluation: evaluation,
          initialZones: const <ZoneState>[],
        );
        await tester.pumpWidget(PulsysApp(api: api, clock: overviewClock));
        await tester.pumpAndSettle();
        final card = find.byKey(const ValueKey('overview-pulse-card'));
        expect(_inside(card, '85 bpm'), findsOneWidget);
        expect(evaluation.inputRecordIds, contains('B'));
        expect(evaluation.evaluatedRecordId, evaluatedLatest ? 'B' : 'A');
        expect(
          _inside(card, 'Alarm'),
          evaluatedLatest ? findsOneWidget : findsNothing,
        );
        expect(
          _inside(card, 'Evaluation unavailable for this reading.'),
          evaluatedLatest ? findsNothing : findsOneWidget,
        );
        expect(find.text('Status could not be refreshed. Retry'), findsNothing);
        expect(api.historyQueries.single.limit, 1);
        expect(api.zoneStatusCalls, 1);
        await _page(tester, 'recommendations');
        final used = find.byKey(const ValueKey('recommendation-measurements'));
        expect(
          _inside(used, 'Evaluated record: ${evaluatedLatest ? 'B' : 'A'}'),
          findsOneWidget,
        );
        expect(
          _inside(used, '85 bpm'),
          evaluatedLatest ? findsOneWidget : findsNothing,
        );
        if (!evaluatedLatest) {
          final latest = find.byKey(
            const ValueKey('recommendation-latest-pulse'),
          );
          expect(_inside(latest, '85 bpm'), findsOneWidget);
          expect(
            _inside(latest, 'Evaluation unavailable for this reading.'),
            findsOneWidget,
          );
        }
      },
    );
  }

  testWidgets(
    'several alarming zones have one brief reason before the action and their own symptom evidence',
    (tester) async {
      final now = overviewClock();
      final api = FakePulsysApi(
        clock: overviewClock,
        initialHistory: <PulsePoint>[
          PulsePoint(
            recordId: 'pulse-unrelated',
            rootId: 'pulse-unrelated',
            bpm: Bpm(72),
            source: RecordSource.form,
            measuredAtUtc: now,
            loggedAtUtc: now,
            isDemo: false,
          ),
        ],
        initialPulseEvaluation: PulseEvaluation(
          status: EvaluationStatus.stable,
          persistent: false,
          escalation: EscalationLevel.continueReading,
          evaluatedAtUtc: now,
          latestMeasuredAtUtc: now,
          ruleId: 'fixture.pulse-unrelated',
          ruleVersion: 'fixture-v1',
          sourceKind: SourceKind.demoHeuristic,
          inputRecordIds: const <String>['pulse-unrelated'],
          historyCount: 1,
          historyNeeded: 1,
        ),
        initialZones: <ZoneState>[
          for (final (zone, id) in <(BodyZone, String)>[
            (BodyZone.head, 'head'),
            (BodyZone.chest, 'chest'),
            (BodyZone.abdomen, 'abdomen'),
          ])
            ZoneState(
              zoneId: zone,
              status: EvaluationStatus.alarm,
              symptoms: <SymptomEvaluation>[
                SymptomEvaluation(
                  status: EvaluationStatus.alarm,
                  evaluatedAtUtc: now,
                  ruleId: 'fixture.$id',
                  ruleVersion: 'fixture-v1',
                  sourceKind: SourceKind.demoHeuristic,
                  sourceRef: 'fixture-source-$id',
                  inputRecordIds: <String>['$id-current', '$id-prior'],
                  inputEventIds: <String>['$id-event'],
                ),
              ],
            ),
        ],
      );
      await tester.pumpWidget(PulsysApp(api: api, clock: overviewClock));
      await tester.pumpAndSettle();
      await _page(tester, 'recommendations');
      final reason = find.byKey(const ValueKey('recommendation-reason'));
      final principal = find.byKey(
        const ValueKey('recommendation-principal-reason'),
      );
      expect(tester.widget<Text>(principal).data, 'Head · Alarm');
      expect(_inside(reason, 'Chest · Alarm'), findsNothing);
      expect(_inside(reason, 'Abdomen · Alarm'), findsNothing);
      expect(
        find.descendant(of: reason, matching: find.byType(Text)),
        findsNWidgets(3),
      );
      final action = find.byKey(const ValueKey('recommendation-action'));
      final used = find.byKey(const ValueKey('recommendation-measurements'));
      expect(
        tester.getRect(action).top,
        greaterThanOrEqualTo(tester.getRect(reason).bottom),
      );
      expect(
        tester.getRect(action).bottom,
        lessThanOrEqualTo(tester.getRect(used).top),
      );
      final sources = find.byKey(const ValueKey('recommendation-sources'));
      expect(
        find.descendant(
          of: sources,
          matching: find.textContaining('fixture-source-head'),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: sources,
          matching: find.textContaining('fixture.pulse-unrelated'),
        ),
        findsNothing,
      );
      for (final id in <String>['head', 'chest', 'abdomen']) {
        final group = find.byKey(ValueKey('recommendation-zone-$id'));
        expect(find.ancestor(of: group, matching: used), findsOneWidget);
        expect(
          _inside(group, 'Evaluation record IDs: $id-current, $id-prior'),
          findsOneWidget,
        );
        expect(
          _inside(group, 'Evaluation event IDs: $id-event'),
          findsOneWidget,
        );
        expect(
          find.descendant(
            of: group,
            matching: find.textContaining('fixture-source-$id'),
          ),
          findsOneWidget,
        );
      }
      expect(
        find.descendant(
          of: used,
          matching: find.textContaining('pulse-unrelated'),
        ),
        findsNothing,
      );
      expect(_inside(used, '72 bpm'), findsNothing);
      final latest = find.byKey(const ValueKey('recommendation-latest-pulse'));
      expect(_inside(latest, '72 bpm'), findsOneWidget);
      expect(
        _inside(latest, 'Evaluation unavailable for this reading.'),
        findsNothing,
      );
    },
  );
}

Finder _inside(Finder parent, String text) =>
    find.descendant(of: parent, matching: find.text(text));

Future<void> _page(WidgetTester tester, String page) async {
  await tester.tap(find.byKey(ValueKey('nav-$page')));
  await tester.pumpAndSettle();
  expect(
    tester.semantics.find(find.byKey(ValueKey('nav-$page'))),
    isSemantics(isSelected: true),
  );
}
