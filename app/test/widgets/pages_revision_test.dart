import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/app.dart';
import 'package:pulsys/features/overview/status_summary.dart';
import 'package:pulsys/features/overview/vital_card.dart';
import 'package:pulsys/features/pulse/pulse_screen.dart';
import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';
import '../support/overview_fixture.dart';

const _failure = ApiError(code: ApiErrorCodes.busy, message: 'fixture');

void main() {
  for (final failStatus in <bool>[true, false]) {
    testWidgets(
      'partial refresh after saving identifies the previous ${failStatus ? 'evaluation' : 'reading'} until retry',
      (tester) async {
        var now = overviewClock();
        DateTime clock() => now;
        final api = overviewApi(clock: clock);
        await tester.pumpWidget(PulsysApp(api: api, clock: clock));
        await tester.pumpAndSettle();
        final card = find.byKey(const ValueKey('overview-pulse-card'));
        expect(_inside(card, 'Stable'), findsOneWidget);
        await _tap(tester, card);
        await _save(tester, '78');
        final point = (await api.effectivePulseHistory()).first;
        now = now.add(const Duration(hours: 1));
        api.setPulseEvaluation(_evaluation(point, now));
        if (failStatus) {
          api.enqueueZoneStatusError(_failure);
        } else {
          api.enqueueHistoryError(_failure);
        }
        await tester.pageBack();
        await tester.pumpAndSettle();
        _expectPage(tester, 'today');
        final previousNotice = failStatus
            ? 'Evaluation could not be refreshed. Previous data from 18 Nov 2026, 12:00 UTC.'
            : 'Readings could not be refreshed. Previous data from 18 Nov 2026, 11:45 UTC.';
        expect(_inside(card, failStatus ? '78 bpm' : '72 bpm'), findsOneWidget);
        expect(_inside(card, previousNotice), findsOneWidget);
        expect(_inside(card, 'Stable'), findsNothing);
        expect(_inside(card, 'Alarm'), findsNothing);
        if (failStatus) {
          expect(
            find.descendant(
              of: find.byType(StatusSummary),
              matching: find.textContaining(previousNotice),
            ),
            findsOneWidget,
          );
        }
        await _page(tester, 'recommendations');
        expect(find.text(previousNotice), findsOneWidget);
        expect(
          _inside(
            find.byKey(const ValueKey('recommendation-latest-pulse')),
            failStatus ? '78 bpm' : '72 bpm',
          ),
          findsOneWidget,
        );
        expect(
          find.text('Evaluation unavailable for this reading.'),
          findsOneWidget,
        );
        await _tap(tester, find.text('Status could not be refreshed. Retry'));
        expect(find.text(previousNotice), findsNothing);
        expect(
          find.text('Evaluation unavailable for this reading.'),
          findsNothing,
        );
        expect(
          _inside(
            find.byKey(const ValueKey('recommendation-measurements')),
            '78 bpm',
          ),
          findsOneWidget,
        );
        await _page(tester, 'today');
        expect(_inside(card, '78 bpm'), findsOneWidget);
        expect(_inside(card, 'Alarm'), findsOneWidget);
        expect(find.textContaining('Previous data from'), findsNothing);
      },
    );
  }

  testWidgets(
    'initial query failures are distinct from an empty store on both pages',
    (tester) async {
      final api = overviewApi()
        ..enqueueHistoryError(_failure)
        ..enqueueZoneStatusError(_failure);
      await tester.pumpWidget(PulsysApp(api: api, clock: overviewClock));
      await tester.pumpAndSettle();
      final card = find.byKey(const ValueKey('overview-pulse-card'));
      expect(_inside(card, 'Readings could not be loaded.'), findsOneWidget);
      expect(_inside(card, 'Evaluation could not be loaded.'), findsOneWidget);
      expect(_inside(card, 'No data'), findsNothing);
      await _page(tester, 'recommendations');
      expect(find.text('Readings could not be loaded.'), findsOneWidget);
      expect(find.text('Evaluation could not be loaded.'), findsOneWidget);
      expect(find.text('Contact flight surgeons.'), findsNothing);
      await _tap(tester, find.text('Status could not be refreshed. Retry'));
      expect(find.text('Readings could not be loaded.'), findsNothing);
      expect(find.text('Evaluation could not be loaded.'), findsNothing);
      expect(find.text('72 bpm'), findsOneWidget);
    },
  );

  for (final confirming in <bool>[false, true]) {
    testWidgets(
      'changing API discards the old ${confirming ? 'confirmation' : 'draft'} and writes only to the new session',
      (tester) async {
        final first = overviewApi()..storeId = 'session-A';
        final second = FakePulsysApi(
          clock: overviewClock,
          storeId: 'session-B',
        );
        first.enqueuePrepareWarnings(const <PulseWarning>[
          PulseWarning(
            code: PulseWarning.unusualValue,
            ruleId: 'fixture.warning',
          ),
        ]);
        await tester.pumpWidget(PulsysApp(api: first, clock: overviewClock));
        await tester.pumpAndSettle();
        await _tap(tester, find.byKey(const ValueKey('overview-pulse-card')));
        await tester.enterText(
          find.byKey(const ValueKey('pulse-bpm-field')),
          '201',
        );
        if (confirming) {
          await _tap(tester, find.byKey(const ValueKey('pulse-save-button')));
          expect(find.text('Confirm pulse reading?'), findsOneWidget);
        }
        final previousWrites = first.confirmedActions.length;
        await tester.pumpWidget(PulsysApp(api: second, clock: overviewClock));
        await tester.pumpAndSettle();
        _expectPage(tester, 'today');
        expect(find.byType(PulseScreen), findsNothing);
        expect(find.byType(AlertDialog), findsNothing);
        expect(first.confirmedActions, hasLength(previousWrites));
        expect(first.historySnapshot.single.bpm, Bpm(72));
        expect(second.preparedInputs, isEmpty);
        await _tap(tester, find.byKey(const ValueKey('overview-pulse-card')));
        final field = tester.widget<TextField>(
          find.byKey(const ValueKey('pulse-bpm-field')),
        );
        expect(field.controller!.text, isEmpty);
        await _save(tester, '78');
        expect(first.historySnapshot.single.bpm, Bpm(72));
        expect(first.confirmedActions, hasLength(previousWrites));
        expect(second.historySnapshot.single.bpm, Bpm(78));
        expect(second.preparedInputs.single.bpm, Bpm(78));
        expect(
          (second.confirmedActions.single.target as LocalPulseTarget).storeId,
          'session-B',
        );
      },
    );
  }

  testWidgets(
    'zone list keeps all eight signs and pulse entry remains available after returning',
    (tester) async {
      await tester.pumpWidget(
        PulsysApp(api: overviewApi(), clock: overviewClock),
      );
      await tester.pumpAndSettle();
      await _tap(tester, find.text('Show zone list'));
      expect(find.byType(VitalCard), findsNWidgets(8));
      expect(find.byKey(const ValueKey('body-canvas')), findsNothing);
      expect(find.byKey(const ValueKey('list-head')), findsOneWidget);
      await _tap(tester, find.byKey(const ValueKey('overview-pulse-card')));
      expect(find.byType(PulseScreen), findsOneWidget);
      await tester.pageBack();
      await tester.pumpAndSettle();
      _expectPage(tester, 'today');
      expect(find.text('Show body figure'), findsOneWidget);
      expect(find.byKey(const ValueKey('list-head')), findsOneWidget);
      expect(find.byType(VitalCard), findsNWidgets(8));
      await _tap(tester, find.byKey(const ValueKey('list-head')));
      expect(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.text('Head'),
        ),
        findsOneWidget,
      );
    },
  );

  for (final persistent in <bool>[false, true]) {
    testWidgets(
      'History ${persistent ? 'shows' : 'omits'} the persistent Rust alert above the profile',
      (tester) async {
        await tester.pumpWidget(
          PulsysApp(
            api: overviewApi(
              persistent: persistent,
              pulseStatus: EvaluationStatus.change,
            ),
            clock: overviewClock,
          ),
        );
        await tester.pumpAndSettle();
        await _page(tester, 'history');
        final alert = find.byKey(const ValueKey('history-alerts'));
        expect(alert, persistent ? findsOneWidget : findsNothing);
        if (persistent) {
          expect(_inside(alert, 'Persistent pulse change'), findsOneWidget);
          expect(
            _inside(alert, 'Change detected · Persistent'),
            findsOneWidget,
          );
          expect(_inside(alert, '18 Nov 2026, 12:00 UTC'), findsOneWidget);
          expect(
            find.descendant(
              of: alert,
              matching: find.textContaining('fixture.pulse'),
            ),
            findsOneWidget,
          );
          expect(
            tester.getRect(alert).bottom,
            lessThanOrEqualTo(tester.getRect(find.byType(ProfileSummary)).top),
          );
        }
      },
    );
  }

  testWidgets(
    'zone alarm places suggestion, reason, action and sources before measurements and disclaimer',
    (tester) async {
      await tester.pumpWidget(
        PulsysApp(
          api: overviewApi(zoneStatus: EvaluationStatus.alarm),
          clock: overviewClock,
        ),
      );
      await tester.pumpAndSettle();
      await _page(tester, 'recommendations');
      final blocks = <String>[
        'suggestion',
        'reason',
        'action',
        'sources',
        'measurements',
        'disclaimer',
        'leisure',
        'exercise',
      ];
      for (var index = 1; index < blocks.length; index++) {
        final previous = tester.getRect(
          find.byKey(ValueKey('recommendation-${blocks[index - 1]}')),
        );
        final next = tester.getRect(
          find.byKey(ValueKey('recommendation-${blocks[index]}')),
        );
        expect(next.top, greaterThanOrEqualTo(previous.bottom));
      }
      expect(
        _inside(
          find.byKey(const ValueKey('recommendation-reason')),
          'Chest · Alarm',
        ),
        findsOneWidget,
      );
      expect(
        _inside(
          find.byKey(const ValueKey('recommendation-action')),
          'Contact flight surgeons.',
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('recommendation-sources')),
          matching: find.text(
            'No symptom evaluation was provided for this zone.',
          ),
        ),
        findsOneWidget,
      );
      expect(find.text('This is guidance, not a diagnosis.'), findsOneWidget);
      expect(find.text('Leisure preference · Coming soon'), findsOneWidget);
      expect(
        find.text('Mission exercise guidelines · Coming soon'),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'already open History refreshes correction, keeps its range and agrees with Today',
    (tester) async {
      final api = overviewApi();
      final original = api.historySnapshot.single;
      await tester.pumpWidget(PulsysApp(api: api, clock: overviewClock));
      await tester.pumpAndSettle();
      await _page(tester, 'history');
      await _tap(tester, find.widgetWithText(ChoiceChip, '6 h'));
      final oldRow = find.byKey(
        ValueKey('history-reading-${original.recordId}'),
      );
      await tester.ensureVisible(oldRow);
      expect(_inside(oldRow, '72 bpm'), findsOneWidget);
      await _tap(tester, find.byKey(const ValueKey('history-pulse-card')));
      await _tap(
        tester,
        find.byKey(ValueKey('pulse-correct-${original.recordId}')),
      );
      await _save(tester, '78');
      final corrected = (await api.effectivePulseHistory()).single;
      api.setPulseEvaluation(_evaluation(corrected, overviewClock()));
      await tester.pageBack();
      await tester.pumpAndSettle();
      _expectPage(tester, 'history');
      expect(
        tester
            .widget<ChoiceChip>(find.widgetWithText(ChoiceChip, '6 h'))
            .selected,
        isTrue,
      );
      final row = find.byKey(ValueKey('history-reading-${corrected.recordId}'));
      await tester.ensureVisible(row);
      await tester.pumpAndSettle();
      expect(_inside(row, '78 bpm'), findsOneWidget);
      expect(oldRow, findsNothing);
      final historyCard = find.byKey(const ValueKey('history-pulse-card'));
      expect(_inside(historyCard, '78 bpm'), findsOneWidget);
      expect(_inside(historyCard, 'Alarm'), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(ProfileSummary),
          matching: find.textContaining('Pulse · Alarm'),
        ),
        findsOneWidget,
      );
      await _page(tester, 'today');
      final todayCard = find.byKey(const ValueKey('overview-pulse-card'));
      expect(_inside(todayCard, '78 bpm'), findsOneWidget);
      expect(_inside(todayCard, 'Alarm'), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(StatusSummary),
          matching: find.textContaining('Pulse · Alarm'),
        ),
        findsOneWidget,
      );
      expect(find.text('72 bpm'), findsNothing);
    },
  );
}

PulseEvaluation _evaluation(PulsePoint point, DateTime time) => PulseEvaluation(
  status: EvaluationStatus.alarm,
  persistent: false,
  escalation: EscalationLevel.urgent,
  evaluatedAtUtc: time,
  latestMeasuredAtUtc: point.measuredAtUtc,
  ruleId: 'fixture.updated',
  ruleVersion: 'fixture-v2',
  sourceKind: SourceKind.demoHeuristic,
  inputRecordIds: <String>[point.recordId],
  historyCount: 1,
  historyNeeded: 1,
);

Finder _inside(Finder parent, String text) =>
    find.descendant(of: parent, matching: find.text(text));

Future<void> _tap(WidgetTester tester, Finder target) async {
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

Future<void> _save(WidgetTester tester, String bpm) async {
  await tester.enterText(find.byKey(const ValueKey('pulse-bpm-field')), bpm);
  await _tap(tester, find.byKey(const ValueKey('pulse-save-button')));
}

Future<void> _page(WidgetTester tester, String page) async {
  await _tap(tester, find.byKey(ValueKey('nav-$page')));
  _expectPage(tester, page);
}

void _expectPage(WidgetTester tester, String page) {
  expect(
    tester.semantics.find(find.byKey(ValueKey('nav-$page'))),
    isSemantics(isSelected: true),
  );
  expect(
    find.byKey(PageStorageKey<String>('$page-scroll')).hitTestable(),
    findsOneWidget,
  );
}
