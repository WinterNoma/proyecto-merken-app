import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/app.dart';
import 'package:pulsys/features/body/body_screen.dart';
import 'package:pulsys/features/overview/vital_card.dart';
import 'package:pulsys/features/overview/overview_screen.dart';
import 'package:pulsys/features/overview/status_summary.dart';
import 'package:pulsys/features/pulse/pulse_screen.dart';
import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';
import '../support/overview_fixture.dart';

void main() {
  testWidgets('opens centrally and page buttons expose selection and tap', (
    tester,
  ) async {
    await tester.pumpWidget(
      PulsysApp(api: overviewApi(), clock: overviewClock),
    );
    await tester.pumpAndSettle();
    for (final (page, name) in <(String, String)>[
      ('history', 'History'),
      ('today', 'Today'),
      ('recommendations', 'Recommendations'),
    ]) {
      final button = find.byKey(ValueKey<String>('nav-$page'));
      expect(
        tester.semantics.find(button),
        isSemantics(
          label: name,
          isButton: true,
          isSelected: page == 'today',
          hasTapAction: true,
        ),
      );
    }
    for (final (page, name) in <(String, String)>[
      ('history', 'History'),
      ('today', 'Today'),
      ('recommendations', 'Recommendations'),
    ]) {
      tester.semantics.tap(find.semantics.byLabel(name));
      await tester.pumpAndSettle();
      expect(
        tester.semantics.find(find.byKey(ValueKey<String>('nav-$page'))),
        isSemantics(isSelected: true),
      );
    }
  });

  testWidgets('horizontal gestures and system back return to Today', (
    tester,
  ) async {
    await tester.pumpWidget(
      PulsysApp(api: overviewApi(), clock: overviewClock),
    );
    await tester.pumpAndSettle();
    final pager = find.byKey(const ValueKey<String>('app-pages'));
    await tester.dragFrom(
      tester.getTopLeft(pager) + const Offset(40, 20),
      const Offset(700, 0),
    );
    await tester.pumpAndSettle();
    expect(
      tester.semantics.find(find.byKey(const ValueKey('nav-history'))),
      isSemantics(isSelected: true),
    );
    _expectPage(tester, 'history');
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(
      tester.semantics.find(find.byKey(const ValueKey('nav-today'))),
      isSemantics(isSelected: true),
    );
    _expectPage(tester, 'today');
    await tester.dragFrom(
      tester.getTopRight(pager) + const Offset(-40, 20),
      const Offset(-700, 0),
    );
    await tester.pumpAndSettle();
    expect(
      tester.semantics.find(find.byKey(const ValueKey('nav-recommendations'))),
      isSemantics(isSelected: true),
    );
    _expectPage(tester, 'recommendations');
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(
      tester.semantics.find(find.byKey(const ValueKey('nav-today'))),
      isSemantics(isSelected: true),
    );
    expect(find.byType(OverviewScreen), findsOneWidget);
    var exitRequested = false;
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        if (call.method == 'SystemNavigator.pop') exitRequested = true;
        return null;
      },
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        null,
      ),
    );
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(exitRequested, isTrue);
  });

  testWidgets('wordmark returns centrally and help records no event or pulse', (
    tester,
  ) async {
    final api = overviewApi();
    await tester.pumpWidget(PulsysApp(api: api, clock: overviewClock));
    await tester.pumpAndSettle();
    await _page(tester, 'history');
    final headerBefore = tester.getRect(find.byKey(const ValueKey('nav-home')));
    await tester.drag(
      find.byKey(const PageStorageKey<String>('history-scroll')),
      const Offset(0, -400),
    );
    await tester.pumpAndSettle();
    expect(
      tester.getRect(find.byKey(const ValueKey('nav-home'))),
      headerBefore,
    );
    await tester.tap(find.byKey(const ValueKey('nav-home')));
    await tester.pumpAndSettle();
    expect(
      tester.semantics.find(find.byKey(const ValueKey('nav-today'))),
      isSemantics(isSelected: true),
    );
    expect(
      tester
          .widget<OutlinedButton>(
            find.byKey(const ValueKey<String>('nav-crew')),
          )
          .onPressed,
      isNull,
    );
    await tester.tap(find.byKey(const ValueKey('nav-help')));
    await tester.pumpAndSettle();
    expect(
      find.text('Help request recording will be available in stage 4.'),
      findsOneWidget,
    );
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(api.preparedInputs, isEmpty);
    expect(api.confirmedActions, isEmpty);
    expect(api.historySnapshot, hasLength(1));
  });

  testWidgets(
    'Today places signs beside the body and opens pulse from its card',
    (tester) async {
      tester.view.physicalSize = const Size(560, 1300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final api = overviewApi();
      await tester.pumpWidget(PulsysApp(api: api, clock: overviewClock));
      await tester.pumpAndSettle();
      final todayCards = find.descendant(
        of: find.byType(OverviewScreen),
        matching: find.byType(VitalCard),
      );
      final cards = tester.widgetList<VitalCard>(todayCards).toList();
      expect(cards.map((card) => card.indicator), <VitalIndicator>[
        VitalIndicator.pulse,
        VitalIndicator.pressure,
        VitalIndicator.oxygen,
        VitalIndicator.mood,
        VitalIndicator.temperature,
        VitalIndicator.glucose,
        VitalIndicator.sleep,
        VitalIndicator.nutrition,
      ]);
      final body = tester.getRect(find.byType(BodyPanel));
      final pulse = find.byKey(const ValueKey('overview-pulse-card'));
      final pulseRect = tester.getRect(pulse);
      final temperature = find.widgetWithText(VitalCard, 'Temperature');
      expect(pulseRect.center.dx, lessThan(body.center.dx));
      expect(tester.getCenter(temperature).dx, greaterThan(body.center.dx));
      await tester.tap(pulse);
      await tester.pumpAndSettle();
      expect(find.byType(PulseScreen), findsOneWidget);
      expect(find.text('Save reading'), findsOneWidget);
      await tester.pageBack();
      await tester.pumpAndSettle();
      expect(api.closed, isFalse);
      final neck = find.byKey(const ValueKey('figure-neck'));
      await tester.ensureVisible(neck);
      await tester.tap(neck);
      await tester.pumpAndSettle();
      expect(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.text('Neck'),
        ),
        findsOneWidget,
      );
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
    },
  );

  testWidgets(
    'History keeps alerts, profile, trend, table and indicators in order',
    (tester) async {
      await tester.pumpWidget(
        PulsysApp(api: overviewApi(persistent: true), clock: overviewClock),
      );
      await tester.pumpAndSettle();
      await _page(tester, 'history');
      final blocks = <Finder>[
        find.byKey(const ValueKey('history-alerts')),
        find.byType(ProfileSummary),
        find.byKey(const ValueKey('history-trend-plot')),
        find.byKey(const ValueKey('history-readings-table')),
        find.byKey(const ValueKey('history-indicators')),
      ];
      for (var index = 1; index < blocks.length; index++) {
        expect(
          tester.getTopLeft(blocks[index]).dy,
          greaterThanOrEqualTo(tester.getTopLeft(blocks[index - 1]).dy),
        );
      }
      expect(
        find.descendant(
          of: find.byType(ProfileSummary),
          matching: find.textContaining('Pulse · Stable'),
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets('confirmation and correction survive the three page flow', (
    tester,
  ) async {
    final api = FakePulsysApi(clock: overviewClock);
    api.enqueuePrepareWarnings(const <PulseWarning>[
      PulseWarning(code: PulseWarning.unusualValue, ruleId: 'fixture.warning'),
    ]);
    await tester.pumpWidget(PulsysApp(api: api, clock: overviewClock));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('overview-pulse-card')));
    await tester.pumpAndSettle();
    final bpm = find.byKey(const ValueKey('pulse-bpm-field'));
    await tester.enterText(bpm, '201');
    final save = find.byKey(const ValueKey('pulse-save-button'));
    await tester.ensureVisible(save);
    await tester.tap(save);
    await tester.pumpAndSettle();
    expect(find.text('Confirm pulse reading?'), findsOneWidget);
    expect(api.historySnapshot, isEmpty);
    await tester.tap(find.text('Confirm and save'));
    await tester.pumpAndSettle();
    final original = api.historySnapshot.single;
    final correct = find.byKey(ValueKey('pulse-correct-${original.recordId}'));
    await tester.ensureVisible(correct);
    await tester.tap(correct);
    await tester.pumpAndSettle();
    await tester.enterText(bpm, '78');
    await tester.ensureVisible(save);
    await tester.tap(save);
    await tester.pumpAndSettle();
    final effective = (await api.effectivePulseHistory()).single;
    expect(effective.replaces, original.recordId);
    await tester.pageBack();
    await tester.pumpAndSettle();
    await _page(tester, 'history');
    final row = find.byKey(ValueKey('history-reading-${effective.recordId}'));
    await tester.ensureVisible(row);
    expect(
      find.descendant(of: row, matching: find.text('78 bpm')),
      findsOneWidget,
    );
    expect(
      find.byKey(ValueKey('history-reading-${original.recordId}')),
      findsNothing,
    );
    await _page(tester, 'recommendations');
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(find.byType(PulseScreen), findsNothing);
    _expectPage(tester, 'today');
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('overview-pulse-card')),
        matching: find.text('78 bpm'),
      ),
      findsOneWidget,
    );
  });

  testWidgets('failed status can be retried while all Today blocks remain', (
    tester,
  ) async {
    final api = overviewApi()
      ..enqueueZoneStatusError(
        const ApiError(code: ApiErrorCodes.busy, message: 'fixture'),
      )
      ..enqueueHistoryError(
        const ApiError(code: ApiErrorCodes.busy, message: 'fixture'),
      );
    await tester.pumpWidget(PulsysApp(api: api, clock: overviewClock));
    await tester.pumpAndSettle();
    expect(find.byType(VitalCard), findsNWidgets(8));
    expect(find.byKey(const ValueKey('overview-context')), findsOneWidget);
    expect(find.byKey(const ValueKey('overview-assistant')), findsOneWidget);
    final retry = find.widgetWithText(
      OutlinedButton,
      'Status could not be refreshed. Retry',
    );
    expect(retry, findsOneWidget);
    await tester.tap(retry);
    await tester.pumpAndSettle();
    expect(retry, findsNothing);
    expect(find.byType(BodyPanel), findsOneWidget);
    expect(find.text('72 bpm'), findsOneWidget);
  });

  for (final (pulse, zone, showWarning)
      in <(EvaluationStatus, EvaluationStatus, bool)>[
        (EvaluationStatus.stable, EvaluationStatus.notEvaluated, false),
        (EvaluationStatus.change, EvaluationStatus.change, false),
        (EvaluationStatus.alarm, EvaluationStatus.notEvaluated, true),
        (EvaluationStatus.stable, EvaluationStatus.alarm, true),
      ]) {
    testWidgets('flight surgeons warning follows pulse $pulse and zone $zone', (
      tester,
    ) async {
      await tester.pumpWidget(
        PulsysApp(
          api: overviewApi(
            pulseStatus: pulse,
            zoneStatus: zone,
            escalation: EscalationLevel.urgent,
          ),
          clock: overviewClock,
        ),
      );
      await tester.pumpAndSettle();
      await _page(tester, 'recommendations');
      expect(
        find.text('Contact flight surgeons.'),
        showWarning ? findsOneWidget : findsNothing,
      );
      expect(find.text('This is guidance, not a diagnosis.'), findsOneWidget);
      expect(find.text('Head · Not evaluated'), findsOneWidget);
    });
  }

  testWidgets('placeholders and page controls remain usable at 320 dp and 2x', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(320, 1000);
    tester.view.devicePixelRatio = 1;
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    await tester.pumpWidget(
      PulsysApp(api: overviewApi(), clock: overviewClock),
    );
    await tester.pumpAndSettle();
    final assistant = find.byKey(const ValueKey('overview-assistant'));
    await tester.ensureVisible(assistant);
    await tester.pumpAndSettle();
    final field = find.descendant(
      of: assistant,
      matching: find.byType(TextField),
    );
    expect(tester.widget<TextField>(field).enabled, isFalse);
    for (final (page, label) in <(String, String)>[
      ('history', 'History'),
      ('today', 'Today'),
      ('recommendations', 'Recommendations'),
    ]) {
      final button = find.byKey(ValueKey('nav-$page'));
      final node = tester.getSemantics(button);
      expect(node.rect.width, greaterThanOrEqualTo(48));
      expect(node.rect.height, greaterThanOrEqualTo(48));
      expect(node.getSemanticsData().label, label);
      await _page(tester, page);
      expect(
        tester.semantics.find(button),
        isSemantics(
          isSelected: true,
          label: label,
          isButton: true,
          hasTapAction: true,
        ),
      );
    }
    await _page(tester, 'today');
    for (final (indicator, label) in <(String, String)>[
      ('pulse', 'Pulse, 72 bpm, Stable'),
      ('pressure', 'Pressure, No data'),
      ('oxygen', 'SpO₂, No data'),
      ('mood', 'Mood, No data'),
      ('temperature', 'Temperature, No data'),
      ('glucose', 'Glucose, No data'),
      ('sleep', 'Sleep, No data'),
      ('nutrition', 'Nutrition, No data'),
    ]) {
      final card = find.byKey(ValueKey('overview-$indicator-card'));
      await tester.ensureVisible(card);
      await tester.pumpAndSettle();
      expect(tester.getSemantics(card).getSemanticsData().label, label);
    }
    expect(tester.takeException(), isNull);
  });
}

Future<void> _page(WidgetTester tester, String name) async {
  await tester.tap(find.byKey(ValueKey<String>('nav-$name')));
  await tester.pumpAndSettle();
  _expectPage(tester, name);
}

void _expectPage(WidgetTester tester, String name) {
  expect(
    tester.semantics.find(find.byKey(ValueKey('nav-$name'))),
    isSemantics(isSelected: true),
  );
  expect(
    find.byKey(PageStorageKey<String>('$name-scroll')).hitTestable(),
    findsOneWidget,
  );
}
