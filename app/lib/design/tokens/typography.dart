import 'package:flutter/painting.dart';

import 'colors.dart';

abstract final class PulsysTypography {
  static const String headingFamily = 'Barlow Semi Condensed';
  static const String bodyFamily = 'Public Sans';
  static const String dataFamily = 'Roboto Mono';

  static const List<String> headingFallback = <String>['Arial', 'sans-serif'];
  static const List<String> bodyFallback = <String>['Arial', 'sans-serif'];
  static const List<String> dataFallback = <String>['Consolas', 'monospace'];

  static const List<FontFeature> tabularLining = <FontFeature>[
    FontFeature.tabularFigures(),
    FontFeature.liningFigures(),
  ];

  static const TextStyle wordmark = TextStyle(
    fontFamily: headingFamily,
    fontFamilyFallback: headingFallback,
    fontSize: 20,
    fontWeight: FontWeight.w600,
    letterSpacing: 20 * 0.06,
    height: 1.2,
    color: PulsysColors.text,
  );

  static const TextStyle sectionTitle = TextStyle(
    fontFamily: headingFamily,
    fontFamilyFallback: headingFallback,
    fontSize: 24,
    fontWeight: FontWeight.w600,
    height: 1.2,
    color: PulsysColors.text,
  );

  static const TextStyle componentHeading = TextStyle(
    fontFamily: headingFamily,
    fontFamilyFallback: headingFallback,
    fontSize: 20,
    fontWeight: FontWeight.w600,
    height: 1.2,
    color: PulsysColors.text,
  );

  static const TextStyle body = TextStyle(
    fontFamily: bodyFamily,
    fontFamilyFallback: bodyFallback,
    fontSize: 16,
    fontWeight: FontWeight.w400,
    height: 1.6,
    color: PulsysColors.text,
  );

  static const TextStyle bodyStrong = TextStyle(
    fontFamily: bodyFamily,
    fontFamilyFallback: bodyFallback,
    fontSize: 16,
    fontWeight: FontWeight.w600,
    height: 1.6,
    color: PulsysColors.text,
  );

  static const TextStyle caption = TextStyle(
    fontFamily: bodyFamily,
    fontFamilyFallback: bodyFallback,
    fontSize: 14,
    fontWeight: FontWeight.w400,
    height: 1.5,
    color: PulsysColors.textMuted,
  );

  static const TextStyle label = TextStyle(
    fontFamily: bodyFamily,
    fontFamilyFallback: bodyFallback,
    fontSize: 14,
    fontWeight: FontWeight.w600,
    height: 1.5,
    color: PulsysColors.text,
  );

  static const TextStyle identifier = TextStyle(
    fontFamily: bodyFamily,
    fontFamilyFallback: bodyFallback,
    fontSize: 12,
    fontWeight: FontWeight.w600,
    letterSpacing: 12 * 0.08,
    height: 1.5,
    color: PulsysColors.textMuted,
  );

  static const TextStyle dataPrimary = TextStyle(
    fontFamily: dataFamily,
    fontFamilyFallback: dataFallback,
    fontSize: 48,
    fontWeight: FontWeight.w400,
    height: 1.1,
    fontFeatures: tabularLining,
    color: PulsysColors.text,
  );

  static const TextStyle dataBody = TextStyle(
    fontFamily: dataFamily,
    fontFamilyFallback: dataFallback,
    fontSize: 16,
    fontWeight: FontWeight.w400,
    height: 1.5,
    fontFeatures: tabularLining,
    color: PulsysColors.text,
  );

  static const TextStyle dataCaption = TextStyle(
    fontFamily: dataFamily,
    fontFamilyFallback: dataFallback,
    fontSize: 14,
    fontWeight: FontWeight.w400,
    height: 1.5,
    fontFeatures: tabularLining,
    color: PulsysColors.textMuted,
  );
}
