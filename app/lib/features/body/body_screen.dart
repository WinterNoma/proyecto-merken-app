import 'dart:async';

import 'package:material_ui/material_ui.dart';

import '../../design/tokens/spacing.dart';
import '../../design/tokens/typography.dart';
import '../../l10n/generated/app_localizations.dart';
import '../../platform/pulsys_api.dart';
import 'body_controller.dart';
import 'body_detail.dart';
import 'body_figure.dart';
import 'body_presentation.dart';

class BodyScreen extends StatefulWidget {
  const BodyScreen({super.key, required this.api});

  final PulsysApi api;

  @override
  State<BodyScreen> createState() => _BodyScreenState();
}

class _BodyScreenState extends State<BodyScreen> {
  late BodyController _controller;

  @override
  void initState() {
    super.initState();
    _loadSession();
  }

  void _loadSession() {
    _controller = BodyController(widget.api);
    unawaited(_controller.load());
  }

  @override
  void didUpdateWidget(BodyScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(widget.api, oldWidget.api)) {
      _controller.dispose();
      _loadSession();
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l10n.bodyTitle)),
      body: SafeArea(
        child: ListenableBuilder(
          listenable: _controller,
          builder: (context, _) => SingleChildScrollView(
            padding: const EdgeInsets.all(PulsysSpacing.s4),
            child: switch (_controller.state) {
              BodyLoading() => Text(l10n.bodyLoading),
              BodyLoadError() => Column(
                children: <Widget>[
                  Text(l10n.bodyLoadError),
                  OutlinedButton(
                    onPressed: _controller.load,
                    child: Text(l10n.bodyRetry),
                  ),
                ],
              ),
              final BodyLoaded data => BodyPanel(snapshot: data.snapshot),
            },
          ),
        ),
      ),
    );
  }
}

class BodyStage extends StatelessWidget {
  const BodyStage({
    super.key,
    this.leading,
    this.trailing,
    required this.child,
  });

  final Widget? leading;
  final Widget? trailing;
  final Widget child;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final withSigns = leading != null || trailing != null;
      final width = withSigns
          ? (constraints.maxWidth * .52).clamp(160.0, 256.0)
          : constraints.maxWidth.clamp(160.0, 256.0);
      return Row(
        mainAxisAlignment: MainAxisAlignment.center,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          if (leading case final signs?) ...<Widget>[
            Expanded(child: signs),
            const SizedBox(width: PulsysSpacing.s2),
          ],
          SizedBox(width: width, child: child),
          if (trailing case final signs?) ...<Widget>[
            const SizedBox(width: PulsysSpacing.s2),
            Expanded(child: signs),
          ],
        ],
      );
    },
  );
}

class BodyPanel extends StatefulWidget {
  const BodyPanel({
    super.key,
    required this.snapshot,
    this.leading,
    this.trailing,
  });

  final ZoneSnapshot snapshot;
  final Widget? leading;
  final Widget? trailing;

  @override
  State<BodyPanel> createState() => _BodyPanelState();
}

class _BodyPanelState extends State<BodyPanel> {
  DialogRoute<void>? _detail;
  BodyZone? _selected;
  bool _showList = false;

  @override
  void didUpdateWidget(BodyPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.snapshot.isDemo != oldWidget.snapshot.isDemo) {
      _discardDetail();
      _selected = null;
    }
  }

  void _discardDetail() {
    final route = _detail;
    _detail = null;
    if (route != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (route.isActive) {
          route.navigator?.removeRoute(route);
        }
      });
    }
  }

  @override
  void dispose() {
    _discardDetail();
    super.dispose();
  }

  Future<void> _select(BodyZone zone, BodyLoaded data) async {
    if (_detail != null) {
      return;
    }
    setState(() => _selected = zone);
    final route = DialogRoute<void>(
      context: context,
      builder: (_) => BodyDetail(
        zone: zone,
        state: data.zones[zone],
        snapshot: data.snapshot,
      ),
    );
    _detail = route;
    await Navigator.of(context).push(route);
    if (identical(_detail, route)) {
      _detail = null;
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final data = BodyLoaded(widget.snapshot);
    void select(BodyZone zone) => unawaited(_select(zone, data));
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (data.snapshot.isDemo) Text(l10n.bodyDemo),
        OutlinedButton(
          onPressed: () => setState(() => _showList = !_showList),
          child: Text(_showList ? l10n.bodyShowFigure : l10n.bodyShowList),
        ),
        const SizedBox(height: PulsysSpacing.s2),
        Semantics(
          header: true,
          child: Text(
            _showList ? l10n.bodyList : l10n.bodyFront,
            style: PulsysTypography.componentHeading,
          ),
        ),
        const SizedBox(height: PulsysSpacing.s2),
        BodyStage(
          key: const ValueKey<String>('body-stage'),
          leading: widget.leading,
          trailing: widget.trailing,
          child: _showList
              ? Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    for (final zone in BodyZone.values)
                      Padding(
                        padding: const EdgeInsets.only(
                          bottom: PulsysSpacing.actionGap,
                        ),
                        child: BodyZoneButton(
                          key: ValueKey('list-${zone.id(l10n)}'),
                          zone: zone,
                          status: BodyStatus.from(data.zones[zone]?.status),
                          selected: _selected == zone,
                          onSelect: select,
                        ),
                      ),
                  ],
                )
              : BodyFigure(
                  zones: data.zones,
                  selected: _selected,
                  onSelect: select,
                ),
        ),
        if (!_showList)
          for (final zone in const <BodyZone>[BodyZone.back, BodyZone.other])
            Padding(
              padding: const EdgeInsets.only(bottom: PulsysSpacing.actionGap),
              child: BodyZoneButton(
                key: ValueKey('figure-${zone.id(l10n)}'),
                zone: zone,
                status: BodyStatus.from(data.zones[zone]?.status),
                selected: _selected == zone,
                onSelect: select,
              ),
            ),
        if (!_showList) _legend(data),
      ],
    );
  }

  Widget _legend(BodyLoaded data) {
    final statuses = {
      for (final zone in BodyZone.values)
        BodyStatus.from(data.zones[zone]?.status),
    };
    return DecoratedBox(
      key: const ValueKey('body-status-legend'),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surface,
        border: Border(
          top: BorderSide(color: Theme.of(context).colorScheme.outlineVariant),
        ),
      ),
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxHeight: MediaQuery.sizeOf(context).height * .28,
        ),
        child: SingleChildScrollView(
          primary: false,
          padding: const EdgeInsets.symmetric(
            horizontal: PulsysSpacing.s4,
            vertical: PulsysSpacing.s2,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                AppLocalizations.of(context).bodyLegendTitle,
                style: PulsysTypography.label,
              ),
              const SizedBox(height: PulsysSpacing.s1),
              Wrap(
                spacing: PulsysSpacing.s4,
                runSpacing: PulsysSpacing.s1,
                children: [
                  for (final status in BodyStatus.values)
                    if (statuses.contains(status)) BodyStatusLabel(status),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
