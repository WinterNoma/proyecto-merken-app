import 'package:flutter/foundation.dart';

import '../../platform/pulsys_api.dart';

sealed class BodyLoadState {
  const BodyLoadState();
}

final class BodyLoading extends BodyLoadState {
  const BodyLoading();
}

final class BodyLoadError extends BodyLoadState {
  const BodyLoadError();
}

final class BodyLoaded extends BodyLoadState {
  BodyLoaded(this.snapshot)
    : zones = Map.unmodifiable({
        for (final zone in snapshot.zones) zone.zoneId: zone,
      });

  final ZoneSnapshot snapshot;
  final Map<BodyZone, ZoneState> zones;
}

final class BodyController extends ChangeNotifier {
  BodyController(this._api);

  final PulsysApi _api;
  BodyLoadState _state = const BodyLoading();
  bool _disposed = false;
  bool _pending = false;

  BodyLoadState get state => _state;

  Future<void> load() async {
    if (_pending || _disposed) {
      return;
    }
    _pending = true;
    _state = const BodyLoading();
    notifyListeners();
    try {
      _state = BodyLoaded(await _api.zoneStatus());
    } catch (_) {
      _state = const BodyLoadError();
    } finally {
      _pending = false;
      if (!_disposed) {
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
