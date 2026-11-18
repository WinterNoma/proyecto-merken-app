import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/app.dart';
import 'package:pulsys/features/pulse/pulse_controller.dart';
import 'package:pulsys/features/pulse/pulse_screen.dart';
import 'package:pulsys/l10n/generated/app_localizations.dart';
import 'package:pulsys/platform/device/app_environment.dart';
import 'package:pulsys/platform/pulsys_api.dart';
import 'package:pulsys/platform/rust_adapter.dart';

import '../test/support/pulse_input.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('Saves offline and reopens the pulse without duplicating it', (
    WidgetTester tester,
  ) async {
    await HttpOverrides.runZoned(
      () => _pulseFlow(tester),
      createHttpClient: (_) => throw StateError('Pulse must work offline.'),
    );
  });

  testWidgets(
    'Form confirmation, correction and adapter reopening stay offline',
    (WidgetTester tester) async {
      await HttpOverrides.runZoned(
        () => _uiPulseFlow(tester),
        createHttpClient: (_) => throw StateError('Pulse must work offline.'),
      );
    },
  );
}

Future<void> _pulseFlow(WidgetTester tester) async {
  final environment = await AppEnvironment.load();
  final privateDirectory = await environment.privateDirectory.createTemp(
    'pulse-flow-',
  );
  addTearDown(() => privateDirectory.delete(recursive: true));

  Future<RustAdapter> open() async {
    final adapter = await RustAdapter.open(
      privateDirectory: privateDirectory,
      appVersion: environment.appVersion,
    );
    addTearDown(adapter.close);
    return adapter;
  }

  final measuredAtUtc = DateTime.fromMillisecondsSinceEpoch(
    DateTime.now().millisecondsSinceEpoch,
    isUtc: true,
  );
  final input = PulseInput(
    key: '5a87e03c-1a14-4a78-b52f-56190c6223fb',
    bpm: Bpm(201),
    measuredAtUtc: measuredAtUtc,
    note: 'Synthetic pulse',
  );

  final first = await open();
  await expectLater(
    first.preparePulse(PulseInput(key: 'invalid', bpm: input.bpm)),
    throwsA(
      isA<ApiError>()
          .having((error) => error.code, 'code', ApiErrorCodes.invalidValue)
          .having((error) => error.field, 'field', ApiFields.key),
    ),
  );
  final prepared = await first.preparePulse(input);
  final pending = await first.confirmPulse(prepared);
  expect(pending, isA<NeedsConfirmation>());
  expect(await first.effectivePulseHistory(), isEmpty);
  final approved = (pending as NeedsConfirmation).prepared.approve();
  final result = await first.confirmPulse(approved);
  expect(result, isA<Saved>());
  final receipt = (result as Saved).receipt;
  expect(receipt.key, input.key);
  expect(receipt.measuredAtUtc, measuredAtUtc);
  expect(receipt.loggedAtUtc.isUtc, isTrue);
  expect(receipt.outboxState, PulseOutboxState.pending);
  await first.close();

  final reopened = await open();
  final beforeReplay = await reopened.effectivePulseHistory();
  expect(beforeReplay, hasLength(1));
  expect(beforeReplay.single.recordId, receipt.recordId);
  expect(beforeReplay.single.measuredAtUtc, measuredAtUtc);
  expect(beforeReplay.single.bpm, input.bpm);

  final replay = await reopened.confirmPulse(
    await reopened.preparePulse(input),
  );
  expect(replay, isA<Saved>());
  final repeatedReceipt = (replay as Saved).receipt;
  expect(repeatedReceipt.recordId, receipt.recordId);
  expect(repeatedReceipt.rootId, receipt.rootId);
  expect(repeatedReceipt.loggedAtUtc, receipt.loggedAtUtc);
  final afterReplay = await reopened.effectivePulseHistory();
  expect(afterReplay, hasLength(1));
  expect(afterReplay.single.recordId, receipt.recordId);

  final zones = await reopened.zoneStatus();
  expect(zones.zones.map((ZoneState zone) => zone.zoneId), hasLength(14));
  expect(zones.evaluatedAtUtc.isUtc, isTrue);

  await tester.pumpWidget(PulsysApp(api: reopened));
  await _waitUntil(
    tester,
    () => find.text('201 bpm').evaluate().isNotEmpty,
    expected: 'the reopened 201 bpm reading in history',
    api: reopened,
  );
}

Future<void> _uiPulseFlow(WidgetTester tester) async {
  final environment = await AppEnvironment.load();
  final directory = await environment.privateDirectory.createTemp('pulse-ui-');
  addTearDown(() => directory.delete(recursive: true));

  Future<RustAdapter> open() async {
    final adapter = await RustAdapter.open(
      privateDirectory: directory,
      appVersion: environment.appVersion,
    );
    addTearDown(adapter.close);
    return adapter;
  }

  final first = await open();
  final probe = await first.preparePulse(
    PulseInput(key: '720cb72b-5624-454d-b57a-5e281ccf41be', bpm: Bpm(201)),
  );
  expect(probe.requiresConfirmation, isTrue);
  expect(
    probe.warnings,
    contains(
      const PulseWarning(
        code: PulseWarning.unusualValue,
        ruleId: 'plaus.heart_rate',
      ),
    ),
  );
  expect(await first.effectivePulseHistory(), isEmpty);
  await tester.pumpWidget(PulsysApp(api: first));
  await _waitUntil(
    tester,
    () {
      final controller = _pulseController(tester);
      return controller != null &&
          controller.canSubmit &&
          !controller.historyLoading &&
          !controller.historyLoadFailed &&
          controller.zoneSnapshot != null &&
          !controller.zonesLoadFailed;
    },
    expected: 'the editable form with initial history and zones loaded',
    api: first,
  );
  final l10n = AppLocalizations.of(tester.element(find.byType(PulseScreen)));
  await _enterBpm(tester, bpm: '201', api: first);
  await _tapVisible(
    tester,
    find.byKey(const ValueKey<String>('pulse-save-button')),
    api: first,
  );
  await _waitUntil(
    tester,
    () => find.text(l10n.confirmDialogTitle).evaluate().isNotEmpty,
    expected: l10n.confirmDialogTitle,
    api: first,
    unexpected: () => _pulseController(tester)?.phase == PulseFormPhase.saved,
  );
  expect(await first.effectivePulseHistory(), isEmpty);
  await _tapVisible(tester, find.text(l10n.confirmDialogConfirm), api: first);
  await _waitUntil(
    tester,
    () => find.text('201 bpm').evaluate().isNotEmpty,
    expected: 'the saved 201 bpm reading in history',
    api: first,
  );
  final original = (await first.effectivePulseHistory()).single;
  expect(
    tester
        .widget<FilledButton>(
          find.byKey(const ValueKey<String>('pulse-save-button')),
        )
        .onPressed,
    isNull,
  );
  expect(await first.effectivePulseHistory(), hasLength(1));

  await _tapVisible(
    tester,
    find.byKey(ValueKey<String>('pulse-correct-${original.recordId}')),
    api: first,
  );
  await _waitUntil(
    tester,
    () {
      final controller = _pulseController(tester);
      return controller?.phase == PulseFormPhase.editing &&
          controller?.replaces == original.recordId &&
          (controller?.canEdit ?? false);
    },
    expected: 'an editable correction of ${original.recordId}',
    api: first,
  );
  await _enterBpm(tester, bpm: '202', api: first);
  await _tapVisible(
    tester,
    find.byKey(const ValueKey<String>('pulse-save-button')),
    api: first,
  );
  await _waitUntil(
    tester,
    () => find.text(l10n.confirmDialogTitle).evaluate().isNotEmpty,
    expected: l10n.confirmDialogTitle,
    api: first,
    unexpected: () => _pulseController(tester)?.phase == PulseFormPhase.saved,
  );
  expect(
    (await first.effectivePulseHistory()).single.recordId,
    original.recordId,
  );
  await _tapVisible(tester, find.text(l10n.confirmDialogConfirm), api: first);
  await _waitUntil(
    tester,
    () => find.text('202 bpm').evaluate().isNotEmpty,
    expected: 'the corrected 202 bpm reading in history',
    api: first,
  );
  final corrected = (await first.effectivePulseHistory()).single;
  expect(corrected.recordId, isNot(original.recordId));
  expect(corrected.rootId, original.rootId);
  expect(corrected.replaces, original.recordId);
  expect(corrected.bpm, Bpm(202));
  expect(corrected.measuredAtUtc, original.measuredAtUtc);
  expect(find.text('Corrected'), findsOneWidget);

  await _tapVisible(
    tester,
    find.byKey(const ValueKey<String>('pulse-new-reading-button')),
    api: first,
  );
  expect(_pulseController(tester)?.replaces, isNull);
  expect(_pulseController(tester)?.bpmText, isEmpty);
  expect(_pulseController(tester)?.measuredAtText, isEmpty);
  await _enterBpm(tester, bpm: '72', api: first);
  await _tapVisible(
    tester,
    find.byKey(const ValueKey<String>('pulse-save-button')),
    api: first,
  );
  await _waitUntil(
    tester,
    () => find.text('72 bpm').evaluate().isNotEmpty,
    expected: 'a new independent 72 bpm reading in history',
    api: first,
  );
  final independent = _pulseController(tester)!.lastReceipt!;
  expect(independent.replaces, isNull);
  expect(independent.rootId, independent.recordId);
  expect(independent.rootId, isNot(original.rootId));
  final effective = await first.effectivePulseHistory();
  expect(effective, hasLength(2));
  expect(effective.map((point) => point.recordId).toSet(), <String>{
    corrected.recordId,
    independent.recordId,
  });

  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pumpAndSettle();
  await first.close();
  final reopened = await open();
  await tester.pumpWidget(PulsysApp(api: reopened));
  await _waitUntil(
    tester,
    () => find.text('202 bpm').evaluate().isNotEmpty,
    expected: 'the reopened 202 bpm reading in history',
    api: reopened,
  );
  final restored = await reopened.effectivePulseHistory();
  expect(restored, hasLength(2));
  final restoredCorrection = restored.singleWhere(
    (point) => point.recordId == corrected.recordId,
  );
  expect(restoredCorrection.rootId, original.rootId);
  expect(restoredCorrection.measuredAtUtc, original.measuredAtUtc);
  final restoredReading = restored.singleWhere(
    (point) => point.recordId == independent.recordId,
  );
  expect(restoredReading.rootId, independent.rootId);
  expect(restoredReading.replaces, isNull);
}

PulseController? _pulseController(WidgetTester tester) => tester
    .widgetList<PulseScreen>(find.byType(PulseScreen))
    .firstOrNull
    ?.controller;

Future<void> _enterBpm(
  WidgetTester tester, {
  required String bpm,
  required PulsysApi api,
}) async {
  try {
    await enterPulseBpm(tester, bpm: bpm);
    expect(_pulseController(tester)?.bpmText, bpm);
  } on Object catch (error) {
    fail('Cannot enter $bpm bpm: $error. ${await _observedState(tester, api)}');
  }
}

Future<void> _tapVisible(
  WidgetTester tester,
  Finder finder, {
  required PulsysApi api,
}) async {
  FocusManager.instance.primaryFocus?.unfocus();
  await tester.pump();
  await _waitUntil(
    tester,
    () => tester.view.viewInsets.bottom == 0,
    expected: 'the native keyboard to close before tapping $finder',
    api: api,
  );
  await tester.ensureVisible(finder);
  await tester.pumpAndSettle();
  final target = finder.hitTestable();
  if (target.evaluate().length != 1) {
    fail('Cannot tap $finder. ${await _observedState(tester, api)}');
  }
  await tester.tap(target);
  await tester.pumpAndSettle();
}

Future<void> _waitUntil(
  WidgetTester tester,
  bool Function() ready, {
  required String expected,
  required PulsysApi api,
  bool Function()? unexpected,
}) async {
  for (var attempt = 0; attempt < 200; attempt++) {
    await tester.pump(const Duration(milliseconds: 100));
    if (ready()) {
      return;
    }
    final controller = _pulseController(tester);
    if ((unexpected?.call() ?? false) ||
        controller?.phase == PulseFormPhase.error ||
        controller?.recoveryError != null) {
      fail(
        'Failed waiting for $expected. ${await _observedState(tester, api)}',
      );
    }
  }
  fail('Timed out waiting for $expected. ${await _observedState(tester, api)}');
}

Future<String> _observedState(WidgetTester tester, PulsysApi api) async {
  final controller = _pulseController(tester);
  final texts = tester
      .widgetList<Text>(find.byType(Text))
      .map((text) => text.data ?? text.textSpan?.toPlainText() ?? '')
      .where((text) => text.isNotEmpty)
      .toList();
  final fields = tester
      .widgetList<TextField>(find.byType(TextField))
      .map(
        (field) => <String, Object?>{
          'key': field.key.toString(),
          'text': field.controller?.text,
          'enabled': field.enabled,
          'error': field.decoration?.errorText,
        },
      )
      .toList();
  final editors = tester
      .widgetList<EditableText>(find.byType(EditableText))
      .map(
        (editor) => <String, Object?>{
          'text': editor.controller.text,
          'focused': editor.focusNode.hasFocus,
          'readOnly': editor.readOnly,
        },
      )
      .toList();
  final buttons = tester
      .widgetList<FilledButton>(find.byType(FilledButton))
      .map(
        (button) => <String, Object?>{
          'key': button.key.toString(),
          'enabled': button.onPressed != null,
        },
      )
      .toList();
  String history;
  try {
    final points = await api.effectivePulseHistory().timeout(
      const Duration(seconds: 3),
    );
    history = points
        .map(
          (point) => <String, Object?>{
            'id': point.recordId,
            'root': point.rootId,
            'replaces': point.replaces,
            'bpm': point.bpm.value,
          },
        )
        .toList()
        .toString();
  } on Object catch (error) {
    history = switch (error) {
      ApiError(:final code, :final field, :final message) =>
        '$code/$field: $message',
      _ => error.toString(),
    };
  }
  final recoveryError = controller?.recoveryError;
  return 'phase=${controller?.phase}; '
      'bpm=${controller?.bpmText}; '
      'canEdit=${controller?.canEdit}; '
      'canSubmit=${controller?.canSubmit}; '
      'replaces=${controller?.replaces}; '
      'pending=${controller?.pendingConfirmation?.key}; '
      'submitError=${controller?.generalError}; '
      'recoveryError=${recoveryError?.code}/${recoveryError?.field}; '
      'historyLoading=${controller?.historyLoading}; '
      'historyLoadFailed=${controller?.historyLoadFailed}; '
      'zonesLoadFailed=${controller?.zonesLoadFailed}; '
      'keyboardInset=${tester.view.viewInsets.bottom}; '
      'fields=$fields; editors=$editors; buttons=$buttons; '
      'routeTexts=$texts; effectiveHistory=$history; '
      'frameworkException=${tester.takeException()}';
}
