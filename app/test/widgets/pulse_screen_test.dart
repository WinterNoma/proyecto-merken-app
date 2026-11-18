import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';
import '../support/pulse_app.dart';

Future<void> _scrollAndTap(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder);
  await tester.pumpAndSettle();
  await tester.tap(finder);
}

void main() {
  const bpmFieldKey = ValueKey<String>('pulse-bpm-field');
  const timeFieldKey = ValueKey<String>('pulse-time-field');
  const noteFieldKey = ValueKey<String>('pulse-note-field');
  const saveButtonKey = ValueKey<String>('pulse-save-button');

  testWidgets('saves a normal pulse and refreshes history', (
    WidgetTester tester,
  ) async {
    final api = FakePulsysApi(clock: () => DateTime.utc(2026, 11, 18, 14, 30));

    await tester.pumpWidget(pulseTestApp(api));
    await tester.pumpAndSettle();

    await tester.enterText(find.byKey(bpmFieldKey), '72');
    await tester.enterText(find.byKey(noteFieldKey), 'Resting');
    await tester.tap(find.byKey(saveButtonKey));
    await tester.pumpAndSettle();

    expect(api.preparedInputs, hasLength(1));
    expect(api.preparedInputs.single.bpm, Bpm(72));
    expect(api.preparedInputs.single.note, 'Resting');
    expect(api.confirmedActions, hasLength(1));
    expect(api.confirmedActions.single.key, api.preparedInputs.single.key);

    expect(find.text('Saved locally'), findsOneWidget);
    expect(
      find.text('Synchronization pending. Ground receipt is not confirmed.'),
      findsOneWidget,
    );
    expect(find.text('72 bpm'), findsOneWidget);
    expect(find.text('18 Nov 2026, 14:30 UTC'), findsOneWidget);
  });

  testWidgets(
    'warning requires confirmation and does not save until confirmed',
    (WidgetTester tester) async {
      final api = FakePulsysApi();
      api.enqueuePrepareWarnings(const <PulseWarning>[
        PulseWarning(
          code: PulseWarning.unusualValue,
          ruleId: 'plaus.heart_rate',
        ),
      ]);

      await tester.pumpWidget(pulseTestApp(api));
      await tester.pumpAndSettle();

      await tester.enterText(find.byKey(bpmFieldKey), '201');
      await tester.tap(find.byKey(saveButtonKey));
      await tester.pumpAndSettle();

      expect(find.text('Confirm pulse reading?'), findsOneWidget);
      expect(
        find.text('This pulse value is outside the typical range.'),
        findsOneWidget,
      );
      expect(api.historySnapshot, isEmpty);

      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();

      expect(find.text('Confirm pulse reading?'), findsNothing);
      expect(api.historySnapshot, isEmpty);
      expect(find.text('Saved locally'), findsNothing);

      api.enqueuePrepareWarnings(const <PulseWarning>[
        PulseWarning(
          code: PulseWarning.unusualValue,
          ruleId: 'plaus.heart_rate',
        ),
      ]);

      await tester.tap(find.byKey(saveButtonKey));
      await tester.pumpAndSettle();
      expect(find.text('Confirm pulse reading?'), findsOneWidget);

      await tester.tap(find.text('Confirm and save'));
      await tester.pumpAndSettle();

      expect(api.historySnapshot, hasLength(1));
      expect(api.historySnapshot.single.bpm, Bpm(201));
      expect(api.confirmedActions, hasLength(3));
      expect(api.confirmedActions[1].key, api.confirmedActions[2].key);
      expect(api.confirmedActions[1].approved, isFalse);
      expect(api.confirmedActions[2].approved, isTrue);
      expect(find.text('Saved locally'), findsOneWidget);
    },
  );

  testWidgets('retries with the same key without creating duplicates', (
    WidgetTester tester,
  ) async {
    final api = FakePulsysApi();
    api.enqueueConfirmError(
      const ApiError(
        code: ApiErrorCodes.storageError,
        message: 'Simulated lost response after save',
      ),
      saveBeforeThrowing: true,
    );

    await tester.pumpWidget(pulseTestApp(api));
    await tester.pumpAndSettle();

    await tester.enterText(find.byKey(bpmFieldKey), '68');
    await tester.tap(find.byKey(saveButtonKey));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey<String>('pulse-general-error')),
      findsOneWidget,
    );
    expect(find.text('Saved locally'), findsNothing);
    expect(api.historySnapshot, hasLength(1));

    await tester.tap(find.byKey(saveButtonKey));
    await tester.pumpAndSettle();

    expect(api.confirmedActions, hasLength(2));
    expect(api.confirmedActions[0].key, api.confirmedActions[1].key);
    expect(api.historySnapshot, hasLength(1));
    expect(find.text('Saved locally'), findsOneWidget);
  });

  testWidgets('displays validation errors on the matching form field', (
    WidgetTester tester,
  ) async {
    final api = FakePulsysApi();

    await tester.pumpWidget(pulseTestApp(api));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(saveButtonKey));
    await tester.pumpAndSettle();

    expect(find.text('Enter a pulse in bpm.'), findsOneWidget);
    expect(api.preparedInputs, isEmpty);

    await tester.enterText(find.byKey(bpmFieldKey), '75');
    await tester.enterText(find.byKey(timeFieldKey), 'not-a-date');
    await _scrollAndTap(tester, find.byKey(saveButtonKey));
    await tester.pumpAndSettle();

    expect(
      find.text('Enter a valid UTC time (YYYY-MM-DD HH:MM).'),
      findsOneWidget,
    );
    expect(api.preparedInputs, isEmpty);

    await tester.enterText(find.byKey(timeFieldKey), '');
    api.enqueuePrepareError(
      const ApiError(
        code: ApiErrorCodes.invalidValue,
        field: ApiFields.bpm,
        message: 'Value must be finite and greater than zero',
      ),
    );

    await _scrollAndTap(tester, find.byKey(saveButtonKey));
    await tester.pumpAndSettle();

    expect(find.text('Enter a positive number in bpm.'), findsOneWidget);
    expect(api.historySnapshot, isEmpty);
  });

  testWidgets('blocks duplicate submissions while saving', (
    WidgetTester tester,
  ) async {
    final gate = Completer<void>();
    final api = FakePulsysApi()..confirmGate = gate;

    await tester.pumpWidget(pulseTestApp(api));
    await tester.pumpAndSettle();

    await tester.enterText(find.byKey(bpmFieldKey), '70');
    await _scrollAndTap(tester, find.byKey(saveButtonKey));
    await tester.pump();

    expect(find.text('Saving…'), findsOneWidget);

    await tester.tap(find.byKey(saveButtonKey));
    await tester.pump();

    expect(api.confirmedActions, hasLength(1));

    gate.complete();
    await tester.pumpAndSettle();

    expect(find.text('Saved locally'), findsOneWidget);
  });

  testWidgets('exposes accessible semantic labels for fields and action', (
    WidgetTester tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      final api = FakePulsysApi();
      await tester.pumpWidget(pulseTestApp(api));
      await tester.pumpAndSettle();

      expect(find.bySemanticsLabel('Pulse (bpm)'), findsOneWidget);
      expect(
        find.bySemanticsLabel('Measurement time (UTC, optional)'),
        findsOneWidget,
      );
      expect(find.bySemanticsLabel('Note (optional)'), findsOneWidget);
      expect(find.bySemanticsLabel('Save reading'), findsOneWidget);
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('renders form and dialog at 2.0x text without overflow', (
    WidgetTester tester,
  ) async {
    tester.view.physicalSize = const Size(390, 844);
    tester.view.devicePixelRatio = 1.0;
    tester.platformDispatcher.textScaleFactorTestValue = 2.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);

    final api = FakePulsysApi(
      initialHistory: <PulsePoint>[
        PulsePoint(
          recordId: 'rec-0',
          rootId: 'rec-0',
          bpm: Bpm(66),
          source: RecordSource.form,
          measuredAtUtc: DateTime.utc(2026, 11, 18, 10, 0),
          loggedAtUtc: DateTime.utc(2026, 11, 18, 10, 1),
          isDemo: false,
        ),
      ],
    );
    api.enqueuePrepareWarnings(const <PulseWarning>[
      PulseWarning(code: PulseWarning.unusualValue, ruleId: 'plaus.heart_rate'),
    ]);

    await tester.pumpWidget(pulseTestApp(api));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);

    await tester.enterText(find.byKey(bpmFieldKey), '205');
    await tester.pumpAndSettle();
    await _scrollAndTap(tester, find.byKey(saveButtonKey));
    await tester.pumpAndSettle();

    expect(find.text('Confirm pulse reading?'), findsOneWidget);
    expect(tester.takeException(), isNull);

    await _scrollAndTap(tester, find.text('Confirm and save'));
    await tester.pumpAndSettle();

    expect(find.text('Saved locally'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
