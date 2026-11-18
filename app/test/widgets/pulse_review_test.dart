import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/design/theme.dart';
import 'package:pulsys/features/pulse/pulse_controller.dart';
import 'package:pulsys/features/pulse/pulse_screen.dart';
import 'package:pulsys/l10n/generated/app_localizations.dart';
import 'package:pulsys/main.dart' show PulsysStartupFailure;
import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';
import '../support/pulse_input.dart';

Widget _screen(PulseController controller) => MaterialApp(
  theme: PulsysTheme.dark(reducedMotion: true),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  home: PulseScreen(controller: controller),
);

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder);
  await tester.pumpAndSettle();
  await tester.tap(finder);
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('shows consecutive confirmations even after warnings disappear', (
    tester,
  ) async {
    final api = FakePulsysApi();
    final now = DateTime.utc(2026, 11, 18, 12);
    PreparedPulse action(List<PulseWarning> warnings) => PreparedPulse(
      key: 'intent',
      bpm: Bpm(0.04),
      measuredAtUtc: now,
      target: const LocalPulseTarget(storeId: 'fake-store-1'),
      warnings: warnings,
      confirmation: PulseConfirmation.requiredReview,
    );
    api.enqueueConfirmation(
      action(const <PulseWarning>[
        PulseWarning(code: PulseWarning.unusualValue, ruleId: 'fixture'),
      ]),
    );
    api.enqueueConfirmation(action(const <PulseWarning>[]));
    final controller = PulseController(api: api, keyGenerator: () => 'intent');
    addTearDown(controller.dispose);
    await controller.loadInitial();
    await tester.pumpWidget(_screen(controller));
    await tester.enterText(
      find.byKey(const ValueKey<String>('pulse-bpm-field')),
      '0.04',
    );
    await _tap(tester, find.byKey(const ValueKey<String>('pulse-save-button')));
    expect(find.text('Confirm pulse reading?'), findsOneWidget);
    expect(find.textContaining('0.04 bpm · Measured'), findsOneWidget);
    expect(api.historySnapshot, isEmpty);

    await _tap(tester, find.text('Confirm and save'));
    expect(find.text('Confirm pulse reading?'), findsOneWidget);
    expect(
      find.text('This pulse value is outside the typical range.'),
      findsNothing,
    );
    expect(controller.pendingConfirmation?.requiresConfirmation, isTrue);
    expect(api.historySnapshot, isEmpty);
    await _tap(tester, find.text('Confirm and save'));
    expect(find.text('Confirm pulse reading?'), findsNothing);
    expect(api.historySnapshot, hasLength(1));
    expect(find.textContaining('0.04 bpm · Measured'), findsOneWidget);
    expect(api.confirmedActions.map((a) => a.key).toSet(), <String>{'intent'});
    expect(tester.takeException(), isNull);
  });

  testWidgets('cached status and history show failure with separate retries', (
    tester,
  ) async {
    final api = FakePulsysApi();
    final controller = PulseController(api: api);
    addTearDown(controller.dispose);
    await controller.loadInitial();
    controller.updateBpm('72');
    await controller.submit();
    api.enqueueHistoryError(
      const ApiError(code: 'storage_error', message: 'History'),
    );
    api.enqueueZoneStatusError(
      const ApiError(code: 'storage_error', message: 'Status'),
    );
    await controller.refreshOverview();
    await tester.pumpWidget(_screen(controller));
    await tester.pumpAndSettle();

    expect(find.text('Last available pulse status'), findsOneWidget);
    expect(find.text('Current pulse status'), findsNothing);
    expect(find.text('Last available pulse history'), findsOneWidget);
    expect(find.text('Saved locally'), findsOneWidget);
    expect(controller.history, hasLength(1));
    final statusCalls = api.zoneStatusCalls;
    await _tap(
      tester,
      find.descendant(
        of: find.byKey(const ValueKey<String>('pulse-history-retry')),
        matching: find.byType(OutlinedButton),
      ),
    );
    expect(api.zoneStatusCalls, statusCalls);
    expect(find.text('Last available pulse status'), findsOneWidget);
    await _tap(tester, find.text('Retry pulse status'));
    expect(find.text('Current pulse status'), findsOneWidget);
    expect(find.text('Saved locally'), findsOneWidget);
  });

  testWidgets(
    'history correction submits replaces and keeps one effective point',
    (tester) async {
      final api = FakePulsysApi();
      final controller = PulseController(api: api);
      addTearDown(controller.dispose);
      await controller.loadInitial();
      controller.updateBpm('72');
      await controller.submit();
      final original = controller.lastReceipt!;
      await tester.pumpWidget(_screen(controller));
      await tester.pumpAndSettle();
      await _tap(
        tester,
        find.byKey(ValueKey<String>('pulse-correct-${original.recordId}')),
      );
      expect(controller.replaces, original.recordId);
      await tester.ensureVisible(
        find.byKey(const ValueKey<String>('pulse-bpm-field')),
      );
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byKey(const ValueKey<String>('pulse-bpm-field')),
        '74',
      );
      await _tap(
        tester,
        find.byKey(const ValueKey<String>('pulse-save-button')),
      );

      expect(api.preparedInputs.last.replaces, original.recordId);
      expect(controller.history, hasLength(1));
      expect(controller.history.single.bpm, Bpm(74));
      expect(controller.lastReceipt?.rootId, original.rootId);
      expect(api.historySnapshot, hasLength(2));
      expect(find.text('Corrected'), findsOneWidget);
    },
  );

  testWidgets('unsupported recovery is visible and prevents new submissions', (
    tester,
  ) async {
    final api = FakePulsysApi()
      ..recovery = const PulseRecoveryBlocked(
        ApiError(
          code: ApiErrorCodes.unsupportedActionVersion,
          field: 'action.version',
          message: 'Future version',
        ),
      );
    final controller = PulseController(api: api);
    addTearDown(controller.dispose);
    await controller.loadInitial();
    await tester.pumpWidget(_screen(controller));
    await tester.pumpAndSettle();
    expect(
      find.text(
        'A pending reading requires a newer app version. It has been kept on this device.',
      ),
      findsOneWidget,
    );
    final save = tester.widget<FilledButton>(
      find.byKey(const ValueKey<String>('pulse-save-button')),
    );
    expect(save.onPressed, isNull);
    expect(api.confirmedActions, isEmpty);
    await _tap(tester, find.text('Retry recovery'));
    expect(
      controller.recoveryError?.code,
      ApiErrorCodes.unsupportedActionVersion,
    );
  });

  testWidgets('startup failure uses the localization catalog', (tester) async {
    await tester.pumpWidget(const PulsysStartupFailure());
    await tester.pumpAndSettle();
    final context = tester.element(find.byType(Scaffold));
    expect(
      find.text(AppLocalizations.of(context).startupStorageError),
      findsOneWidget,
    );
  });
  testWidgets('recovered reading opens a decision with its original key', (
    tester,
  ) async {
    final api = FakePulsysApi();
    final prepared = PreparedPulse(
      key: 'recovered-intent',
      bpm: Bpm(73.5),
      measuredAtUtc: DateTime.utc(2026, 11, 18, 12),
      target: const LocalPulseTarget(storeId: 'fake-store-1'),
      warnings: const <PulseWarning>[],
      confirmation: PulseConfirmation.requiredReview,
    );
    api.recovery = RecoveredPulse(prepared);
    final controller = PulseController(api: api);
    addTearDown(controller.dispose);
    await controller.loadInitial();
    await tester.pumpWidget(_screen(controller));
    await tester.pumpAndSettle();
    expect(find.text('Confirm pulse reading?'), findsOneWidget);
    expect(find.textContaining('73.5 bpm · Measured'), findsOneWidget);
    expect(controller.currentIntentKey, 'recovered-intent');
    expect(api.historySnapshot, isEmpty);
    await _tap(tester, find.text('Cancel'));
    expect(api.cancelledActions.single.key, 'recovered-intent');
    expect(api.historySnapshot, isEmpty);
    expect(
      tester
          .widget<TextField>(
            find.byKey(const ValueKey<String>('pulse-bpm-field')),
          )
          .controller
          ?.text,
      '73.5',
    );
  });
  testWidgets(
    'correction refocuses a cached editor without the keyboard stub',
    (tester) async {
      final api = FakePulsysApi();
      const warnings = <PulseWarning>[
        PulseWarning(
          code: PulseWarning.unusualValue,
          ruleId: 'plaus.heart_rate',
        ),
      ];
      api.enqueuePrepareWarnings(warnings);
      final controller = PulseController(api: api);
      addTearDown(controller.dispose);
      await controller.loadInitial();
      await tester.pumpWidget(_screen(controller));
      await tester.pumpAndSettle();
      final bpm = find.byKey(const ValueKey<String>('pulse-bpm-field'));
      final save = find.byKey(const ValueKey<String>('pulse-save-button'));
      final editable = tester.state<EditableTextState>(
        find.descendant(of: bpm, matching: find.byType(EditableText)),
      );
      tester.testTextInput.unregister();
      try {
        await enterPulseBpm(tester, bpm: '201');
        expect(controller.bpmText, '201');
        expect(tester.binding.focusedEditable, same(editable));
        FocusManager.instance.primaryFocus?.unfocus();
        await tester.pump();
        expect(editable.widget.focusNode.hasFocus, isFalse);
        expect(tester.binding.focusedEditable, same(editable));
        await _tap(tester, save);
        expect(find.text('Confirm pulse reading?'), findsOneWidget);
        expect(api.historySnapshot, isEmpty);
        await _tap(tester, find.text('Confirm and save'));
        final original = controller.history.single;
        final originalKey = controller.currentIntentKey;
        expect(controller.phase, PulseFormPhase.saved);
        expect(tester.widget<FilledButton>(save).onPressed, isNull);
        await _tap(
          tester,
          find.byKey(ValueKey<String>('pulse-correct-${original.recordId}')),
        );
        expect(controller.phase, PulseFormPhase.editing);
        expect(controller.canEdit, isTrue);
        expect(controller.canSubmit, isTrue);
        expect(controller.replaces, original.recordId);
        expect(controller.currentIntentKey, isNot(originalKey));
        expect(tester.widget<TextField>(bpm).enabled, isTrue);
        expect(editable.widget.readOnly, isFalse);
        expect(editable.widget.focusNode.hasFocus, isFalse);
        expect(tester.binding.focusedEditable, same(editable));

        await enterPulseBpm(tester, bpm: '202');
        expect(editable.widget.focusNode.hasFocus, isTrue);
        expect(controller.bpmText, '202');
        await controller.refreshOverview();
        await tester.pumpAndSettle();
        expect(editable.widget.controller.text, '202');
        expect(controller.bpmText, '202');
        expect(controller.replaces, original.recordId);

        api.enqueuePrepareWarnings(warnings);
        FocusManager.instance.primaryFocus?.unfocus();
        await tester.pump();
        await _tap(tester, save);
        expect(find.text('Confirm pulse reading?'), findsOneWidget);
        expect(controller.history.single.recordId, original.recordId);
        expect(api.historySnapshot, hasLength(1));
        await _tap(tester, find.text('Confirm and save'));
        final corrected = controller.history.single;
        expect(corrected.bpm, Bpm(202));
        expect(corrected.replaces, original.recordId);
        expect(corrected.rootId, original.rootId);
        expect(corrected.recordId, isNot(original.recordId));
        expect(api.preparedInputs.last.bpm, Bpm(202));
        expect(api.preparedInputs.last.replaces, original.recordId);
        expect(api.historySnapshot, hasLength(2));
        expect(find.text('202 bpm'), findsOneWidget);
        expect(tester.takeException(), isNull);
      } finally {
        FocusManager.instance.primaryFocus?.unfocus();
        await tester.pump();
        tester.testTextInput.register();
        tester.binding.focusedEditable = null;
      }
    },
  );
  testWidgets('new reading leaves correction mode and creates a new root', (
    tester,
  ) async {
    var nowUtc = DateTime.utc(2026, 11, 18, 12, 34, 56, 789);
    final api = FakePulsysApi(clock: () => nowUtc);
    var nextKey = 0;
    final controller = PulseController(
      api: api,
      keyGenerator: () => 'intent-${nextKey++}',
    );
    addTearDown(controller.dispose);
    await controller.loadInitial();
    await tester.pumpWidget(_screen(controller));
    await tester.pumpAndSettle();
    final save = find.byKey(const ValueKey<String>('pulse-save-button'));
    final newReading = find.byKey(
      const ValueKey<String>('pulse-new-reading-button'),
    );
    expect(newReading, findsNothing);
    await enterPulseBpm(tester, bpm: '72');
    await _tap(tester, save);
    final original = controller.history.single;
    expect(tester.widget<FilledButton>(save).onPressed, isNull);
    await _tap(
      tester,
      find.byKey(ValueKey<String>('pulse-correct-${original.recordId}')),
    );
    await enterPulseBpm(tester, bpm: '74');
    await _tap(tester, save);
    final corrected = controller.history.single;
    final correctionKey = controller.currentIntentKey;
    expect(corrected.replaces, original.recordId);
    expect(corrected.rootId, original.rootId);
    expect(corrected.measuredAtUtc, original.measuredAtUtc);
    expect(tester.widget<FilledButton>(save).onPressed, isNull);
    final l10n = AppLocalizations.of(tester.element(newReading));
    expect(find.text(l10n.pulseNewReadingAction), findsOneWidget);
    await _tap(tester, newReading);
    expect(controller.phase, PulseFormPhase.editing);
    expect(controller.replaces, isNull);
    expect(controller.currentIntentKey, isNot(correctionKey));
    expect(controller.bpmText, isEmpty);
    expect(controller.measuredAtText, isEmpty);
    expect(controller.noteText, isEmpty);
    expect(controller.lastReceipt, isNull);
    expect(find.text(l10n.pulseCorrectionHeading), findsNothing);
    nowUtc = nowUtc.add(const Duration(minutes: 1));
    await enterPulseBpm(tester, bpm: '76');
    await _tap(tester, save);
    final receipt = controller.lastReceipt!;
    expect(api.preparedInputs.last.replaces, isNull);
    expect(api.preparedInputs.last.measuredAtUtc, isNull);
    expect(receipt.replaces, isNull);
    expect(receipt.rootId, receipt.recordId);
    expect(receipt.rootId, isNot(original.rootId));
    expect(receipt.measuredAtUtc, nowUtc);
    expect(controller.history, hasLength(2));
    expect(controller.history.map((point) => point.rootId).toSet(), <String>{
      corrected.rootId,
      receipt.rootId,
    });
    expect(
      controller.history.map((point) => point.recordId),
      contains(corrected.recordId),
    );
    expect(api.preparedInputs, hasLength(3));
    expect(api.historySnapshot, hasLength(3));
    expect(tester.widget<FilledButton>(save).onPressed, isNull);
    expect(tester.takeException(), isNull);
  });
}
