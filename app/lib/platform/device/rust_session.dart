import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;

import '../generated/api.dart' as ffi;
import '../generated/frb_generated.dart' show RustLib;

abstract interface class RustSession {
  Future<ffi.PreparedPulse> preparePulse(ffi.PulseInput input);
  Future<ffi.SaveOutcome> confirmPulse(ffi.PreparedPulse action);
  Future<Uint8List> encodePreparedPulse(ffi.PreparedPulse action);
  Future<ffi.PreparedPulse> decodePreparedPulse(Uint8List bytes);
  Future<List<ffi.PulsePoint>> effectivePulseHistory(ffi.HistoryQuery query);
  Future<ffi.ZoneSnapshot> zoneStatus();
  Future<void> close();
}

final class NativeRustSession implements RustSession {
  NativeRustSession._(this._session);

  static Future<void>? _initialized;
  final ffi.Session _session;

  static Future<NativeRustSession> open({
    required Directory privateDirectory,
    required String appVersion,
  }) async {
    await (_initialized ??= RustLib.init(
      externalLibrary: Platform.isIOS
          ? ExternalLibrary.process(iKnowHowToUseIt: true)
          : null,
    ));
    return NativeRustSession._(
      await ffi.openStore(
        privateDir: privateDirectory.path,
        appVersion: appVersion,
      ),
    );
  }

  @override
  Future<ffi.PreparedPulse> preparePulse(ffi.PulseInput input) =>
      ffi.preparePulse(session: _session, input: input);

  @override
  Future<ffi.SaveOutcome> confirmPulse(ffi.PreparedPulse action) =>
      ffi.confirmPulse(session: _session, action: action);

  @override
  Future<Uint8List> encodePreparedPulse(ffi.PreparedPulse action) =>
      ffi.encodePreparedPulse(action: action);

  @override
  Future<ffi.PreparedPulse> decodePreparedPulse(Uint8List bytes) =>
      ffi.decodePreparedPulse(bytes: bytes);

  @override
  Future<List<ffi.PulsePoint>> effectivePulseHistory(ffi.HistoryQuery query) =>
      ffi.effectivePulseHistory(session: _session, query: query);

  @override
  Future<ffi.ZoneSnapshot> zoneStatus() => ffi.zoneStatus(session: _session);

  @override
  Future<void> close() async {
    try {
      await ffi.closeStore(session: _session);
    } finally {
      _session.dispose();
    }
  }
}
