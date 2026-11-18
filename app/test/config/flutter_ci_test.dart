import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

void main() {
  test('CI checks adoption crates and regenerated localization files', () {
    final workflow = File('../.github/workflows/flutter-ci.yml')
        .readAsStringSync();
    expect(
      workflow,
      contains('cargo fmt -p pulsys-api -p pulsys-bridge -- --check'),
    );
    expect(workflow, isNot(contains('cargo fmt --all')));
    expect(
      workflow.indexOf('flutter gen-l10n'),
      lessThan(workflow.indexOf('git diff --exit-code')),
    );
    expect(workflow, contains('            app/lib/l10n/generated'));
    expect(workflow, contains('git ls-files --others --exclude-standard'));
  });

  test('manual Flutter and Android files have no template instructions', () {
    for (final file in <String>[
      'pubspec.yaml',
      'android/app/build.gradle.kts',
      'android/app/src/main/AndroidManifest.xml',
    ]) {
      final content = File(file).readAsStringSync();
      expect(content, isNot(contains('TODO:')), reason: file);
      expect(content, isNot(contains('The following')), reason: file);
      expect(content, isNot(contains('To learn more')), reason: file);
    }
  });
}
