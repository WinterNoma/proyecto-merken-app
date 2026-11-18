import 'package:material_ui/material_ui.dart';

import 'tokens/colors.dart';
import 'tokens/spacing.dart';
import 'tokens/typography.dart';

enum PulsysTone { ok, warn, alarm, neutral }

@immutable
final class PulsysToneColors {
  const PulsysToneColors({required this.foreground, required this.background});

  final Color foreground;
  final Color background;

  static PulsysToneColors forTone(PulsysTone tone) {
    return switch (tone) {
      PulsysTone.ok => const PulsysToneColors(
        foreground: PulsysColors.ok,
        background: PulsysColors.okBg,
      ),
      PulsysTone.warn => const PulsysToneColors(
        foreground: PulsysColors.warn,
        background: PulsysColors.warnBg,
      ),
      PulsysTone.alarm => const PulsysToneColors(
        foreground: PulsysColors.alarm,
        background: PulsysColors.alarmBg,
      ),
      PulsysTone.neutral => const PulsysToneColors(
        foreground: PulsysColors.neutral,
        background: PulsysColors.neutralBg,
      ),
    };
  }
}

abstract final class PulsysTheme {
  static ThemeData dark({bool reducedMotion = false}) {
    const colorScheme = ColorScheme(
      brightness: Brightness.dark,
      primary: PulsysColors.accent,
      onPrimary: PulsysColors.onAccent,
      secondary: PulsysColors.surfaceRaised,
      onSecondary: PulsysColors.text,
      error: PulsysColors.alarm,
      onError: PulsysColors.onEmergency,
      surface: PulsysColors.surface,
      onSurface: PulsysColors.text,
      surfaceContainerHighest: PulsysColors.surfaceRaised,
      onSurfaceVariant: PulsysColors.textMuted,
      outline: PulsysColors.border,
      outlineVariant: PulsysColors.divider,
      scrim: PulsysColors.scrim,
    );

    final controlShape = RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(PulsysRadii.md),
    );

    final inputBorder = OutlineInputBorder(
      borderRadius: BorderRadius.circular(PulsysRadii.md),
      borderSide: const BorderSide(
        color: PulsysColors.border,
        width: PulsysBorders.control,
      ),
    );

    final focusedInputBorder = OutlineInputBorder(
      borderRadius: BorderRadius.circular(PulsysRadii.md),
      borderSide: const BorderSide(
        color: PulsysColors.focus,
        width: PulsysBorders.focus,
      ),
    );

    final errorInputBorder = OutlineInputBorder(
      borderRadius: BorderRadius.circular(PulsysRadii.md),
      borderSide: const BorderSide(
        color: PulsysColors.alarm,
        width: PulsysBorders.focus,
      ),
    );

    return ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      colorScheme: colorScheme,
      scaffoldBackgroundColor: PulsysColors.bg,
      canvasColor: PulsysColors.surface,
      splashFactory: NoSplash.splashFactory,
      pageTransitionsTheme: reducedMotion
          ? const PageTransitionsTheme(
              builders: <TargetPlatform, PageTransitionsBuilder>{
                TargetPlatform.android: _NoAnimationTransitionsBuilder(),
                TargetPlatform.iOS: _NoAnimationTransitionsBuilder(),
                TargetPlatform.linux: _NoAnimationTransitionsBuilder(),
                TargetPlatform.macOS: _NoAnimationTransitionsBuilder(),
                TargetPlatform.windows: _NoAnimationTransitionsBuilder(),
              },
            )
          : const PageTransitionsTheme(),
      appBarTheme: const AppBarTheme(
        backgroundColor: PulsysColors.bg,
        foregroundColor: PulsysColors.text,
        elevation: 0,
        scrolledUnderElevation: 0,
        titleTextStyle: PulsysTypography.wordmark,
        shape: Border(
          bottom: BorderSide(
            color: PulsysColors.divider,
            width: PulsysBorders.control,
          ),
        ),
      ),
      cardTheme: CardThemeData(
        color: PulsysColors.surface,
        elevation: 0,
        margin: EdgeInsets.zero,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(PulsysRadii.md),
          side: const BorderSide(
            color: PulsysColors.divider,
            width: PulsysBorders.control,
          ),
        ),
      ),
      dividerTheme: const DividerThemeData(
        color: PulsysColors.divider,
        thickness: PulsysBorders.control,
        space: PulsysSpacing.s4,
      ),
      dialogTheme: DialogThemeData(
        backgroundColor: PulsysColors.surface,
        elevation: 0,
        shadowColor: PulsysColors.dialogShadow,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(PulsysRadii.lg),
          side: const BorderSide(
            color: PulsysColors.border,
            width: PulsysBorders.control,
          ),
        ),
        titleTextStyle: PulsysTypography.componentHeading,
        contentTextStyle: PulsysTypography.body,
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: PulsysColors.surfaceRaised,
        labelStyle: PulsysTypography.label,
        floatingLabelStyle: PulsysTypography.label,
        hintStyle: PulsysTypography.caption,
        helperStyle: PulsysTypography.caption,
        errorStyle: PulsysTypography.caption.copyWith(
          color: PulsysColors.alarm,
        ),
        suffixStyle: PulsysTypography.dataCaption,
        border: inputBorder,
        enabledBorder: inputBorder,
        focusedBorder: focusedInputBorder,
        errorBorder: errorInputBorder,
        focusedErrorBorder: errorInputBorder,
        contentPadding: const EdgeInsets.symmetric(
          horizontal: PulsysSpacing.s3,
          vertical: PulsysSpacing.s3,
        ),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: ButtonStyle(
          minimumSize: const WidgetStatePropertyAll<Size>(
            Size(PulsysSpacing.minTarget, PulsysSpacing.minTarget),
          ),
          padding: const WidgetStatePropertyAll<EdgeInsetsGeometry>(
            EdgeInsets.symmetric(
              horizontal: PulsysSpacing.s4,
              vertical: PulsysSpacing.s3,
            ),
          ),
          shape: WidgetStatePropertyAll<OutlinedBorder>(controlShape),
          textStyle: const WidgetStatePropertyAll<TextStyle>(
            PulsysTypography.bodyStrong,
          ),
          backgroundColor: WidgetStateProperty.resolveWith<Color>((
            Set<WidgetState> states,
          ) {
            if (states.contains(WidgetState.disabled)) {
              return PulsysColors.surfaceRaised;
            }
            if (states.contains(WidgetState.pressed)) {
              return PulsysColors.accentPressed;
            }
            if (states.contains(WidgetState.hovered)) {
              return PulsysColors.accentHover;
            }
            return PulsysColors.accent;
          }),
          foregroundColor: WidgetStateProperty.resolveWith<Color>((
            Set<WidgetState> states,
          ) {
            if (states.contains(WidgetState.disabled)) {
              return PulsysColors.textMuted;
            }
            return PulsysColors.onAccent;
          }),
          side: WidgetStateProperty.resolveWith<BorderSide>((
            Set<WidgetState> states,
          ) {
            if (states.contains(WidgetState.focused)) {
              return const BorderSide(
                color: PulsysColors.focus,
                width: PulsysBorders.focus,
              );
            }
            if (states.contains(WidgetState.disabled)) {
              return const BorderSide(
                color: PulsysColors.border,
                width: PulsysBorders.control,
              );
            }
            return BorderSide.none;
          }),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: ButtonStyle(
          minimumSize: const WidgetStatePropertyAll<Size>(
            Size(PulsysSpacing.minTarget, PulsysSpacing.minTarget),
          ),
          padding: const WidgetStatePropertyAll<EdgeInsetsGeometry>(
            EdgeInsets.symmetric(
              horizontal: PulsysSpacing.s4,
              vertical: PulsysSpacing.s3,
            ),
          ),
          shape: WidgetStatePropertyAll<OutlinedBorder>(controlShape),
          textStyle: const WidgetStatePropertyAll<TextStyle>(
            PulsysTypography.bodyStrong,
          ),
          backgroundColor: const WidgetStatePropertyAll<Color>(
            PulsysColors.surface,
          ),
          foregroundColor: const WidgetStatePropertyAll<Color>(
            PulsysColors.text,
          ),
          side: WidgetStateProperty.resolveWith<BorderSide>((
            Set<WidgetState> states,
          ) {
            if (states.contains(WidgetState.focused)) {
              return const BorderSide(
                color: PulsysColors.focus,
                width: PulsysBorders.focus,
              );
            }
            return const BorderSide(
              color: PulsysColors.border,
              width: PulsysBorders.control,
            );
          }),
        ),
      ),
      textTheme: const TextTheme(
        headlineMedium: PulsysTypography.sectionTitle,
        titleLarge: PulsysTypography.componentHeading,
        titleMedium: PulsysTypography.bodyStrong,
        bodyLarge: PulsysTypography.body,
        bodyMedium: PulsysTypography.body,
        bodySmall: PulsysTypography.caption,
        labelLarge: PulsysTypography.label,
        labelSmall: PulsysTypography.identifier,
      ),
    );
  }
}

final class _NoAnimationTransitionsBuilder extends PageTransitionsBuilder {
  const _NoAnimationTransitionsBuilder();

  @override
  Widget buildTransitions<T>(
    PageRoute<T> route,
    BuildContext context,
    Animation<double> animation,
    Animation<double> secondaryAnimation,
    Widget child,
  ) => child;
}
