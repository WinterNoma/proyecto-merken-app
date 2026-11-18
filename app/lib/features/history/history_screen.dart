import 'dart:async';

import 'package:flutter/semantics.dart' show SemanticsRole;
import 'package:intl/intl.dart';
import 'package:material_ui/material_ui.dart';

import '../../design/tokens/spacing.dart';
import '../../design/tokens/typography.dart';
import '../../l10n/generated/app_localizations.dart';
import '../../platform/pulsys_api.dart';
import 'history_controller.dart';
import 'pulse_trend.dart';
import 'pulse_trend_painter.dart';

class HistoryScreen extends StatefulWidget {
  const HistoryScreen({
    super.key,
    required this.api,
    this.beforeTrend,
    this.afterTrend,
    this.refreshToken = 0,
    this.clock,
    this.initialRange = HistoryRange.hours24,
  });

  final PulsysApi api;
  final Widget? beforeTrend;
  final Widget? afterTrend;
  final int refreshToken;
  final DateTime Function()? clock;
  final HistoryRange initialRange;

  @override
  State<HistoryScreen> createState() => _HistoryScreenState();
}

class _HistoryScreenState extends State<HistoryScreen> {
  late HistoryController _controller;

  @override
  void initState() {
    super.initState();
    _openHistory();
  }

  @override
  void didUpdateWidget(covariant HistoryScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.api != oldWidget.api || widget.clock != oldWidget.clock) {
      _controller.dispose();
      _openHistory();
    } else if (widget.refreshToken != oldWidget.refreshToken) {
      unawaited(_controller.load());
    }
  }

  void _openHistory() {
    _controller = HistoryController(
      api: widget.api,
      clock: widget.clock,
      range: widget.initialRange,
    );
    unawaited(_controller.load());
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return SingleChildScrollView(
      key: const PageStorageKey<String>('history-scroll'),
      padding: const EdgeInsets.all(PulsysSpacing.s4),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 640),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(l10n.navHistory, style: PulsysTypography.sectionTitle),
              if (widget.beforeTrend case final before?) ...<Widget>[
                const SizedBox(height: PulsysSpacing.s4),
                before,
              ],
              const SizedBox(height: PulsysSpacing.s4),
              ListenableBuilder(
                listenable: _controller,
                builder: (context, _) => Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    Wrap(
                      spacing: PulsysSpacing.s2,
                      runSpacing: PulsysSpacing.s2,
                      children: <Widget>[
                        for (final range in HistoryRange.values)
                          ChoiceChip(
                            label: Text(switch (range) {
                              HistoryRange.hours6 => l10n.historyRange6Hours,
                              HistoryRange.hours24 => l10n.historyRange24Hours,
                              HistoryRange.days7 => l10n.historyRange7Days,
                            }),
                            selected: _controller.range == range,
                            onSelected: _controller.state is HistoryLoading
                                ? null
                                : (_) => _controller.selectRange(range),
                          ),
                      ],
                    ),
                    const SizedBox(height: PulsysSpacing.s3),
                    switch (_controller.state) {
                      HistoryLoading() => _HistoryNotice(
                        message: l10n.pulseHistoryLoading,
                      ),
                      HistoryEmpty() => _HistoryNotice(
                        message: l10n.pulseHistoryEmptyBody,
                      ),
                      HistoryRangeEmpty() => _HistoryNotice(
                        message: l10n.historyRangeEmpty(
                          switch (_controller.range) {
                            HistoryRange.hours6 => l10n.historyRange6Hours,
                            HistoryRange.hours24 => l10n.historyRange24Hours,
                            HistoryRange.days7 => l10n.historyRange7Days,
                          },
                        ),
                      ),
                      HistoryLoadFailure() => _HistoryNotice(
                        message: l10n.historyUnavailable,
                        onRetry: () => unawaited(_controller.load()),
                      ),
                      HistoryReady(:final trend) => _HistoryContent(
                        trend: trend,
                      ),
                    },
                  ],
                ),
              ),
              if (widget.afterTrend case final after?) ...<Widget>[
                const SizedBox(height: PulsysSpacing.s4),
                after,
              ],
              ListenableBuilder(
                listenable: _controller,
                builder: (context, _) => _controller.canOfferDemo
                    ? Padding(
                        key: const ValueKey('history-demo-availability'),
                        padding: const EdgeInsets.only(top: PulsysSpacing.s4),
                        child: Text(l10n.historyDemoUnavailable),
                      )
                    : const SizedBox.shrink(),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _HistoryNotice extends StatelessWidget {
  const _HistoryNotice({required this.message, this.onRetry});

  final String message;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(PulsysSpacing.s4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Semantics(
              container: true,
              liveRegion: true,
              child: Text(message, style: PulsysTypography.body),
            ),
            if (onRetry case final retry?) ...<Widget>[
              const SizedBox(height: PulsysSpacing.s3),
              OutlinedButton(
                onPressed: retry,
                child: Text(l10n.pulseHistoryRetry),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _HistoryContent extends StatelessWidget {
  const _HistoryContent({required this.trend});

  final PulseTrend trend;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final seconds = DateFormat('dd MMM yyyy, HH:mm:ss', l10n.localeName);
    final milliseconds = DateFormat(
      'dd MMM yyyy, HH:mm:ss.SSS',
      l10n.localeName,
    );
    String timestamp(DateTime instant) =>
        (instant.millisecond == 0 ? seconds : milliseconds).format(instant);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _PulseTrendChart(
          trend: trend,
          start: timestamp(trend.startUtc),
          end: timestamp(trend.endUtc),
        ),
        const SizedBox(height: PulsysSpacing.s5),
        Semantics(
          header: true,
          child: Text(
            l10n.historyReadings,
            style: PulsysTypography.componentHeading,
          ),
        ),
        const SizedBox(height: PulsysSpacing.s2),
        _HistoryTable(points: trend.points, timestamp: timestamp),
      ],
    );
  }
}

class _PulseTrendChart extends StatelessWidget {
  const _PulseTrendChart({
    required this.trend,
    required this.start,
    required this.end,
  });

  final PulseTrend trend;
  final String start;
  final String end;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final minimum = trend.minimumBpm.format();
    final maximum = trend.maximumBpm.format();
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(PulsysSpacing.s4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Semantics(
              header: true,
              child: Text(
                l10n.historyTrendTitle,
                style: PulsysTypography.sectionTitle,
              ),
            ),
            if (trend.hasDemoReadings)
              Text(l10n.historyDemoNotice, style: PulsysTypography.caption),
            if (trend.points.length < trend.sourceCount)
              Text(
                l10n.historySampleSummary(
                  trend.points.length,
                  trend.sourceCount,
                ),
                style: PulsysTypography.caption,
              ),
            const SizedBox(height: PulsysSpacing.s3),
            Text(
              minimum == maximum
                  ? '$minimum ${l10n.pulseBpmUnit}'
                  : l10n.historyPlotRange(minimum, maximum),
              style: PulsysTypography.dataCaption,
            ),
            const SizedBox(height: PulsysSpacing.s2),
            ExcludeSemantics(
              child: RepaintBoundary(
                child: CustomPaint(
                  key: const ValueKey<String>('history-trend-plot'),
                  size: const Size.fromHeight(144),
                  painter: PulseTrendPainter(trend: trend),
                ),
              ),
            ),
            const SizedBox(height: PulsysSpacing.s2),
            Text(l10n.historyTimeAxis, style: PulsysTypography.label),
            if (start == end)
              Text(
                start,
                style: PulsysTypography.dataCaption,
                textAlign: TextAlign.center,
              )
            else
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Expanded(
                    child: Text(start, style: PulsysTypography.dataCaption),
                  ),
                  const SizedBox(width: PulsysSpacing.s3),
                  Expanded(
                    child: Text(
                      end,
                      style: PulsysTypography.dataCaption,
                      textAlign: TextAlign.end,
                    ),
                  ),
                ],
              ),
          ],
        ),
      ),
    );
  }
}

class _HistoryTable extends StatelessWidget {
  const _HistoryTable({required this.points, required this.timestamp});

  final List<PulsePoint> points;
  final String Function(DateTime) timestamp;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(PulsysSpacing.s4),
        child: Semantics(
          key: const ValueKey<String>('history-readings-table'),
          container: true,
          explicitChildNodes: true,
          role: SemanticsRole.table,
          child: Column(
            children: <Widget>[
              _row(
                time: Semantics(
                  container: true,
                  role: SemanticsRole.columnHeader,
                  child: Text(
                    l10n.historyMeasuredColumn,
                    style: PulsysTypography.label,
                  ),
                ),
                pulse: Semantics(
                  container: true,
                  role: SemanticsRole.columnHeader,
                  child: Text(
                    l10n.historyPulseColumn,
                    style: PulsysTypography.label,
                    textAlign: TextAlign.end,
                  ),
                ),
              ),
              for (final point in points.reversed) ...<Widget>[
                const Divider(),
                _reading(l10n, point),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _reading(AppLocalizations l10n, PulsePoint point) {
    final time = timestamp(point.measuredAtUtc);
    final bpm = point.bpm.format();
    return _row(
      key: ValueKey<String>('history-reading-${point.recordId}'),
      time: Semantics(
        container: true,
        excludeSemantics: true,
        role: SemanticsRole.cell,
        label: l10n.historyMeasuredLabel(time),
        child: Text(time, style: PulsysTypography.dataCaption),
      ),
      pulse: Semantics(
        container: true,
        excludeSemantics: true,
        role: SemanticsRole.cell,
        label: l10n.historyPulseLabel(bpm),
        value: point.isDemo ? l10n.pulseDemoTag : null,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              '$bpm ${l10n.pulseBpmUnit}',
              style: PulsysTypography.dataBody,
              textAlign: TextAlign.end,
            ),
            if (point.isDemo)
              Text(
                l10n.pulseDemoTag,
                style: PulsysTypography.label,
                textAlign: TextAlign.end,
              ),
          ],
        ),
      ),
    );
  }

  static Widget _row({Key? key, required Widget time, required Widget pulse}) =>
      Semantics(
        key: key,
        container: true,
        explicitChildNodes: true,
        role: SemanticsRole.row,
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Expanded(flex: 3, child: time),
            const SizedBox(width: PulsysSpacing.s3),
            Expanded(flex: 2, child: pulse),
          ],
        ),
      );
}
