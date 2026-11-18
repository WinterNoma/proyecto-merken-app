import 'package:material_ui/material_ui.dart';

import '../features/pulse/pulse_screen.dart';
import '../platform/pulsys_api.dart';
import 'app_shell.dart';

abstract final class AppRouter {
  static Widget home({required PulsysApi api, DateTime Function()? clock}) =>
      AppShell(
        api: api,
        clock: clock,
        openPulse: (context) async {
          await Navigator.of(context).push<void>(pulse(api: api));
        },
      );

  static MaterialPageRoute<void> pulse({required PulsysApi api}) =>
      MaterialPageRoute<void>(
        settings: const RouteSettings(name: '/pulse'),
        builder: (context) => PulsePage(api: api),
      );
}
