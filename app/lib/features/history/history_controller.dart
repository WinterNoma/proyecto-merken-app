import 'package:flutter/foundation.dart';

import '../../platform/pulsys_api.dart';
import 'pulse_trend.dart';

enum HistoryRange {
  hours6(Duration(hours: 6)),
  hours24(Duration(hours: 24)),
  days7(Duration(days: 7));

  const HistoryRange(this.duration);

  final Duration duration;
}

enum StoreEmptiness { unknown, empty, occupied }

sealed class HistoryState {
  const HistoryState();
}

final class HistoryLoading extends HistoryState {
  const HistoryLoading();
}

final class HistoryEmpty extends HistoryState {
  const HistoryEmpty();
}

final class HistoryRangeEmpty extends HistoryState {
  const HistoryRangeEmpty();
}

final class HistoryLoadFailure extends HistoryState {
  const HistoryLoadFailure();
}

final class HistoryReady extends HistoryState {
  const HistoryReady(this.trend);

  final PulseTrend trend;
}

final class HistoryController extends ChangeNotifier {
  HistoryController({
    required this._api,
    DateTime Function()? clock,
    this._range = HistoryRange.hours24,
  }) : _clock = clock ?? DateTime.now;

  static const int sourceLimit = 2000;

  final PulsysApi _api;
  final DateTime Function() _clock;
  HistoryRange _range;
  List<PulsePoint> _points = const <PulsePoint>[];
  DateTime? _throughUtc;
  HistoryState _state = const HistoryLoading();
  bool _requestInFlight = false;
  bool _disposed = false;
  int _generation = 0;
  StoreEmptiness _storeEmptiness = StoreEmptiness.unknown;

  HistoryState get state => _state;
  HistoryRange get range => _range;
  bool get canOfferDemo =>
      _state is HistoryEmpty && _storeEmptiness == StoreEmptiness.empty;

  Future<void> load() async {
    if (_disposed) {
      return;
    }
    final generation = ++_generation;
    _requestInFlight = true;
    _storeEmptiness = StoreEmptiness.unknown;
    _update(const HistoryLoading());
    try {
      final throughUtc = _clock().toUtc();
      final points = await _api.effectivePulseHistory(
        HistoryQuery(limit: sourceLimit),
      );
      if (_disposed || generation != _generation) {
        return;
      }
      final emptiness = points.isEmpty
          ? await _loadStoreEmptiness()
          : StoreEmptiness.occupied;
      if (_disposed || generation != _generation) {
        return;
      }
      _points = points;
      _storeEmptiness = emptiness;
      _throughUtc = throughUtc;
      _filter();
    } on ApiError {
      if (generation == _generation) {
        _update(const HistoryLoadFailure());
      }
    } finally {
      if (generation == _generation) {
        _requestInFlight = false;
      }
    }
  }

  Future<StoreEmptiness> _loadStoreEmptiness() async {
    try {
      return (await _api.zoneStatus()).isStoreEmpty
          ? StoreEmptiness.empty
          : StoreEmptiness.occupied;
    } on ApiError {
      return StoreEmptiness.unknown;
    }
  }

  void selectRange(HistoryRange range) {
    if (_requestInFlight || _disposed || range == _range) {
      return;
    }
    _range = range;
    if (_state is HistoryReady ||
        _state is HistoryEmpty ||
        _state is HistoryRangeEmpty) {
      _filter();
    } else {
      notifyListeners();
    }
  }

  void _filter() {
    final throughUtc = _throughUtc;
    if (throughUtc == null) {
      return;
    }
    final fromUtc = throughUtc.subtract(_range.duration);
    final filtered = _points
        .where(
          (point) =>
              !point.measuredAtUtc.isBefore(fromUtc) &&
              !point.measuredAtUtc.isAfter(throughUtc),
        )
        .toList(growable: false);
    final trend = PulseTrend.fromEffectiveHistory(filtered);
    _update(
      _points.isEmpty
          ? const HistoryEmpty()
          : trend == null
          ? const HistoryRangeEmpty()
          : HistoryReady(trend),
    );
  }

  void _update(HistoryState state) {
    if (_disposed || identical(_state, state)) {
      return;
    }
    _state = state;
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
