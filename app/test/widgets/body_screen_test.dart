import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/design/theme.dart';
import 'package:pulsys/features/body/body_screen.dart';
import 'package:pulsys/l10n/generated/app_localizations.dart';
import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';

const _names = <String, String>{
  'head': 'Head',
  'neck': 'Neck',
  'chest': 'Chest',
  'abdomen': 'Abdomen',
  'right_arm': 'Right arm',
  'left_arm': 'Left arm',
  'right_hand': 'Right hand',
  'left_hand': 'Left hand',
  'right_leg': 'Right leg',
  'left_leg': 'Left leg',
  'right_foot': 'Right foot',
  'left_foot': 'Left foot',
  'back': 'Back',
  'other': 'Other',
};

Widget _app(PulsysApi api) => MaterialApp(
  theme: PulsysTheme.dark(reducedMotion: true),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  home: BodyScreen(api: api),
);

void _viewport(WidgetTester tester, {double scale = 1, double width = 390}) {
  tester.view.physicalSize = Size(width, 900);
  tester.view.devicePixelRatio = 1;
  tester.platformDispatcher.textScaleFactorTestValue = scale;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
}

FakePulsysApi _sample({bool demo = false}) => FakePulsysApi(
  demoTarget: demo ? const DemoTarget(generation: 'test') : null,
  initialZones: [
    for (final zone in BodyZone.values)
      if (zone != BodyZone.other)
        ZoneState(
          zoneId: zone,
          status: switch (zone) {
            BodyZone.neck => EvaluationStatus.change,
            BodyZone.leftArm => EvaluationStatus.alarm,
            BodyZone.back => EvaluationStatus.stable,
            _ => EvaluationStatus.notEvaluated,
          },
          symptoms: zone == BodyZone.neck
              ? [
                  SymptomEvaluation(
                    status: EvaluationStatus.notEvaluated,
                    reason: NotEvaluatedReason.stale,
                    evaluatedAtUtc: DateTime.utc(2026, 11, 16, 8, 42, 10),
                    ruleId: 'fixture.first',
                    ruleVersion: 'v1-test',
                    sourceKind: SourceKind.published,
                    sourceRef: 'fixture-reference',
                    inputRecordIds: const [],
                    inputEventIds: const [],
                  ),
                  SymptomEvaluation(
                    status: EvaluationStatus.change,
                    evaluatedAtUtc: DateTime.utc(2026, 11, 17, 10),
                    ruleId: 'fixture.second',
                    ruleVersion: 'v2-test',
                    sourceKind: SourceKind.demoHeuristic,
                    inputRecordIds: const [],
                    inputEventIds: const [],
                  ),
                ]
              : [],
        ),
  ],
);

String _expectedStatus(String id) => switch (id) {
  'neck' => 'Change',
  'left_arm' => 'Alarm',
  'back' => 'Stable',
  'other' => 'Status unavailable',
  _ => 'Not evaluated',
};

Future<void> _tap(WidgetTester tester, Finder target) async {
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  await tester.tap(target);
  await tester.pumpAndSettle();
}

Finder _insideDetail(String text) =>
    find.descendant(of: find.byType(AlertDialog), matching: find.text(text));

void main() {
  testWidgets('front figure labels astronaut sides at 320 dp and 2x text', (
    tester,
  ) async {
    _viewport(tester, scale: 2, width: 320);
    await tester.pumpWidget(_app(_sample()));
    await tester.pumpAndSettle();

    final right = find.text('R');
    final left = find.text('L');
    expect(right, findsOneWidget);
    expect(left, findsOneWidget);
    expect(
      tester.semantics.find(right),
      isSemantics(label: 'Astronaut’s right'),
    );
    expect(tester.semantics.find(left), isSemantics(label: 'Astronaut’s left'));
    final rightPosition = tester.getCenter(right);
    final leftPosition = tester.getCenter(left);
    expect(rightPosition.dx, lessThan(leftPosition.dx));
    expect(rightPosition.dy, leftPosition.dy);
    expect(
      rightPosition.dx,
      lessThan(
        tester.getCenter(find.byKey(const ValueKey('figure-right_arm'))).dx,
      ),
    );
    expect(
      leftPosition.dx,
      greaterThan(
        tester.getCenter(find.byKey(const ValueKey('figure-left_arm'))).dx,
      ),
    );
    expect(right.hitTestable(), findsOneWidget);
    expect(left.hitTestable(), findsOneWidget);
    expect(MediaQuery.textScalerOf(tester.element(right)).scale(14), 28);
    expect(tester.takeException(), isNull);
  });

  testWidgets('all 14 targets in each view announce the zone and Rust status', (
    tester,
  ) async {
    _viewport(tester);
    final api = _sample();
    await tester.pumpWidget(_app(api));
    await tester.pumpAndSettle();

    for (final view in ['figure', 'list']) {
      if (view == 'list') {
        await _tap(tester, find.text('Show zone list'));
      }
      for (final entry in _names.entries) {
        final target = find.byKey(ValueKey('$view-${entry.key}'));
        await tester.ensureVisible(target);
        await tester.pumpAndSettle();
        final label = '${entry.value}, ${_expectedStatus(entry.key)}';
        final node = tester.semantics.find(target);
        expect(
          node,
          isSemantics(label: label, isButton: true, hasTapAction: true),
        );
        expect(node.rect.width, greaterThanOrEqualTo(48));
        expect(node.rect.height, greaterThanOrEqualTo(48));
        tester.semantics.tap(find.semantics.byLabel(label));
        await tester.pumpAndSettle();
        expect(_insideDetail(entry.value), findsOneWidget);
        expect(_insideDetail(_expectedStatus(entry.key)), findsWidgets);
        await _tap(tester, _insideDetail('Close'));
        expect(tester.semantics.find(target), isSemantics(isSelected: true));
      }
    }
    expect(api.zoneStatusCalls, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets('touching the painted neck opens the same evidence as its row', (
    tester,
  ) async {
    _viewport(tester);
    await tester.pumpWidget(_app(_sample()));
    await tester.pumpAndSettle();
    final canvas = find.byKey(const ValueKey('body-canvas'));
    await tester.ensureVisible(canvas);
    final rect = tester.getRect(canvas);
    await tester.tapAt(
      rect.topLeft + Offset(rect.width * .5, rect.height * .177),
    );
    await tester.pumpAndSettle();

    for (final view in ['figure', 'list']) {
      if (view == 'list') {
        await _tap(tester, find.text('Show zone list'));
        await _tap(tester, find.byKey(const ValueKey('list-neck')));
      }
      expect(_insideDetail('Neck'), findsOneWidget);
      expect(_insideDetail('Change'), findsNWidgets(2));
      expect(_insideDetail('Not evaluated'), findsOneWidget);
      expect(_insideDetail('Stale reading'), findsOneWidget);
      expect(_insideDetail('18 Nov 2026, 12:00:00'), findsOneWidget);
      expect(_insideDetail('16 Nov 2026, 08:42:10'), findsOneWidget);
      expect(_insideDetail('17 Nov 2026, 10:00:00'), findsOneWidget);
      for (final field in [
        'fixture.first',
        'v1-test',
        'fixture.second',
        'v2-test',
        'Published source',
        'fixture-reference',
        'Demo rule, not medical advice.',
      ]) {
        expect(_insideDetail(field), findsOneWidget);
      }
      expect(_insideDetail('Not provided'), findsNWidgets(2));
      await _tap(tester, _insideDetail('Close'));
    }
  });

  testWidgets('actual left and right anatomy maps to the astronaut’s side', (
    tester,
  ) async {
    _viewport(tester);
    await tester.pumpWidget(_app(_sample()));
    await tester.pumpAndSettle();
    final canvas = find.byKey(const ValueKey('body-canvas'));
    await tester.ensureVisible(canvas);
    final rect = tester.getRect(canvas);
    for (final (fraction, name) in [(0.38, 'Right leg'), (0.62, 'Left leg')]) {
      await tester.tapAt(
        rect.topLeft + Offset(rect.width * fraction, rect.height * .80),
      );
      await tester.pumpAndSettle();
      expect(_insideDetail(name), findsOneWidget);
      await _tap(tester, _insideDetail('Close'));
    }
  });

  testWidgets('empty and absent zones retain distinct returned status', (
    tester,
  ) async {
    _viewport(tester);
    await tester.pumpWidget(_app(_sample()));
    await tester.pumpAndSettle();
    for (final id in ['back', 'other']) {
      await _tap(tester, find.byKey(ValueKey('figure-$id')));
      expect(_insideDetail(_expectedStatus(id)), findsOneWidget);
      expect(
        _insideDetail('No symptom evaluation was provided for this zone.'),
        findsOneWidget,
      );
      expect(_insideDetail('Not provided'), findsWidgets);
      expect(_insideDetail('Demo rule, not medical advice.'), findsNothing);
      await _tap(tester, _insideDetail('Close'));
    }
    expect(find.text('DEMO'), findsNothing);
  });

  testWidgets('loading, error, retry and DEMO follow the API response', (
    tester,
  ) async {
    final api =
        FakePulsysApi(
          latency: const Duration(milliseconds: 80),
          demoTarget: const DemoTarget(generation: 'test'),
        )..enqueueZoneStatusError(
          const ApiError(code: ApiErrorCodes.busy, message: 'fixture'),
        );
    await tester.pumpWidget(_app(api));
    expect(find.text('Loading body zones…'), findsOneWidget);
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump();
    expect(find.text('Body zones could not be loaded.'), findsOneWidget);
    expect(find.byKey(const ValueKey('figure-head')), findsNothing);
    await tester.tap(find.text('Try again'));
    await tester.pump();
    expect(find.text('Loading body zones…'), findsOneWidget);
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pumpAndSettle();
    expect(api.zoneStatusCalls, 2);
    expect(find.text('DEMO'), findsOneWidget);
    await _tap(tester, find.byKey(const ValueKey('figure-head')));
    expect(_insideDetail('DEMO'), findsOneWidget);
    await _tap(tester, _insideDetail('Close'));
    await _tap(tester, find.text('Show zone list'));
    expect(find.text('DEMO'), findsOneWidget);
  });

  testWidgets(
    'late results from a previous API cannot replace the new profile',
    (tester) async {
      final previous = FakePulsysApi(
        latency: const Duration(seconds: 1),
        demoTarget: const DemoTarget(generation: 'previous'),
      );
      final current = _sample();
      await tester.pumpWidget(_app(previous));
      expect(find.text('Loading body zones…'), findsOneWidget);
      await tester.pumpWidget(_app(current));
      await tester.pumpAndSettle();
      await tester.pump(const Duration(seconds: 1));
      await tester.pumpAndSettle();
      expect(previous.zoneStatusCalls, 1);
      expect(current.zoneStatusCalls, 1);
      expect(find.text('DEMO'), findsNothing);
      await _tap(tester, find.byKey(const ValueKey('figure-neck')));
      expect(_insideDetail('fixture.first'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('figure, list and detail wrap at 2x on a narrow dark screen', (
    tester,
  ) async {
    _viewport(tester, scale: 2, width: 320);
    await tester.pumpWidget(_app(_sample(demo: true)));
    await tester.pumpAndSettle();
    final context = tester.element(find.byType(BodyScreen));
    expect(Theme.of(context).brightness, Brightness.dark);
    expect(MediaQuery.textScalerOf(context).scale(16), 32);
    expect(tester.takeException(), isNull);
    await _tap(tester, find.byKey(const ValueKey('figure-other')));
    await tester.ensureVisible(_insideDetail('Rule ID'));
    expect(tester.takeException(), isNull);
    await _tap(tester, _insideDetail('Close'));
    await _tap(tester, find.text('Show zone list'));
    await _tap(tester, find.byKey(const ValueKey('list-neck')));
    await tester.ensureVisible(_insideDetail('v2-test'));
    expect(_insideDetail('DEMO'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
