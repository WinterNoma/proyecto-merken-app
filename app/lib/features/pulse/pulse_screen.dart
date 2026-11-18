import 'dart:async';

import 'package:material_ui/material_ui.dart';

import '../../design/theme.dart';
import '../../design/tokens/colors.dart';
import '../../design/tokens/spacing.dart';
import '../../design/tokens/typography.dart';
import '../../l10n/generated/app_localizations.dart';
import '../../platform/pulsys_api.dart';
import 'pulse_confirmation_dialog.dart';
import 'pulse_controller.dart';
import 'pulse_format.dart';

class PulsePage extends StatefulWidget {
  const PulsePage({required this.api, super.key});

  final PulsysApi api;

  @override
  State<PulsePage> createState() => _PulsePageState();
}

class _PulsePageState extends State<PulsePage> {
  late final PulseController _controller;

  @override
  void initState() {
    super.initState();
    _controller = PulseController(api: widget.api);
    unawaited(_controller.loadInitial());
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return PulseScreen(controller: _controller);
  }
}

class PulseScreen extends StatefulWidget {
  const PulseScreen({required this.controller, super.key});

  final PulseController controller;

  @override
  State<PulseScreen> createState() => _PulseScreenState();
}

class _PulseScreenState extends State<PulseScreen> {
  final TextEditingController _bpmController = TextEditingController();
  final TextEditingController _timeController = TextEditingController();
  final TextEditingController _noteController = TextEditingController();
  final FocusNode _saveFocusNode = FocusNode();
  bool _dialogOpen = false;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_onControllerChanged);
    _onControllerChanged();
  }

  @override
  void didUpdateWidget(covariant PulseScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_onControllerChanged);
      widget.controller.addListener(_onControllerChanged);
      _onControllerChanged();
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_onControllerChanged);
    _bpmController.dispose();
    _timeController.dispose();
    _noteController.dispose();
    _saveFocusNode.dispose();
    super.dispose();
  }

  void _onControllerChanged() {
    _syncText(_bpmController, widget.controller.bpmText);
    _syncText(_timeController, widget.controller.measuredAtText);
    _syncText(_noteController, widget.controller.noteText);
    if (widget.controller.pendingConfirmation != null &&
        !_dialogOpen &&
        mounted) {
      _dialogOpen = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          unawaited(_openConfirmations());
        }
      });
    }
  }

  void _syncText(TextEditingController field, String text) {
    if (field.text != text) {
      field.value = TextEditingValue(
        text: text,
        selection: TextSelection.collapsed(offset: text.length),
      );
    }
  }

  Future<void> _openConfirmations() async {
    try {
      while (true) {
        if (!mounted) {
          return;
        }
        final prepared = widget.controller.pendingConfirmation;
        if (prepared == null) {
          break;
        }
        final confirmed = await showPulseConfirmationDialog(
          context: context,
          prepared: prepared,
        );
        if (!mounted) {
          return;
        }
        if (confirmed) {
          await widget.controller.confirmPending();
        } else {
          await widget.controller.cancelConfirmation();
        }
      }
    } finally {
      _dialogOpen = false;
      if (mounted) {
        _saveFocusNode.requestFocus();
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final controller = widget.controller;

    return ListenableBuilder(
      listenable: controller,
      builder: (BuildContext context, Widget? child) {
        final snapshot = controller.zoneSnapshot;
        final isDemo = snapshot?.isDemo ?? false;

        return Scaffold(
          appBar: AppBar(
            title: Text(l10n.wordmark),
            actions: <Widget>[
              if (isDemo)
                Padding(
                  padding: const EdgeInsets.only(right: PulsysSpacing.s4),
                  child: _TagChip(label: l10n.pulseDemoTag),
                ),
            ],
          ),
          body: SafeArea(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(PulsysSpacing.s4),
              child: Center(
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 640),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      Semantics(
                        header: true,
                        child: Text(
                          l10n.pulseScreenTitle,
                          style: PulsysTypography.sectionTitle,
                        ),
                      ),
                      const SizedBox(height: PulsysSpacing.s1),
                      Text(
                        l10n.pulseSectionSubtitle,
                        style: PulsysTypography.caption,
                      ),
                      const SizedBox(height: PulsysSpacing.s4),
                      if (snapshot != null) ...<Widget>[
                        _PulseStatusCard(
                          snapshot: snapshot,
                          isStale: controller.zonesLoadFailed,
                        ),
                        const SizedBox(height: PulsysSpacing.s4),
                      ],
                      if (controller.zonesLoadFailed)
                        _RetryNotice(
                          key: const ValueKey<String>('pulse-status-retry'),
                          message: l10n.pulseStatusUnavailable,
                          action: l10n.pulseStatusRetry,
                          onRetry: controller.refreshZones,
                        ),
                      if (controller.recoveryError != null)
                        _PulseRecoveryNotice(controller: controller),
                      _PulseFormCard(
                        controller: controller,
                        bpmController: _bpmController,
                        timeController: _timeController,
                        noteController: _noteController,
                        saveFocusNode: _saveFocusNode,
                        isDemo: isDemo,
                      ),
                      const SizedBox(height: PulsysSpacing.s4),
                      _PulseFeedbackRegion(controller: controller),
                      const SizedBox(height: PulsysSpacing.s5),
                      _PulseHistorySection(controller: controller),
                    ],
                  ),
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}

class _PulseStatusCard extends StatelessWidget {
  const _PulseStatusCard({required this.snapshot, required this.isStale});

  final bool isStale;

  final ZoneSnapshot snapshot;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final presentation = PulseFormat.pulseStatus(l10n, snapshot.pulse);
    final toneColors = PulsysToneColors.forTone(presentation.tone);

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(PulsysSpacing.s4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              isStale ? l10n.pulseStatusLastAvailable : l10n.pulseStatusHeading,
              style: PulsysTypography.caption,
            ),
            const SizedBox(height: PulsysSpacing.s2),
            Container(
              padding: const EdgeInsets.symmetric(
                horizontal: PulsysSpacing.s2,
                vertical: PulsysSpacing.s1,
              ),
              decoration: BoxDecoration(
                color: toneColors.background,
                borderRadius: BorderRadius.circular(PulsysRadii.sm),
                border: Border.all(
                  color: toneColors.foreground,
                  width: PulsysBorders.control,
                ),
              ),
              child: Wrap(
                spacing: PulsysSpacing.s2,
                runSpacing: PulsysSpacing.s1,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  Icon(
                    presentation.icon,
                    size: PulsysSpacing.iconSize,
                    color: toneColors.foreground,
                  ),
                  Text(
                    presentation.label,
                    style: PulsysTypography.label.copyWith(
                      color: toneColors.foreground,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: PulsysSpacing.s2),
            Text(
              l10n.zonesSummaryLabel(snapshot.zones.length),
              style: PulsysTypography.caption,
            ),
          ],
        ),
      ),
    );
  }
}

class _PulseFormCard extends StatelessWidget {
  const _PulseFormCard({
    required this.controller,
    required this.bpmController,
    required this.timeController,
    required this.noteController,
    required this.saveFocusNode,
    required this.isDemo,
  });

  final PulseController controller;
  final TextEditingController bpmController;
  final TextEditingController timeController;
  final TextEditingController noteController;
  final FocusNode saveFocusNode;
  final bool isDemo;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final submitting = controller.isSubmitting;
    final buttonLabel = submitting
        ? l10n.pulseSavingAction
        : isDemo
        ? l10n.pulseSaveRealDataAction
        : l10n.pulseSaveAction;

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(PulsysSpacing.s4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            if (controller.replaces != null) ...<Widget>[
              Text(l10n.pulseCorrectionHeading, style: PulsysTypography.label),
              const SizedBox(height: PulsysSpacing.s3),
            ],
            TextField(
              key: const ValueKey<String>('pulse-bpm-field'),
              controller: bpmController,
              enabled: controller.canEdit,
              keyboardType: const TextInputType.numberWithOptions(
                decimal: true,
              ),
              textInputAction: TextInputAction.next,
              style: PulsysTypography.dataBody,
              onChanged: controller.updateBpm,
              decoration: InputDecoration(
                labelText: l10n.pulseBpmLabel,
                hintText: l10n.pulseBpmHint,
                suffixText: l10n.pulseBpmUnit,
                errorText: PulseFormat.bpmErrorText(l10n, controller.bpmError),
                errorMaxLines: 3,
              ),
            ),
            const SizedBox(height: PulsysSpacing.s4),
            TextField(
              key: const ValueKey<String>('pulse-time-field'),
              controller: timeController,
              enabled: controller.canEdit,
              textInputAction: TextInputAction.next,
              style: PulsysTypography.dataBody,
              onChanged: controller.updateMeasuredAt,
              decoration: InputDecoration(
                labelText: l10n.pulseTimeLabel,
                hintText: l10n.pulseTimeHint,
                helperText: l10n.pulseTimeHelper,
                helperMaxLines: 3,
                errorText: PulseFormat.timeErrorText(
                  l10n,
                  controller.measuredAtError,
                ),
                errorMaxLines: 3,
              ),
            ),
            const SizedBox(height: PulsysSpacing.s4),
            TextField(
              key: const ValueKey<String>('pulse-note-field'),
              controller: noteController,
              enabled: controller.canEdit,
              minLines: 1,
              maxLines: 3,
              style: PulsysTypography.body,
              onChanged: controller.updateNote,
              decoration: InputDecoration(
                labelText: l10n.pulseNoteLabel,
                hintText: l10n.pulseNoteHint,
                errorText: PulseFormat.noteErrorText(
                  l10n,
                  controller.noteError,
                ),
                errorMaxLines: 3,
              ),
            ),
            const SizedBox(height: PulsysSpacing.s5),
            FilledButton(
              key: const ValueKey<String>('pulse-save-button'),
              focusNode: saveFocusNode,
              onPressed: controller.canSubmit
                  ? () => unawaited(controller.submit())
                  : null,
              child: Text(buttonLabel, textAlign: TextAlign.center),
            ),
            if (controller.replaces != null ||
                controller.phase == PulseFormPhase.saved) ...<Widget>[
              const SizedBox(height: PulsysSpacing.s2),
              OutlinedButton(
                key: const ValueKey<String>('pulse-new-reading-button'),
                onPressed: controller.canStartNewReading
                    ? controller.startNewReading
                    : null,
                child: Text(l10n.pulseNewReadingAction),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _PulseFeedbackRegion extends StatelessWidget {
  const _PulseFeedbackRegion({required this.controller});

  final PulseController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final receipt = controller.lastReceipt;
    final generalError = controller.generalError;

    if (receipt == null && generalError == null) {
      return const SizedBox.shrink();
    }

    if (generalError != null) {
      final colors = PulsysToneColors.forTone(PulsysTone.alarm);
      return Semantics(
        liveRegion: true,
        child: Container(
          key: const ValueKey<String>('pulse-general-error'),
          padding: const EdgeInsets.all(PulsysSpacing.s4),
          decoration: BoxDecoration(
            color: colors.background,
            borderRadius: BorderRadius.circular(PulsysRadii.md),
            border: Border(
              left: BorderSide(
                color: colors.foreground,
                width: PulsysBorders.statusRule,
              ),
            ),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Icon(
                Icons.error_outline,
                size: PulsysSpacing.iconSize,
                color: colors.foreground,
              ),
              const SizedBox(width: PulsysSpacing.s3),
              Expanded(
                child: Text(
                  PulseFormat.submitErrorText(l10n, generalError),
                  style: PulsysTypography.body,
                ),
              ),
            ],
          ),
        ),
      );
    }

    final savedReceipt = receipt!;
    final colors = PulsysToneColors.forTone(PulsysTone.ok);
    final syncLabel = savedReceipt.outboxState == PulseOutboxState.pending
        ? l10n.pulseSyncPending
        : l10n.pulseSyncLocalOnly;

    return Semantics(
      liveRegion: true,
      child: Container(
        key: const ValueKey<String>('pulse-saved-banner'),
        padding: const EdgeInsets.all(PulsysSpacing.s4),
        decoration: BoxDecoration(
          color: colors.background,
          borderRadius: BorderRadius.circular(PulsysRadii.md),
          border: Border(
            left: BorderSide(
              color: colors.foreground,
              width: PulsysBorders.statusRule,
            ),
          ),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Icon(
              Icons.check_circle_outline,
              size: PulsysSpacing.iconSize,
              color: colors.foreground,
            ),
            const SizedBox(width: PulsysSpacing.s3),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(
                    l10n.pulseSavedLocallyHeading,
                    style: PulsysTypography.bodyStrong.copyWith(
                      color: colors.foreground,
                    ),
                  ),
                  const SizedBox(height: PulsysSpacing.s1),
                  Text(
                    l10n.pulseSavedReceipt(
                      savedReceipt.bpm.format(),
                      PulseFormat.utcTimestamp(savedReceipt.measuredAtUtc),
                    ),
                    style: PulsysTypography.dataBody,
                  ),
                  const SizedBox(height: PulsysSpacing.s1),
                  Text(syncLabel, style: PulsysTypography.caption),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _PulseHistorySection extends StatelessWidget {
  const _PulseHistorySection({required this.controller});

  final PulseController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final history = controller.history;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Semantics(
          header: true,
          child: Text(
            controller.historyLoadFailed && history.isNotEmpty
                ? l10n.pulseHistoryLastAvailable
                : l10n.pulseHistoryHeading,
            style: PulsysTypography.componentHeading,
          ),
        ),
        const SizedBox(height: PulsysSpacing.s3),
        if (controller.historyLoading && history.isEmpty)
          Text(l10n.pulseHistoryLoading, style: PulsysTypography.caption),
        if (controller.historyLoadFailed)
          _RetryNotice(
            key: const ValueKey<String>('pulse-history-retry'),
            message: l10n.pulseHistoryUnavailable,
            action: l10n.pulseHistoryRetry,
            onRetry: controller.refreshHistory,
          ),
        if (history.isEmpty &&
            !controller.historyLoadFailed &&
            !controller.historyLoading)
          Card(
            child: Padding(
              padding: const EdgeInsets.all(PulsysSpacing.s4),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  const Icon(
                    Icons.remove_circle_outline,
                    size: PulsysSpacing.iconSize,
                    color: PulsysColors.neutral,
                  ),
                  const SizedBox(width: PulsysSpacing.s3),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          l10n.pulseHistoryEmptyTitle,
                          style: PulsysTypography.bodyStrong,
                        ),
                        const SizedBox(height: PulsysSpacing.s1),
                        Text(
                          l10n.pulseHistoryEmptyBody,
                          style: PulsysTypography.caption,
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          )
        else
          for (final point in history) ...<Widget>[
            _PulseHistoryTile(
              point: point,
              onCorrect: controller.canEdit
                  ? () => controller.startCorrection(point)
                  : null,
            ),
            const SizedBox(height: PulsysSpacing.s2),
          ],
      ],
    );
  }
}

class _PulseHistoryTile extends StatelessWidget {
  const _PulseHistoryTile({required this.point, required this.onCorrect});

  final VoidCallback? onCorrect;

  final PulsePoint point;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(PulsysSpacing.s3),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Wrap(
              spacing: PulsysSpacing.s2,
              runSpacing: PulsysSpacing.s1,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                Text(
                  '${point.bpm.format()} ${l10n.pulseBpmUnit}',
                  style: PulsysTypography.dataBody,
                ),
                if (point.isDemo) _TagChip(label: l10n.pulseDemoTag),
                if (point.replaces != null)
                  _TagChip(label: l10n.pulseCorrectedLabel),
              ],
            ),
            const SizedBox(height: PulsysSpacing.s1),
            Text(
              PulseFormat.utcTimestamp(point.measuredAtUtc),
              style: PulsysTypography.dataCaption,
            ),
            const SizedBox(height: PulsysSpacing.s1),
            Text(
              PulseFormat.recordSource(l10n, point.source),
              style: PulsysTypography.caption,
            ),

            OutlinedButton(
              key: ValueKey<String>('pulse-correct-${point.recordId}'),
              onPressed: onCorrect,
              child: Text(l10n.pulseCorrectAction),
            ),
          ],
        ),
      ),
    );
  }
}

class _TagChip extends StatelessWidget {
  const _TagChip({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: PulsysSpacing.s2,
        vertical: PulsysSpacing.s1,
      ),
      decoration: BoxDecoration(
        color: PulsysColors.neutralBg,
        borderRadius: BorderRadius.circular(PulsysRadii.sm),
        border: Border.all(
          color: PulsysColors.neutral,
          width: PulsysBorders.control,
        ),
      ),
      child: Text(
        label,
        style: PulsysTypography.identifier.copyWith(
          color: PulsysColors.neutral,
        ),
      ),
    );
  }
}

class _PulseRecoveryNotice extends StatelessWidget {
  const _PulseRecoveryNotice({required this.controller});

  final PulseController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final error = controller.recoveryError!;
    return _RetryNotice(
      key: const ValueKey<String>('pulse-recovery-blocked'),
      message: error.code == ApiErrorCodes.unsupportedActionVersion
          ? l10n.pulseRecoveryNewerVersion
          : l10n.pulseRecoveryUnavailable,
      action: l10n.pulseRecoveryRetry,
      onRetry: controller.recoverPendingPulse,
    );
  }
}

class _RetryNotice extends StatelessWidget {
  const _RetryNotice({
    required this.message,
    required this.action,
    required this.onRetry,
    super.key,
  });

  final String message;
  final String action;
  final Future<void> Function() onRetry;

  @override
  Widget build(BuildContext context) => Semantics(
    liveRegion: true,
    child: Padding(
      padding: const EdgeInsets.symmetric(vertical: PulsysSpacing.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(message, style: PulsysTypography.caption),
          OutlinedButton(
            onPressed: () => unawaited(onRetry()),
            child: Text(action),
          ),
        ],
      ),
    ),
  );
}
