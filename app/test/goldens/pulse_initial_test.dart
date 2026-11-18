// Baselines are rendered on Windows; Linux CI rasterizes text differently.
@Tags(<String>['golden'])
library;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/features/pulse/pulse_screen.dart';

import '../fakes/fake_pulsys_api.dart';
import '../support/pulse_app.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final fonts = <String, String>{
      'Barlow Semi Condensed':
          'lib/design/fonts/BarlowSemiCondensed-SemiBold.ttf',
      'Public Sans': 'lib/design/fonts/PublicSans-Variable.ttf',
      'Roboto Mono': 'lib/design/fonts/RobotoMono-Variable.ttf',
      'MaterialIcons': 'fonts/MaterialIcons-Regular.otf',
    };
    for (final entry in fonts.entries) {
      final loader = FontLoader(entry.key)
        ..addFont(rootBundle.load(entry.value));
      await loader.load();
    }
  });

  testWidgets('initial pulse screen with bundled fonts', (tester) async {
    tester.view.physicalSize = const Size(390, 1100);
    tester.view.devicePixelRatio = 1;
    tester.platformDispatcher.textScaleFactorTestValue = 1;
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    addTearDown(() => debugDefaultTargetPlatformOverride = null);

    const boundary = ValueKey<String>('pulse-golden');
    final api = FakePulsysApi(clock: () => DateTime.utc(2026, 11, 18, 12));
    await tester.pumpWidget(
      MediaQuery(
        data: const MediaQueryData(disableAnimations: true),
        child: RepaintBoundary(key: boundary, child: pulseTestApp(api)),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('No pulse readings recorded yet.'), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('pulse-new-reading-button')),
      findsNothing,
    );
    expect(
      tester
          .widget<MaterialApp>(find.byType(MaterialApp))
          .debugShowCheckedModeBanner,
      isFalse,
    );
    expect(find.byType(CheckedModeBanner), findsNothing);
    expect(tester.takeException(), isNull);
    await expectLater(
      find.byKey(boundary),
      matchesGoldenFile('baselines/pulse_initial.png'),
    );
  });
  testWidgets('saved correction offers a new reading with bundled fonts', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(390, 1400);
    tester.view.devicePixelRatio = 1;
    tester.platformDispatcher.textScaleFactorTestValue = 1;
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    addTearDown(() => debugDefaultTargetPlatformOverride = null);

    const boundary = ValueKey<String>('pulse-corrected-golden');
    final api = FakePulsysApi(
      clock: () => DateTime.utc(2026, 11, 18, 12, 34, 56, 789),
    );
    await tester.pumpWidget(
      MediaQuery(
        data: const MediaQueryData(disableAnimations: true),
        child: RepaintBoundary(key: boundary, child: pulseTestApp(api)),
      ),
    );
    await tester.pumpAndSettle();
    final controller = tester
        .widget<PulseScreen>(find.byType(PulseScreen))
        .controller;
    controller.updateBpm('72');
    await controller.submit();
    controller.startCorrection(controller.history.single);
    controller.updateBpm('74');
    await controller.submit();
    await tester.pumpAndSettle();
    expect(find.text('Correcting a previous reading'), findsOneWidget);
    expect(find.text('New reading'), findsOneWidget);
    expect(find.text('74 bpm'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await expectLater(
      find.byKey(boundary),
      matchesGoldenFile('baselines/pulse_corrected.png'),
    );
  });
}
