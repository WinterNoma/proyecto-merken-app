@Tags(<String>['golden'])
library;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/design/theme.dart';
import 'package:pulsys/features/body/body_screen.dart';
import 'package:pulsys/l10n/generated/app_localizations.dart';
import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';

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

  testWidgets('front view, legend and extra zones with bundled fonts', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(390, 1100);
    tester.view.devicePixelRatio = 1;
    tester.platformDispatcher.textScaleFactorTestValue = 1;
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    addTearDown(() => debugDefaultTargetPlatformOverride = null);

    const boundary = ValueKey('body-golden');
    final api = FakePulsysApi(
      demoTarget: const DemoTarget(generation: 'golden'),
      initialZones: [
        for (final zone in BodyZone.values)
          ZoneState(
            zoneId: zone,
            status: switch (zone) {
              BodyZone.neck => EvaluationStatus.change,
              BodyZone.leftArm => EvaluationStatus.alarm,
              BodyZone.rightLeg => EvaluationStatus.stable,
              _ => EvaluationStatus.notEvaluated,
            },
            symptoms: const [],
          ),
      ],
    );
    await tester.pumpWidget(
      RepaintBoundary(
        key: boundary,
        child: MaterialApp(
          debugShowCheckedModeBanner: false,
          theme: PulsysTheme.dark(reducedMotion: true),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: BodyScreen(api: api),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('DEMO'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('figure-other')).hitTestable(),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
    await expectLater(
      find.byKey(boundary),
      matchesGoldenFile('baselines/body_figure.png'),
    );
  });
}
