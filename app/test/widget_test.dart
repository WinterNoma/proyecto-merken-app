import 'package:flutter_test/flutter_test.dart';

import 'package:pulsys/app.dart';

import 'fakes/fake_pulsys_api.dart';

void main() {
  testWidgets('PulsysApp renders pulse screen smoke test', (
    WidgetTester tester,
  ) async {
    final api = FakePulsysApi();
    await tester.pumpWidget(PulsysApp(api: api));
    await tester.pumpAndSettle();

    expect(find.text('PULSYS'), findsOneWidget);
    expect(find.text('Pulse'), findsOneWidget);
    expect(find.text('Save reading'), findsOneWidget);
  });
}
