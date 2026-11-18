import 'dart:async';

import 'package:material_ui/material_ui.dart';

import 'app.dart';
import 'l10n/generated/app_localizations.dart';
import 'platform/device/app_environment.dart';
import 'platform/rust_adapter.dart';

Future<RustAdapter> createPulsysApi() async {
  final environment = await AppEnvironment.load();
  return RustAdapter.open(
    privateDirectory: environment.privateDirectory,
    appVersion: environment.appVersion,
  );
}

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  try {
    final api = await createPulsysApi();
    runApp(_PulsysRoot(api: api));
  } catch (error, stackTrace) {
    FlutterError.reportError(
      FlutterErrorDetails(exception: error, stack: stackTrace),
    );
    runApp(const PulsysStartupFailure());
  }
}

class _PulsysRoot extends StatefulWidget {
  const _PulsysRoot({required this.api});

  final RustAdapter api;

  @override
  State<_PulsysRoot> createState() => _PulsysRootState();
}

class _PulsysRootState extends State<_PulsysRoot> {
  @override
  Widget build(BuildContext context) => PulsysApp(api: widget.api);

  @override
  void dispose() {
    unawaited(_close(widget.api));
    super.dispose();
  }

  Future<void> _close(RustAdapter api) async {
    try {
      await api.close();
    } catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(exception: error, stack: stackTrace),
      );
    }
  }
}

class PulsysStartupFailure extends StatelessWidget {
  const PulsysStartupFailure({super.key});

  @override
  Widget build(BuildContext context) => MaterialApp(
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: Builder(
      builder: (context) => Scaffold(
        body: Center(
          child: Text(AppLocalizations.of(context).startupStorageError),
        ),
      ),
    ),
  );
}
