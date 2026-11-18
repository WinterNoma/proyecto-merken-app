import 'package:material_ui/material_ui.dart';
import 'package:pulsys/design/theme.dart';
import 'package:pulsys/features/pulse/pulse_screen.dart';
import 'package:pulsys/l10n/generated/app_localizations.dart';
import 'package:pulsys/platform/pulsys_api.dart';

Widget pulseTestApp(PulsysApi api) => MaterialApp(
  debugShowCheckedModeBanner: false,
  theme: PulsysTheme.dark(reducedMotion: true),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  home: PulsePage(api: api),
);
