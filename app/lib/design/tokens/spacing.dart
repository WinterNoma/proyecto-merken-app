import 'package:flutter/animation.dart';

abstract final class PulsysSpacing {
  static const double s0 = 0;
  static const double s1 = 4;
  static const double s2 = 8;
  static const double s3 = 12;
  static const double s4 = 16;
  static const double s5 = 24;
  static const double s6 = 32;
  static const double s7 = 48;
  static const double s8 = 64;
  static const double s9 = 96;

  static const double minTarget = 48;
  static const double actionGap = 8;
  static const double iconSize = 20;
}

abstract final class PulsysRadii {
  static const double none = 0;
  static const double sm = 2;
  static const double md = 4;
  static const double lg = 8;
}

abstract final class PulsysBorders {
  static const double control = 1;
  static const double statusRule = 3;
  static const double focus = 2;
  static const double focusOffset = 3;
}

abstract final class PulsysMotion {
  static const Duration instant = Duration.zero;
  static const Duration fast = Duration(milliseconds: 120);
  static const Duration normal = Duration(milliseconds: 180);
  static const Curve ease = Cubic(0.2, 0.0, 0.0, 1.0);
}
