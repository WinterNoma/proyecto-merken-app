import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/app.dart';

import 'fakes/fake_pulsys_api.dart';

void main() {
  testWidgets('PulsysApp opens Today with a pulse card', (tester) async {
    await tester.pumpWidget(PulsysApp(api: FakePulsysApi()));
    await tester.pumpAndSettle();
    expect(find.text('PULSYS'), findsOneWidget);
    expect(find.byKey(const ValueKey('overview-pulse-card')), findsOneWidget);
    expect(find.text('Save reading'), findsNothing);
  });
}
