import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

Future<void> enterPulseBpm(WidgetTester tester, {required String bpm}) async {
  final field = find.byKey(const ValueKey<String>('pulse-bpm-field'));
  await tester.ensureVisible(field);
  await tester.pumpAndSettle();
  expect(tester.widget<TextField>(field).enabled, isTrue);
  final editable = tester.state<EditableTextState>(
    find.descendant(of: field, matching: find.byType(EditableText)),
  );
  expect(editable.widget.readOnly, isFalse);
  final target = field.hitTestable();
  expect(target, findsOneWidget);
  await tester.tap(target);
  await tester.pump();
  expect(editable.widget.focusNode.hasFocus, isTrue);
  await tester.enterText(field, bpm);
  expect(editable.widget.controller.text, bpm);
}
