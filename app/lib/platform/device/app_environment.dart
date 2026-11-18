import 'dart:io';

import 'package:flutter/services.dart';

import '../pulsys_api.dart';

final class AppEnvironment {
  const AppEnvironment({
    required this.privateDirectory,
    required this.appVersion,
  });

  final Directory privateDirectory;
  final String appVersion;

  static const _channel = MethodChannel('com.pulsys.app/environment');
  static const _unavailable = ApiError(
    code: ApiErrorCodes.storageError,
    message: 'The private application directory is unavailable.',
  );

  static Future<AppEnvironment> load() async {
    try {
      final values = await _channel.invokeMapMethod<String, String>(
        'getEnvironment',
      );
      final path = values?['privateDirectory'];
      final version = values?['appVersion'];
      if (path == null || path.isEmpty || version == null || version.isEmpty) {
        throw _unavailable;
      }
      return AppEnvironment(
        privateDirectory: Directory(path),
        appVersion: version,
      );
    } on PlatformException {
      throw _unavailable;
    } on MissingPluginException {
      throw _unavailable;
    }
  }
}
