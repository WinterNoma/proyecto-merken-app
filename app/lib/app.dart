import 'package:material_ui/material_ui.dart';

import 'design/theme.dart';
import 'l10n/generated/app_localizations.dart';
import 'platform/pulsys_api.dart';
import 'routing/app_router.dart';

class PulsysApp extends StatelessWidget {
  const PulsysApp({required this.api, super.key});

  final PulsysApi api;

  @override
  Widget build(BuildContext context) {
    final reducedMotion = MediaQuery.maybeDisableAnimationsOf(context) ?? false;
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      onGenerateTitle: (BuildContext context) =>
          AppLocalizations.of(context).appTitle,
      theme: PulsysTheme.dark(reducedMotion: reducedMotion),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      initialRoute: AppRouter.pulse,
      onGenerateRoute: (RouteSettings settings) =>
          AppRouter.onGenerateRoute(settings, api: api),
    );
  }
}
