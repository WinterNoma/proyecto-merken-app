import 'package:material_ui/material_ui.dart';

import '../features/pulse/pulse_screen.dart';
import '../platform/pulsys_api.dart';

abstract final class AppRouter {
  static const String pulse = '/';

  static Route<dynamic> onGenerateRoute(
    RouteSettings settings, {
    required PulsysApi api,
  }) {
    return switch (settings.name) {
      pulse || null => MaterialPageRoute<void>(
        settings: settings,
        builder: (BuildContext context) => PulsePage(api: api),
      ),
      _ => MaterialPageRoute<void>(
        settings: settings,
        builder: (BuildContext context) => PulsePage(api: api),
      ),
    };
  }
}
