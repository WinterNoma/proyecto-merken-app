import 'package:flutter/semantics.dart' show SemanticsRole;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:pulsys/design/theme.dart';
import 'package:pulsys/features/history/history_screen.dart';
import 'package:pulsys/features/history/history_controller.dart';
import 'package:pulsys/features/history/pulse_trend_painter.dart';
import 'package:pulsys/features/history/pulse_trend.dart';
import 'package:pulsys/l10n/generated/app_localizations.dart';
import 'package:pulsys/platform/pulsys_api.dart';

import '../fakes/fake_pulsys_api.dart';

const _plotKey = ValueKey<String>('history-trend-plot');
const _tableKey = ValueKey<String>('history-readings-table');

void main() {
  testWidgets('shows loading and an empty effective history', (tester) async {
    const latency = Duration(milliseconds: 200);
    final api = FakePulsysApi(latency: latency);

    await _mountHistory(tester, api);
    expect(find.text('Loading pulse history…'), findsOneWidget);
    expect(find.byKey(_plotKey), findsNothing);
    expect(find.byKey(_tableKey), findsNothing);

    await tester.pump(latency);
    expect(find.text('Loading pulse history…'), findsOneWidget);
    await tester.pump(latency);
    await tester.pumpAndSettle();
    expect(find.text('Loading pulse history…'), findsNothing);
    expect(find.text('No pulse readings recorded yet.'), findsOneWidget);
    expect(find.byKey(_plotKey), findsNothing);
    expect(find.byKey(_tableKey), findsNothing);
    expect(
      find.byKey(const ValueKey('history-demo-availability')),
      findsOneWidget,
    );
    expect(
      find.text('DEMO scenarios are not available in this version.'),
      findsOneWidget,
    );
    await tester.tap(find.widgetWithText(ChoiceChip, '6 h'));
    await tester.pumpAndSettle();
    expect(find.text('No pulse readings recorded yet.'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('history-demo-availability')),
      findsOneWidget,
    );
  });

  testWidgets('a symptom without pulses does not offer DEMO', (tester) async {
    final api = FakePulsysApi(
      initialZones: <ZoneState>[
        ZoneState(
          zoneId: BodyZone.head,
          status: EvaluationStatus.change,
          symptoms: <SymptomEvaluation>[
            SymptomEvaluation(
              status: EvaluationStatus.change,
              evaluatedAtUtc: DateTime.utc(2026, 11, 18),
              ruleId: 'fixture.symptom',
              ruleVersion: 'fixture-v1',
              sourceKind: SourceKind.demoHeuristic,
              inputRecordIds: const <String>['symptom-only'],
              inputEventIds: const <String>[],
            ),
          ],
        ),
      ],
    );
    await _mountHistory(tester, api);
    await tester.pumpAndSettle();
    expect(find.text('No pulse readings recorded yet.'), findsOneWidget);
    expect(api.historySnapshot, isEmpty);
    expect(api.zoneStatusCalls, 1);
    expect(
      find.byKey(const ValueKey('history-demo-availability')),
      findsNothing,
    );
  });

  testWidgets('other records without pulses do not offer DEMO', (tester) async {
    await _mountHistory(tester, FakePulsysApi(hasOtherRecords: true));
    await tester.pumpAndSettle();
    expect(find.text('No pulse readings recorded yet.'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('history-demo-availability')),
      findsNothing,
    );
  });

  testWidgets('failed emptiness query never offers DEMO without proof', (
    tester,
  ) async {
    final api = FakePulsysApi()
      ..enqueueZoneStatusError(
        const ApiError(code: ApiErrorCodes.busy, message: 'fixture'),
      );
    await _mountHistory(tester, api);
    await tester.pumpAndSettle();
    expect(find.text('No pulse readings recorded yet.'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('history-demo-availability')),
      findsNothing,
    );
  });

  testWidgets('retries a failed query and draws a single reading', (
    tester,
  ) async {
    final point = _reading(0);
    final api = FakePulsysApi(initialHistory: <PulsePoint>[point])
      ..enqueueHistoryError(
        const ApiError(
          code: ApiErrorCodes.storageError,
          message: 'Temporary store failure',
        ),
      );

    await _mountHistory(tester, api);
    await tester.pumpAndSettle();
    expect(find.text('Pulse history could not be loaded.'), findsOneWidget);
    expect(find.byKey(_plotKey), findsNothing);

    await tester.tap(find.widgetWithText(OutlinedButton, 'Retry'));
    await tester.pumpAndSettle();

    final plot = find.byKey(_plotKey);
    expect(plot, paintsExactlyCountTimes(#drawCircle, 1));
    expect(plot, paintsExactlyCountTimes(#drawPath, 0));
    expect(find.byKey(_readingKey(point)), findsOneWidget);
    expect(find.text('Pulse history could not be loaded.'), findsNothing);
    expect(api.historyQueries.map((query) => query.limit), <int>[2000, 2000]);
  });

  for (final sourceCount in <int>[30, 2000]) {
    testWidgets('draws 30 readings from $sourceCount effective readings', (
      tester,
    ) async {
      final minimumIndex = sourceCount ~/ 4;
      final maximumIndex = minimumIndex + 1;
      final extremaBpm = <int, double>{
        minimumIndex: 54.25,
        maximumIndex: 125.75,
      };
      final points = List<PulsePoint>.generate(
        sourceCount,
        (index) => _reading(
          index,
          bpm: extremaBpm[index] ?? (70 + index % 9).toDouble(),
        ),
      );
      final api = FakePulsysApi(initialHistory: points);

      await _mountHistory(tester, api);
      await tester.pumpAndSettle();

      final plot = find.byKey(_plotKey);
      expect(plot, paintsExactlyCountTimes(#drawCircle, 30));
      final painter = tester.widget<CustomPaint>(plot).painter;
      final trend = (painter as PulseTrendPainter).trend;
      expect(api.historyQueries.single.limit, 2000);
      expect(trend.sourceCount, sourceCount);
      expect(trend.points, hasLength(30));
      expect(trend.points.first, same(points.first));
      expect(trend.points.last, same(points.last));
      expect(
        trend.points,
        containsAllInOrder(<PulsePoint>[
          points[minimumIndex],
          points[maximumIndex],
        ]),
      );
      expect(trend.minimumBpm, Bpm(54.25));
      expect(trend.maximumBpm, Bpm(125.75));

      final tableRows = find.descendant(
        of: find.byKey(_tableKey),
        matching: find.byWidgetPredicate(
          (widget) =>
              widget is Semantics &&
              widget.properties.role == SemanticsRole.row,
        ),
      );
      expect(tableRows, findsNWidgets(31));
      for (final point in trend.points) {
        expect(find.byKey(_readingKey(point)), findsOneWidget);
      }

      final summary = find.text('Showing 30 of the latest 2,000 readings.');
      if (sourceCount == 30) {
        expect(trend.points, orderedEquals(points));
        expect(summary, findsNothing);
      } else {
        expect(summary, findsOneWidget);
      }
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('uses local insertion order for readings at the same instant', (
    tester,
  ) async {
    final instant = DateTime.utc(2026, 11, 18, 10);
    final points = <PulsePoint>[
      _reading(0, recordId: 'z', measuredAtUtc: instant),
      _reading(1, recordId: 'a', measuredAtUtc: instant),
      _reading(2, recordId: 'm', measuredAtUtc: instant),
    ];
    final api = FakePulsysApi(initialHistory: points);
    final latest = await api.effectivePulseHistory(HistoryQuery(limit: 2));
    expect(latest.map((point) => point.recordId), <String>['m', 'a']);

    await _mountHistory(tester, api);
    await tester.pumpAndSettle();
    final painter = tester.widget<CustomPaint>(find.byKey(_plotKey)).painter;
    final trend = (painter as PulseTrendPainter).trend;
    expect(trend.points, orderedEquals(points));
    final tableRows = find.descendant(
      of: find.byKey(_tableKey),
      matching: find.byWidgetPredicate(
        (widget) =>
            widget is Semantics && widget.properties.role == SemanticsRole.row,
      ),
    );
    expect(
      tester.widgetList<Semantics>(tableRows).skip(1).map((row) => row.key),
      <ValueKey<String>>[
        _readingKey(points[2]),
        _readingKey(points[1]),
        _readingKey(points[0]),
      ],
    );
  });

  testWidgets('draws 30 distinct records when all values and times coincide', (
    tester,
  ) async {
    final instant = DateTime.utc(2026, 11, 18, 10);
    final points = List<PulsePoint>.generate(
      31,
      (index) => _reading(index, measuredAtUtc: instant),
    );
    await _mountHistory(tester, FakePulsysApi(initialHistory: points));
    await tester.pumpAndSettle();

    final plot = find.byKey(_plotKey);
    expect(plot, paintsExactlyCountTimes(#drawCircle, 30));
    final painter = tester.widget<CustomPaint>(plot).painter;
    final trend = (painter as PulseTrendPainter).trend;
    expect(trend.points.map((point) => point.recordId).toSet(), hasLength(30));
    expect(tester.takeException(), isNull);
  });

  testWidgets('exposes table cells with UTC, units and demo at 2x text', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(320, 720);
    tester.view.devicePixelRatio = 1;
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    final semantics = tester.ensureSemantics();

    final point = _reading(
      0,
      bpm: 72.25,
      measuredAtUtc: DateTime.utc(2026, 11, 18, 10, 0, 0, 789),
      isDemo: true,
    );
    await _mountHistory(
      tester,
      FakePulsysApi(initialHistory: <PulsePoint>[point]),
    );
    await tester.pumpAndSettle();
    final row = find.byKey(_readingKey(point));
    await tester.ensureVisible(row);
    await tester.pumpAndSettle();

    final table = tester.getSemantics(find.byKey(_tableKey));
    expect(table.role, SemanticsRole.table);
    expect(tester.getSemantics(row).role, SemanticsRole.row);
    final measured = find.bySemanticsLabel(
      'Measured 18 Nov 2026, 10:00:00.789 UTC',
    );
    final pulse = find.bySemanticsLabel('Pulse 72.25 beats per minute');
    expect(measured, findsOneWidget);
    expect(pulse, findsOneWidget);
    expect(tester.getSemantics(measured).role, SemanticsRole.cell);
    expect(tester.getSemantics(pulse).getSemanticsData().value, 'DEMO');
    expect(
      find.descendant(of: row, matching: find.text('72.25 bpm')),
      findsOneWidget,
    );
    expect(MediaQuery.textScalerOf(tester.element(row)).scale(16), 32);
    expect(tester.takeException(), isNull);
    semantics.dispose();
  });

  testWidgets(
    'range buttons filter before sampling more than 30 points and include boundaries',
    (tester) async {
      final now = DateTime.utc(2026, 11, 20, 12);
      final recent = List<PulsePoint>.generate(
        40,
        (index) => _reading(
          82 + index,
          bpm: (65 + index % 13).toDouble(),
          measuredAtUtc: now.subtract(
            Duration(minutes: 360 - index * 360 ~/ 39),
          ),
        ),
      );
      final points = <PulsePoint>[
        for (var index = 0; index < 80; index++)
          _reading(
            index,
            measuredAtUtc: now.subtract(Duration(hours: 168 - index)),
          ),
        _reading(80, measuredAtUtc: now.subtract(const Duration(hours: 25))),
        _reading(81, measuredAtUtc: now.subtract(const Duration(hours: 24))),
        ...recent,
        _reading(122, measuredAtUtc: now.add(const Duration(milliseconds: 1))),
      ];
      final api = FakePulsysApi(initialHistory: points);
      await _mountHistory(tester, api);
      await tester.pumpAndSettle();
      PulseTrend trend() =>
          (tester.widget<CustomPaint>(find.byKey(_plotKey)).painter!
                  as PulseTrendPainter)
              .trend;
      expect(trend().sourceCount, 122);
      expect(trend().points, hasLength(30));
      expect(trend().points.first, points.first);
      expect(trend().points.last, recent.last);
      await tester.tap(find.widgetWithText(ChoiceChip, '24 h'));
      await tester.pumpAndSettle();
      expect(trend().sourceCount, 41);
      expect(trend().points.first, points[81]);
      await tester.tap(find.widgetWithText(ChoiceChip, '6 h'));
      await tester.pumpAndSettle();
      expect(trend().sourceCount, 40);
      final expected = PulseTrend.fromEffectiveHistory(
        recent.reversed.toList(),
      )!;
      expect(trend().points, orderedEquals(expected.points));
      expect(trend().points.first, recent.first);
      expect(trend().points.last, recent.last);
      final sampledFirst =
          PulseTrend.fromEffectiveHistory(
            points.take(122).toList().reversed.toList(),
          )!.points.where(
            (point) =>
                !point.measuredAtUtc.isBefore(recent.first.measuredAtUtc),
          );
      expect(sampledFirst, isNot(orderedEquals(expected.points)));
      expect(api.historyQueries, hasLength(1));
    },
  );

  testWidgets('a range without readings shows the empty state', (tester) async {
    final now = DateTime.utc(2026, 11, 20, 12);
    final api = FakePulsysApi(
      initialHistory: <PulsePoint>[
        _reading(0, measuredAtUtc: now.subtract(const Duration(days: 2))),
      ],
    );
    await _mountHistory(tester, api);
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(ChoiceChip, '6 h'));
    await tester.pumpAndSettle();
    expect(
      find.text('No pulse readings in the selected range (6 h).'),
      findsOneWidget,
    );
    expect(find.text('No pulse readings recorded yet.'), findsNothing);
    expect(
      find.byKey(const ValueKey('history-demo-availability')),
      findsNothing,
    );
    expect(find.byKey(_plotKey), findsNothing);
    await tester.tap(find.widgetWithText(ChoiceChip, '7 days'));
    await tester.pumpAndSettle();
    expect(find.byKey(_plotKey), findsOneWidget);
    expect(
      find.byKey(const ValueKey('history-demo-availability')),
      findsNothing,
    );
  });
}

Future<void> _mountHistory(WidgetTester tester, PulsysApi api) =>
    tester.pumpWidget(
      MaterialApp(
        theme: PulsysTheme.dark(),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: HistoryScreen(
            api: api,
            clock: () => DateTime.utc(2026, 11, 20, 12),
            initialRange: HistoryRange.days7,
          ),
        ),
      ),
    );

PulsePoint _reading(
  int index, {
  double bpm = 72,
  String? recordId,
  DateTime? measuredAtUtc,
  bool isDemo = false,
}) {
  final id = recordId ?? 'point-$index';
  final origin = DateTime.utc(2026, 11, 18, 10);
  final measuredAt = measuredAtUtc ?? origin.add(Duration(minutes: index));
  return PulsePoint(
    recordId: id,
    rootId: id,
    bpm: Bpm(bpm),
    source: RecordSource.form,
    measuredAtUtc: measuredAt,
    loggedAtUtc: measuredAt.add(const Duration(seconds: 1)),
    isDemo: isDemo,
  );
}

ValueKey<String> _readingKey(PulsePoint point) =>
    ValueKey<String>('history-reading-${point.recordId}');
