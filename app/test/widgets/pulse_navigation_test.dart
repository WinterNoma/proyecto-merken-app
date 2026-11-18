import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/app.dart';
import 'package:pulsys/features/pulse/pulse_screen.dart';
import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';
import '../support/pulse_app.dart';

void main() {
  testWidgets('keeps the pulse route visible while confirmation is delayed', (
    tester,
  ) async {
    const delay = Duration(milliseconds: 200);
    final gate = Completer<void>();
    final api = FakePulsysApi(latency: delay)..confirmGate = gate;
    api.enqueuePrepareWarnings(const <PulseWarning>[
      PulseWarning(code: PulseWarning.unusualValue, ruleId: 'fixture.warning'),
    ]);
    await tester.pumpWidget(PulsysApp(api: api));
    await tester.pump(delay);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('overview-pulse-card')));
    await tester.pump(delay);
    await tester.pumpAndSettle();
    await tester.pump(delay);
    await tester.pumpAndSettle();

    await tester.enterText(
      find.byKey(const ValueKey<String>('pulse-bpm-field')),
      '201',
    );
    FocusManager.instance.primaryFocus?.unfocus();
    final save = find.byKey(const ValueKey<String>('pulse-save-button'));
    await tester.ensureVisible(save);
    await tester.tap(save);
    await tester.pump();
    await tester.binding.handlePopRoute();
    await tester.pump();
    expect(find.byType(PulseScreen), findsOneWidget);

    await tester.pump(delay);
    await tester.pump(delay);
    expect(api.confirmedActions, hasLength(1));
    await tester.binding.handlePopRoute();
    await tester.pump();
    expect(find.byType(PulseScreen), findsOneWidget);
    expect(
      find.byKey(const ValueKey('nav-history')).hitTestable(),
      findsNothing,
    );

    gate.complete();
    await tester.pumpAndSettle();
    expect(find.text('Confirm pulse reading?'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    await tester.pageBack();
    await tester.pumpAndSettle();
    expect(find.byType(PulseScreen), findsNothing);
    expect(api.historySnapshot, isEmpty);
  });

  testWidgets('allows returning to Today after recovery is blocked', (
    tester,
  ) async {
    final api = FakePulsysApi()
      ..recovery = const PulseRecoveryBlocked(
        ApiError(code: ApiErrorCodes.invalidSchema, message: 'fixture'),
      );
    await tester.pumpWidget(PulsysApp(api: api));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('overview-pulse-card')));
    await tester.pumpAndSettle();
    final form = tester.widget<PulseScreen>(find.byType(PulseScreen));
    expect(form.controller.canEdit, isFalse);
    await tester.pageBack();
    await tester.pumpAndSettle();
    expect(find.byType(PulseScreen), findsNothing);
  });

  for (final evaluatedCount in <int>[0, 3]) {
    testWidgets('counts $evaluatedCount evaluated zones from Rust states', (
      tester,
    ) async {
      const evaluatedStatuses = <EvaluationStatus>[
        EvaluationStatus.stable,
        EvaluationStatus.change,
        EvaluationStatus.alarm,
      ];
      final api = evaluatedCount == 0
          ? FakePulsysApi()
          : FakePulsysApi(
              initialZones: <ZoneState>[
                for (final (index, zone) in BodyZone.values.indexed)
                  ZoneState(
                    zoneId: zone,
                    status: index < evaluatedCount
                        ? evaluatedStatuses[index]
                        : EvaluationStatus.notEvaluated,
                    symptoms: const <SymptomEvaluation>[],
                  ),
              ],
            );
      await tester.pumpWidget(pulseTestApp(api));
      await tester.pumpAndSettle();
      expect(
        find.text('Body zones evaluated: $evaluatedCount'),
        findsOneWidget,
      );
      expect(find.text('Body zones evaluated: 14'), findsNothing);
    });
  }
}
