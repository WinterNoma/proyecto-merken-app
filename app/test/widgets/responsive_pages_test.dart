import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/app.dart';

import '../support/overview_fixture.dart';

const _frontZones = <String>[
  'head',
  'neck',
  'chest',
  'abdomen',
  'right_arm',
  'left_arm',
  'right_hand',
  'left_hand',
  'right_leg',
  'left_leg',
  'right_foot',
  'left_foot',
];

void main() {
  for (final width in <double>[320, 411, 600]) {
    testWidgets(
      'three pages fit $width dp without nested horizontal scrolling',
      (tester) async {
        _viewport(tester, width);
        await tester.pumpWidget(
          PulsysApp(api: overviewApi(), clock: overviewClock),
        );
        await tester.pumpAndSettle();
        final canvas = tester.getRect(
          find.byKey(const ValueKey('body-canvas')),
        );
        for (final indicator in <String>[
          'pulse',
          'pressure',
          'oxygen',
          'mood',
          'temperature',
          'glucose',
          'sleep',
          'nutrition',
        ]) {
          final card = tester.getRect(
            find.byKey(ValueKey('overview-$indicator-card')),
          );
          expect(card.left, greaterThanOrEqualTo(0));
          expect(card.right, lessThanOrEqualTo(width));
          if (<String>[
            'pulse',
            'pressure',
            'oxygen',
            'mood',
          ].contains(indicator)) {
            expect(card.right, lessThanOrEqualTo(canvas.left));
          } else {
            expect(card.left, greaterThanOrEqualTo(canvas.right));
          }
        }
        final targets = <Rect>[];
        for (final zone in _frontZones) {
          final target = tester.getRect(find.byKey(ValueKey('figure-$zone')));
          expect(target.width, greaterThanOrEqualTo(48 - .001));
          expect(target.height, greaterThanOrEqualTo(48 - .001));
          expect(target.left, greaterThanOrEqualTo(canvas.left - .001));
          expect(target.right, lessThanOrEqualTo(canvas.right + .001));
          expect(target.top, greaterThanOrEqualTo(canvas.top - .001));
          expect(target.bottom, lessThanOrEqualTo(canvas.bottom + .001));
          for (final previous in targets) {
            final intersection = target.intersect(previous);
            expect(
              intersection.width > .001 && intersection.height > .001,
              isFalse,
              reason: '$zone overlaps another body target at $width dp',
            );
          }
          targets.add(target);
        }
        for (final zone in <String>['back', 'other']) {
          final button = tester.getRect(find.byKey(ValueKey('figure-$zone')));
          expect(button.left, greaterThanOrEqualTo(0));
          expect(button.right, lessThanOrEqualTo(width));
          expect(button.width, greaterThan(canvas.width));
          expect(button.top, greaterThanOrEqualTo(canvas.bottom));
        }
        final navigation = find.byKey(const ValueKey('nav-recommendations'));
        final text = find.descendant(
          of: navigation,
          matching: find.text('Recommendations'),
        );
        final label = tester.widget<Text>(text);
        expect(label.maxLines, 1);
        expect(label.softWrap, isFalse);
        final labelBounds = tester.getRect(text);
        final buttonBounds = tester.getRect(navigation);
        expect(
          labelBounds.left,
          greaterThanOrEqualTo(buttonBounds.left - .001),
        );
        expect(labelBounds.right, lessThanOrEqualTo(buttonBounds.right + .001));
        final paragraph = tester.renderObject<RenderParagraph>(text);
        expect(paragraph.didExceedMaxLines, isFalse);
        final assistant = find.byKey(const ValueKey('overview-assistant'));
        expect(
          find.descendant(of: assistant, matching: find.text('Coming soon')),
          findsOneWidget,
        );
        expect(find.text('Zone status'), findsOneWidget);
        final header = tester.getRect(find.byKey(const ValueKey('nav-home')));
        final legend = find.byKey(const ValueKey('body-status-legend'));
        await tester.ensureVisible(legend);
        await tester.pumpAndSettle();
        final viewport = tester.getRect(
          find.byKey(const ValueKey('app-pages')),
        );
        final legendBounds = tester.getRect(legend);
        expect(legendBounds.height, greaterThan(0));
        for (final label in <String>['Zone status', 'Not evaluated']) {
          final text = find.descendant(of: legend, matching: find.text(label));
          final bounds = tester.getRect(text);
          expect(bounds.height, greaterThan(0));
          expect(bounds.top, greaterThanOrEqualTo(viewport.top));
          expect(bounds.bottom, lessThanOrEqualTo(viewport.bottom));
          expect(bounds.top, greaterThanOrEqualTo(legendBounds.top));
          expect(bounds.bottom, lessThanOrEqualTo(legendBounds.bottom));
        }
        expect(tester.getRect(find.byKey(const ValueKey('nav-home'))), header);

        for (final page in <String>['history', 'today', 'recommendations']) {
          await tester.tap(find.byKey(ValueKey('nav-$page')));
          await tester.pumpAndSettle();
          final horizontal = find.byWidgetPredicate(
            (widget) =>
                widget is Scrollable &&
                axisDirectionToAxis(widget.axisDirection) == Axis.horizontal,
          );
          expect(horizontal, findsOneWidget);
          expect(
            find.ancestor(of: horizontal, matching: find.byType(PageView)),
            findsOneWidget,
          );
          expect(tester.takeException(), isNull);
        }
      },
    );
  }

  testWidgets('swiping across Today figure selects the adjacent pages', (
    tester,
  ) async {
    _viewport(tester, 411);
    await tester.pumpWidget(
      PulsysApp(api: overviewApi(), clock: overviewClock),
    );
    await tester.pumpAndSettle();
    final canvas = find.byKey(const ValueKey('body-canvas'));
    await tester.dragFrom(tester.getCenter(canvas), const Offset(300, 0));
    await tester.pumpAndSettle();
    expect(
      tester.semantics.find(find.byKey(const ValueKey('nav-history'))),
      isSemantics(isSelected: true),
    );
    await tester.tap(find.byKey(const ValueKey('nav-today')));
    await tester.pumpAndSettle();
    await tester.dragFrom(tester.getCenter(canvas), const Offset(-300, 0));
    await tester.pumpAndSettle();
    expect(
      tester.semantics.find(find.byKey(const ValueKey('nav-recommendations'))),
      isSemantics(isSelected: true),
    );
    expect(find.byType(AlertDialog), findsNothing);
  });
}

void _viewport(WidgetTester tester, double width) {
  tester.view.physicalSize = Size(width, 891);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
}
