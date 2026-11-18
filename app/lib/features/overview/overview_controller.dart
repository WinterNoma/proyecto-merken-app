import 'package:flutter/foundation.dart';

import '../../platform/pulsys_api.dart';

sealed class OverviewResult<T> {
  const OverviewResult();

  T? get value => switch (this) {
    OverviewCurrent<T>(:final data) => data,
    OverviewPrevious<T>(:final data) => data,
    _ => null,
  };

  bool get needsRetry =>
      this is OverviewPrevious<T> || this is OverviewFailure<T>;
}

final class OverviewLoading<T> extends OverviewResult<T> {
  const OverviewLoading();
}

final class OverviewCurrent<T> extends OverviewResult<T> {
  const OverviewCurrent(this.data, this.loadedAtUtc);
  final T data;
  final DateTime loadedAtUtc;
}

final class OverviewPrevious<T> extends OverviewResult<T> {
  const OverviewPrevious(this.data, this.loadedAtUtc);
  final T data;
  final DateTime loadedAtUtc;
}

final class OverviewFailure<T> extends OverviewResult<T> {
  const OverviewFailure();
}

class OverviewController extends ChangeNotifier {
  OverviewController({required this._api, DateTime Function()? clock})
    : _clock = clock ?? DateTime.now;

  final PulsysApi _api;
  final DateTime Function() _clock;
  OverviewResult<ZoneSnapshot> status = const OverviewLoading();
  OverviewResult<List<PulsePoint>> readings = const OverviewLoading();
  bool loading = true;
  bool _disposed = false;
  int _generation = 0;

  ZoneSnapshot? get snapshot => status.value;
  PulsePoint? get latestPulse => readings.value?.firstOrNull;
  bool get statusFailed => status.needsRetry;
  bool get historyFailed => readings.needsRetry;

  PulseEvaluation? get currentPulseEvaluation {
    if (status is! OverviewCurrent<ZoneSnapshot> ||
        readings is! OverviewCurrent<List<PulsePoint>>) {
      return null;
    }
    final pulse = snapshot?.pulse;
    final point = latestPulse;
    return point == null || pulse?.evaluatedRecordId == point.recordId
        ? pulse
        : null;
  }

  Future<void> load() async {
    if (_disposed) return;
    final generation = ++_generation;
    loading = true;
    notifyListeners();
    final statusFuture = _read(_api.zoneStatus, status);
    final pulseFuture = _read(
      () => _api.effectivePulseHistory(HistoryQuery(limit: 1)),
      readings,
    );
    final nextStatus = await statusFuture;
    final nextReadings = await pulseFuture;
    if (_disposed || generation != _generation) return;
    status = nextStatus;
    readings = nextReadings;
    loading = false;
    notifyListeners();
  }

  Future<OverviewResult<T>> _read<T>(
    Future<T> Function() query,
    OverviewResult<T> previous,
  ) async {
    try {
      return OverviewCurrent(await query(), _clock().toUtc());
    } on ApiError {
      return switch (previous) {
        OverviewCurrent<T>(:final data, :final loadedAtUtc) ||
        OverviewPrevious<T>(
          :final data,
          :final loadedAtUtc,
        ) => OverviewPrevious(data, loadedAtUtc),
        _ => OverviewFailure<T>(),
      };
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
