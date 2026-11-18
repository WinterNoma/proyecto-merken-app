import 'dart:async';

import 'package:material_ui/material_ui.dart';

import '../design/tokens/typography.dart';
import '../features/assistant/recommendations_screen.dart';
import '../features/history/history_screen.dart';
import '../features/overview/overview_controller.dart';
import '../features/overview/overview_screen.dart';
import '../features/overview/status_summary.dart';
import '../features/overview/vital_card.dart';
import '../l10n/generated/app_localizations.dart';
import '../platform/pulsys_api.dart';

enum AppPage { history, today, recommendations }

class AppShell extends StatefulWidget {
  const AppShell({
    super.key,
    required this.api,
    required this.openPulse,
    this.clock,
  });

  final PulsysApi api;
  final Future<void> Function(BuildContext context) openPulse;
  final DateTime Function()? clock;

  @override
  State<AppShell> createState() => _AppShellState();
}

class _AppShellState extends State<AppShell> {
  final PageController _pages = PageController(initialPage: 1);
  late OverviewController _controller;
  AppPage _page = AppPage.today;
  int _revision = 0;

  @override
  void initState() {
    super.initState();
    _loadSession();
  }

  void _loadSession() {
    _controller = OverviewController(api: widget.api, clock: widget.clock);
    unawaited(_controller.load());
  }

  @override
  void didUpdateWidget(AppShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(widget.api, oldWidget.api)) {
      _controller.dispose();
      _revision++;
      _loadSession();
    }
  }

  @override
  void dispose() {
    _pages.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _selectPage(AppPage page) {
    FocusManager.instance.primaryFocus?.unfocus();
    _pages.jumpToPage(page.index);
    setState(() => _page = page);
  }

  Future<void> _openPulse() async {
    await widget.openPulse(context);
    if (!mounted) {
      return;
    }
    setState(() => _revision++);
    await _controller.load();
  }

  Future<void> _showHelp() => showDialog<void>(
    context: context,
    builder: (context) {
      final l10n = AppLocalizations.of(context);
      return AlertDialog(
        title: Text(l10n.navNeedHelp),
        content: Text(l10n.navHelpStage4),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(l10n.bodyClose),
          ),
        ],
      );
    },
  );

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return PopScope<void>(
      canPop: _page == AppPage.today,
      onPopInvokedWithResult: (didPop, result) {
        if (!didPop) {
          _selectPage(AppPage.today);
        }
      },
      child: Scaffold(
        body: SafeArea(
          child: Column(
            children: <Widget>[
              Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: <Widget>[
                  Expanded(
                    child: TextButton(
                      key: const ValueKey<String>('nav-home'),
                      onPressed: () => _selectPage(AppPage.today),
                      child: Text(
                        l10n.wordmark,
                        style: PulsysTypography.wordmark,
                      ),
                    ),
                  ),
                  Expanded(
                    child: OutlinedButton(
                      key: const ValueKey<String>('nav-crew'),
                      onPressed: null,
                      child: Text(l10n.pulseLocalProfile),
                    ),
                  ),
                  Expanded(
                    child: TextButton(
                      key: const ValueKey<String>('nav-help'),
                      onPressed: () => unawaited(_showHelp()),
                      child: Text(l10n.navNeedHelp),
                    ),
                  ),
                ],
              ),
              Expanded(
                child: ListenableBuilder(
                  listenable: _controller,
                  builder: (context, _) => Column(
                    children: <Widget>[
                      if (_controller.loading) Text(l10n.navLoadingStatus),
                      if (_controller.statusFailed || _controller.historyFailed)
                        OutlinedButton(
                          onPressed: () => unawaited(_controller.load()),
                          child: Text(l10n.navRetryStatus),
                        ),
                      Expanded(
                        child: PageView(
                          key: const ValueKey<String>('app-pages'),
                          controller: _pages,
                          onPageChanged: (index) =>
                              setState(() => _page = AppPage.values[index]),
                          children: <Widget>[
                            HistoryScreen(
                              api: widget.api,
                              clock: widget.clock,
                              refreshToken: _revision,
                              beforeTrend: Column(
                                crossAxisAlignment: CrossAxisAlignment.stretch,
                                children: <Widget>[
                                  if (_controller.snapshot?.pulse.persistent ??
                                      false)
                                    PersistentPulseAlert(
                                      controller: _controller,
                                    ),
                                  if (_controller.snapshot case final snapshot?)
                                    ProfileSummary(
                                      snapshot: snapshot,
                                      notice: overviewResultNotice(
                                        l10n,
                                        _controller.status,
                                        l10n.navEvaluation,
                                        previousAtUtc: snapshot.evaluatedAtUtc,
                                      ),
                                    ),
                                ],
                              ),
                              afterTrend: IndicatorList(
                                controller: _controller,
                                onOpenPulse: () => unawaited(_openPulse()),
                              ),
                            ),
                            OverviewScreen(
                              controller: _controller,
                              onOpenPulse: () => unawaited(_openPulse()),
                            ),
                            RecommendationsScreen(controller: _controller),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              Row(
                key: const ValueKey<String>('page-indicators'),
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  for (final page in AppPage.values)
                    Expanded(
                      child: _PageButton(
                        page: page,
                        selected: _page == page,
                        onSelect: () => _selectPage(page),
                      ),
                    ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _PageButton extends StatelessWidget {
  const _PageButton({
    required this.page,
    required this.selected,
    required this.onSelect,
  });

  final AppPage page;
  final bool selected;
  final VoidCallback onSelect;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final label = switch (page) {
      AppPage.history => l10n.navHistory,
      AppPage.today => l10n.navToday,
      AppPage.recommendations => l10n.navRecommendations,
    };
    return Semantics(
      key: ValueKey<String>('nav-${page.name}'),
      container: true,
      button: true,
      selected: selected,
      label: label,
      onTap: onSelect,
      excludeSemantics: true,
      child: TextButton(
        onPressed: onSelect,
        style: TextButton.styleFrom(
          padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 12),
        ),
        child: FittedBox(
          fit: BoxFit.scaleDown,
          child: Text(
            label,
            maxLines: 1,
            softWrap: false,
            textAlign: TextAlign.center,
            style: selected ? PulsysTypography.bodyStrong : null,
          ),
        ),
      ),
    );
  }
}
