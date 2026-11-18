import 'package:flutter_test/flutter_test.dart';
import 'package:pulsys/platform/pulsys_api.dart';

void main() {
  test('Bpm formatting preserves every accepted value', () {
    for (final value in <double>[
      0.04,
      0.00000001,
      73.55555555555556,
      72,
      1e30,
      double.maxFinite,
    ]) {
      expect(double.parse(Bpm(value).format()), value);
    }
    expect(Bpm(0.04).format(), '0.04');
    expect(Bpm(72).format(), '72');
  });

  test('Rust confirmation remains explicit when warnings disappear', () {
    final prepared = PreparedPulse(
      key: 'key',
      bpm: Bpm(72),
      measuredAtUtc: DateTime.utc(2026),
      target: const LocalPulseTarget(storeId: 'store'),
      warnings: const <PulseWarning>[],
      confirmation: PulseConfirmation.requiredReview,
    );
    expect(prepared.requiresConfirmation, isTrue);
    expect(prepared.approve().requiresConfirmation, isFalse);
    expect(prepared.approve().confirmation, PulseConfirmation.approved);
  });
}
