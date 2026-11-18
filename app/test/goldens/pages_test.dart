@Tags(<String>['golden'])
library;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/app.dart';

import '../support/overview_fixture.dart';

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

  for (final size in <Size>[const Size(560, 1450), const Size(411, 891)]) {
    for (final page in <String>['history', 'today', 'recommendations']) {
      testWidgets('$page page at ${size.width} with bundled fonts', (
        tester,
      ) async {
        tester.view.physicalSize = size;
        tester.view.devicePixelRatio = 1;
        tester.platformDispatcher.textScaleFactorTestValue = 1;
        debugDefaultTargetPlatformOverride = TargetPlatform.android;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        addTearDown(() => debugDefaultTargetPlatformOverride = null);

        const boundary = ValueKey<String>('page-golden');
        await tester.pumpWidget(
          MediaQuery(
            data: MediaQueryData.fromView(tester.view)
                .copyWith(disableAnimations: true),
            child: RepaintBoundary(
              key: boundary,
              child: PulsysApp(api: overviewApi(), clock: overviewClock),
            ),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(ValueKey<String>('nav-$page')));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        await expectLater(
          find.byKey(boundary),
          matchesGoldenFile(
            'baselines/page_$page${size.width == 411 ? '_411' : ''}.png',
          ),
        );
        if (page == 'today') {
          final legend = find.byKey(const ValueKey('body-status-legend'));
          await tester.ensureVisible(legend);
          await tester.pumpAndSettle();
          final viewport = tester.getRect(
            find.byKey(const ValueKey('app-pages')),
          );
          final legendBounds = tester.getRect(legend);
          expect(legendBounds.height, greaterThan(0));
          for (final label in <String>['Zone status', 'Not evaluated']) {
            final text = find.descendant(
              of: legend,
              matching: find.text(label),
            );
            final bounds = tester.getRect(text);
            expect(bounds.height, greaterThan(0));
            expect(bounds.top, greaterThanOrEqualTo(viewport.top));
            expect(bounds.bottom, lessThanOrEqualTo(viewport.bottom));
            expect(bounds.top, greaterThanOrEqualTo(legendBounds.top));
            expect(bounds.bottom, lessThanOrEqualTo(legendBounds.bottom));
          }
          await expectLater(
            find.byKey(boundary),
            matchesGoldenFile(
              'baselines/page_today_bottom${size.width == 411 ? '_411' : ''}.png',
            ),
          );
        }
      });
    }
  }
}
