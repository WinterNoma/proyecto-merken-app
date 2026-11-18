import 'package:material_ui/material_ui.dart';

import 'design/theme.dart';
import 'l10n/generated/app_localizations.dart';
import 'platform/pulsys_api.dart';
import 'routing/app_router.dart';

class PulsysApp extends StatelessWidget {
  const PulsysApp({required this.api, this.clock, super.key});

  final PulsysApi api;
  final DateTime Function()? clock;

  @override
  Widget build(BuildContext context) {
    final reducedMotion = MediaQuery.maybeDisableAnimationsOf(context) ?? false;
    return MaterialApp(
      key: ObjectKey(api),
      debugShowCheckedModeBanner: false,
      onGenerateTitle: (BuildContext context) =>
          AppLocalizations.of(context).appTitle,
      theme: PulsysTheme.dark(reducedMotion: reducedMotion),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: AppRouter.home(api: api, clock: clock),
    );
  }
}
