// Baselines are rendered on Windows; Linux CI rasterizes text differently.
@Tags(<String>['golden'])
library;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/design/theme.dart';
import 'package:pulsys/features/history/history_screen.dart';
import 'package:pulsys/l10n/generated/app_localizations.dart';
import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    const fonts = <String, String>{
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

  testWidgets('pulse trend and readings with bundled fonts', (tester) async {
    tester.view.physicalSize = const Size(390, 900);
    tester.view.devicePixelRatio = 1;
    tester.platformDispatcher.textScaleFactorTestValue = 1;
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    addTearDown(() => debugDefaultTargetPlatformOverride = null);

    const boundary = ValueKey<String>('history-golden');
    final api = FakePulsysApi(
      initialHistory: <PulsePoint>[
        _point(1, 69, 2),
        _point(2, 78.5, 15),
        _point(3, 71.25, 21),
        _point(4, 73, 45),
      ],
    );
    await tester.pumpWidget(
      MediaQuery(
        data: const MediaQueryData(disableAnimations: true),
        child: RepaintBoundary(
          key: boundary,
          child: MaterialApp(
            debugShowCheckedModeBanner: false,
            theme: PulsysTheme.dark(reducedMotion: true),
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: Scaffold(
              body: HistoryScreen(
                api: api,
                clock: () => DateTime.utc(2026, 11, 18, 12),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Pulse trend'), findsOneWidget);
    expect(find.text('73 bpm'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await expectLater(
      find.byKey(boundary),
      matchesGoldenFile('baselines/history_trend.png'),
    );
  });
}

PulsePoint _point(int index, double bpm, int minute) {
  final id = 'trend-$index';
  final measuredAt = DateTime.utc(2026, 11, 18, 11, minute);
  return PulsePoint(
    recordId: id,
    rootId: id,
    bpm: Bpm(bpm),
    source: RecordSource.form,
    measuredAtUtc: measuredAt,
    loggedAtUtc: measuredAt.add(const Duration(seconds: 1)),
    isDemo: false,
  );
}
