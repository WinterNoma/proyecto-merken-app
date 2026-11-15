import { core, isCoreError, type DemoScenario, type Evaluation, type ExplainResponse, type Indicator, type NoteStatus, type ProfileResponse, type RecordItem, type StatusResponse, type RaiseEventResult, type EventItem, type LogResult } from "../core-api";
import { element, el, setText, timestamp, utc } from "./dom";
import { evaluationCopy, statusCopy, escalationCopy, paintBadge, badge } from "./status-copy";
import { indicatorName, recordValue } from "./core-format";
import { zoneName } from "./body-map";
import { sparkline } from "./sparkline";
import { runWithSlmProgress } from "./assistant";

const chartIndicators: Indicator[] = ["heart_rate", "body_temperature", "oxygen_in_blood", "glucose", "sleep"];
interface EvaluationView {
  node: HTMLElement;
  update: (evaluation: Evaluation, demo: boolean) => void;
  history: (records: RecordItem[] | null, stale: boolean) => void;
}
function noteUnavailableMessage(status: NoteStatus): string | null {
  switch (status) {
    case "available":
    case "no_fichas":
      return null;
    case "model_unavailable":
    case "timeout":
    case "slm_error":
    case "discarded":
    case "cancelled":
      return `AI note unavailable (${status}). The facts above come from the local core.`;
  }
}
export function evaluationView(withHistory: boolean, available = false): EvaluationView {
  const node = el("article", "evaluation-row panel");
  const heading = el("div", "evaluation-heading");
  const name = el("h4");
  const status = badge(statusCopy("not_evaluated"));
  heading.append(name, status);
  const value = el("p", "record-value data");
  const when = el("p", "caption");
  const summary = el("p", "evaluation-summary");
  const source = el("p", "caption");
  const qualification = el("p", "caption");
  const escalation = el("p", "caption");
  const actions = el("div", "evaluation-actions");
  const explainButton = el("button", "button secondary", "Explain");
  explainButton.type = "button";
  explainButton.disabled = !available;
  const explainProgress = el("p", "caption explain-progress");
  explainProgress.setAttribute("role", "status");
  actions.append(explainButton, explainProgress);
  const explainBox = el("div", "notice explain-notice");
  explainBox.dataset.tone = "neutral";
  explainBox.hidden = true;
  const history = el("details", "history-disclosure");
  const historyBody = el("div");
  history.append(el("summary", "", "View recorded history"), historyBody);
  node.append(heading, value, when, summary, source, qualification, escalation, actions, explainBox);
  if (withHistory) node.append(history);
  let historySignature = "";
  let currentEvaluation: Evaluation | null = null;
  let explaining = false;
  function renderExplanation(response: ExplainResponse): void {
    const nodes: HTMLElement[] = [];
    if (response.facts.length > 0) {
      const factsList = el("ul", "explain-facts");
      for (const fact of response.facts) factsList.append(el("li", "", fact.text));
      nodes.push(factsList);
    }
    if (response.note) {
      nodes.push(el("p", "explain-note-heading", response.note.label));
      if (response.note.statements.length > 0) {
        const statementsList = el("ul", "explain-note-list");
        for (const statement of response.note.statements) statementsList.append(el("li", "", statement.text));
        nodes.push(statementsList);
      }
    }
    const unavailable = noteUnavailableMessage(response.note_status);
    if (unavailable) nodes.push(el("p", "caption", unavailable));
    explainBox.replaceChildren(...nodes);
    explainBox.hidden = nodes.length === 0;
  }
  explainButton.addEventListener("click", async () => {
    if (!available || explaining || !currentEvaluation) return;
    const evaluation = currentEvaluation;
    const requestId = crypto.randomUUID();
    explaining = true;
    explainButton.disabled = true;
    explainButton.setAttribute("aria-busy", "true");
    setText(explainProgress, "Loading explanation…");
    try {
      const response = await runWithSlmProgress(
        requestId,
        message => { if (message) setText(explainProgress, message); },
        () => core.explainIndicator({
          request_id: requestId,
          indicator: evaluation.indicator as Indicator,
          body_zone: evaluation.body_zone,
        }),
      );
      setText(explainProgress, "");
      renderExplanation(response);
    } catch (error) {
      setText(explainProgress, isCoreError(error) ? error.message : "Local service unavailable.");
    } finally {
      explaining = false;
      explainButton.disabled = !available;
      explainButton.setAttribute("aria-busy", "false");
    }
  });
  return {
    node,
    update(evaluation, demo): void {
      currentEvaluation = evaluation;
      const copy = evaluationCopy(evaluation);
      node.dataset.tone = copy.tone;
      node.classList.toggle("context-row", evaluation.reason === "not_applicable");
      status.classList.toggle("context-label", evaluation.reason === "not_applicable");
      setText(name, indicatorName(evaluation.indicator) + (evaluation.body_zone ? " · " + zoneName(evaluation.body_zone) : ""));
      paintBadge(status, copy);
      setText(value, (evaluation.reason === "stale" ? "Last recorded: " : "") + recordValue(evaluation.latest_value, evaluation.indicator));
      when.replaceChildren(document.createTextNode("Measured "), timestamp(evaluation.latest_measured_at));
      setText(summary, evaluation.summary);
      const sources = { published: "Published", approved_protocol: "Approved protocol", demo_heuristic: "Demo heuristic" };
      setText(source, "Source: " + sources[evaluation.source_kind] + (evaluation.source_ref ? " · " + evaluation.source_ref : ""));
      setText(qualification, [demo ? "DEMO · Simulated records." : "", evaluation.source_kind === "demo_heuristic" ? "Demo rule, not medical advice" : ""].filter(Boolean).join(" "));
      qualification.hidden = !qualification.textContent;
      setText(escalation, escalationCopy(evaluation.escalation));
      escalation.hidden = evaluation.escalation === "continue";
      node.dataset.indicator = evaluation.indicator;
    },
    history(records, stale): void {
      const signature = JSON.stringify([records, stale]);
      if (signature === historySignature) return;
      historySignature = signature;
      const sampleDetailsOpen = historyBody.querySelector("details")?.open ?? false;
      historyBody.replaceChildren(records === null ? el("p", "caption", "History unavailable. Try refreshing the local service.")
        : sparkline(records, node.dataset.indicator as Indicator, stale));
      const sampleDetails = historyBody.querySelector("details");
      if (sampleDetails) sampleDetails.open = sampleDetailsOpen;
    },
  };
}
export interface CorePanel {
  refresh: (force?: boolean) => Promise<StatusResponse | null>;
  snapshot: () => StatusResponse | null;
  profile: () => ProfileResponse | null;
  acceptHelp: (result: RaiseEventResult) => void;
  acceptRecord: (result: Extract<LogResult, { status: "saved" }>) => void;
  stop: () => void;
}
export function createCorePanel(available: boolean, onChange: (status: StatusResponse | null, profile: ProfileResponse | null, online: boolean, confirmedAlert?: Evaluation) => void): CorePanel {
  let current: StatusResponse | null = null;
  let confirmedAlert: Evaluation | undefined;
  let summaryPending = false;
  let persistentAlerts: Evaluation[] = [];
  let profile: ProfileResponse | null = null;
  let pending: Promise<StatusResponse | null> | null = null;
  let stopped = false;
  let revision = 0;
  let actionBusy = false;
  let demoBlocked = false;
  const record = element("#core-record");
  const availability = element("#core-availability");
  const retry = element<HTMLButtonElement>("#core-retry");
  const demoButton = element<HTMLButtonElement>("#load-demo");
  const demoScenario = element<HTMLSelectElement>("#demo-scenario");
  const indicatorViews = new Map<string, EvaluationView>();
  const symptomViews = new Map<string, EvaluationView>();
  const alertViews = new Map<string, HTMLElement>();
  const alertSignatures = new Map<string, string>();
  const announced = new Set<string>();
  const knownEvents = new Map<string, EventItem>();

  function renderAlerts(alerts: Evaluation[], demo: boolean): void {
    persistentAlerts = alerts;
    const liveKeys = new Set<string>();
    const newNames: string[] = [];
    for (const evaluation of alerts) {
      const key = JSON.stringify([evaluation.indicator, evaluation.rule_id, evaluation.input_event_ids]);
      liveKeys.add(key);
      let notice = alertViews.get(key);
      if (!notice) {
        notice = el("article", "notice alarm-notice");
        alertViews.set(key, notice);
        element("#core-alerts").append(notice);
      }
      const events = evaluation.input_event_ids.map(id => knownEvents.get(id)).filter((event): event is EventItem => event !== undefined);
      const signature = JSON.stringify({ ...evaluation, evaluated_at: undefined, demo, queue: current?.sync, events });
      if (alertSignatures.get(key) === signature) continue;
      alertSignatures.set(key, signature);
      const existingDetails = notice.querySelector("details");
      const copy = evaluationCopy(evaluation);
      notice.dataset.tone = copy.tone;
      const title = el("h3", "notice-title");
      title.append(badge(copy), document.createTextNode(" " + indicatorName(evaluation.indicator)));
      const details = existingDetails ?? el("details");
      if (!existingDetails) details.append(el("summary", "", "View event details"));
      const information = el("div");
      information.append(
        el("p", "caption", "Event IDs: " + (evaluation.input_event_ids.join(", ") || "No event identifier")),
        el("p", "caption", "Evaluated " + utc(evaluation.evaluated_at)),
        el("p", "caption", "Source: " + evaluation.source_kind.replace(/_/g, " ") + (evaluation.source_ref ? " · " + evaluation.source_ref : "")),
        el("p", "caption", "Rule: " + evaluation.rule_id + " · " + evaluation.rule_version),
      );
      for (const event of events) {
        information.append(el("p", "caption", "Occurred " + utc(event.occurred_at)));
        if (event.message) information.append(el("p", "", event.message));
        const sync = { pending: "Synchronization pending…", synced: "Marked as synchronized. Ground review is not confirmed.", failed: "Not synchronized. Saved locally.", local_only: "Saved on this device only." };
        information.append(el("p", "caption", sync[event.sync]));
      }
      details.querySelector("div")?.remove();
      details.append(information);
      notice.replaceChildren(title, el("p", "", evaluation.summary),
        el("p", "caption", escalationCopy(evaluation.escalation) + " · Persistent event"),
        el("p", "caption", demo ? "DEMO · Simulated event. Not uploaded to ground." : "Saved locally. Ground receipt is not confirmed."),
        el("p", "caption", current ? "Device queue: " + current.sync.pending + " pending · " + current.sync.failed + " not synced" : "Refresh the local record for synchronization status."),
        details);
      if (evaluation.source_kind === "demo_heuristic") notice.append(el("p", "caption", "Demo rule, not medical advice"));
      if (!announced.has(key)) { announced.add(key); newNames.push(indicatorName(evaluation.indicator)); }
    }
    // Remove only on a successful authoritative status response (e.g. demo
    // replacement). Refresh failures never call this function or hide alarms.
    for (const [key, notice] of alertViews) if (!liveKeys.has(key)) { notice.remove(); alertViews.delete(key); alertSignatures.delete(key); }
    for (const key of announced) if (!liveKeys.has(key)) announced.delete(key);
    const referenced = new Set(alerts.flatMap(alert => alert.input_event_ids));
    for (const id of knownEvents.keys()) if (!referenced.has(id)) knownEvents.delete(id);
    if (newNames.length) setText(element("#alarm-announcement"), "Alarm. " + newNames.join(". ") + ". Review the on-device record.");
  }
  function renderEvaluations(evaluations: Evaluation[], views: Map<string, EvaluationView>, container: HTMLElement, demo: boolean): void {
    const keys = new Set<string>();
    for (const evaluation of evaluations) {
      const key = evaluation.indicator + ":" + (evaluation.body_zone ?? "");
      keys.add(key);
      let view = views.get(key);
      if (!view) { view = evaluationView(chartIndicators.includes(evaluation.indicator as Indicator), available); views.set(key, view); container.append(view.node); }
      view.update(evaluation, demo);
    }
    for (const [key, view] of views) if (!keys.has(key)) { view.node.remove(); views.delete(key); }
  }
  function render(status: StatusResponse): void {
    current = status;
    confirmedAlert = undefined;
    summaryPending = false;
    record.hidden = false;
    availability.hidden = true;
    element("#demo-badge").hidden = !status.is_demo;
    element("#demo-copy").hidden = !status.is_demo;
    setText(element("#core-profile"), profile?.profile?.astronaut.display_name ?? (profile ? "No profile available" : "Profile unavailable"));
    setText(element("#core-mission"), profile?.profile?.astronaut.mission?.name ?? "No mission profile");
    paintBadge(element("#core-overall"), statusCopy(status.overall.status));
    setText(element("#core-escalation"), escalationCopy(status.overall.escalation));
    setText(element("#core-worst"), status.overall.worst.length ? status.overall.worst.map(indicatorName).join(", ") : "None reported");
    setText(element("#core-met"), status.met ?? "--");
    setText(element("#core-sync"), status.sync.pending + " pending · " + status.sync.failed + " not synced");
    setText(element("#core-evaluated"), "Evaluated " + utc(status.evaluated_at));
    renderAlerts(status.alerts, status.is_demo);
    onChange(status, profile, true);
    renderEvaluations(status.indicators, indicatorViews, element("#core-indicators"), status.is_demo);
    renderEvaluations(status.symptoms, symptomViews, element("#core-symptoms"), status.is_demo);
  }
  async function read(): Promise<StatusResponse | null> {
    const startedAtRevision = revision;
    retry.disabled = true;
    try {
      const [status, profileResult, recordResult, eventResult] = await Promise.allSettled([
        core.getStatus(), core.getProfile(), core.listRecords({ limit: 1 }), core.listEvents({ limit: 1 }),
      ]);
      if (stopped || startedAtRevision !== revision) return null;
      if (status.status === "rejected") throw status.reason;
      profile = profileResult.status === "fulfilled" ? profileResult.value : null;
      if (eventResult.status === "fulfilled") for (const event of eventResult.value.events) knownEvents.set(event.event_id, event);
      render(status.value);
      const empty = profileResult.status === "fulfilled" && profileResult.value.profile === null
        && recordResult.status === "fulfilled" && recordResult.value.records.length === 0
        && eventResult.status === "fulfilled" && eventResult.value.events.length === 0
        && !status.value.is_demo && !status.value.alerts.length && !status.value.symptoms.length
        && status.value.indicators.every(evaluation => evaluation.latest_value === null);
      // Also shown over DEMO data so another scenario can replace it.
      showDemoChooser(!(empty || status.value.is_demo) || demoBlocked, status.value.is_demo);
      const history = await Promise.allSettled(chartIndicators.map(indicator => core.listRecords({ indicator, limit: 40 })));
      if (stopped || startedAtRevision !== revision) return status.value;
      chartIndicators.forEach((indicator, index) => {
        const result = history[index];
        const evaluation = status.value.indicators.find(entry => entry.indicator === indicator);
        indicatorViews.get(indicator + ":")?.history(result?.status === "fulfilled" ? result.value.records : null, evaluation?.reason === "stale");
      });
      return status.value;
    } catch {
      if (stopped || startedAtRevision !== revision) return null;
      availability.hidden = false;
      setText(element("#core-availability-title"), "Local service unavailable");
      setText(element("#core-availability-copy"), summaryPending
        ? "The latest action was saved locally. The summary could not be refreshed; its confirmed evaluation is shown."
        : current
        ? "The local service could not be reached. Showing the last confirmed record from " + utc(current.evaluated_at) + "."
        : "The local service could not be reached. This action was not confirmed.");
      element("#demo-empty").hidden = true;
      retry.hidden = false;
      onChange(current, profile, false, confirmedAlert);
      return null;
    } finally {
      retry.disabled = false;
    }
  }
  function invalidateSummary(evaluation: Evaluation | null): void {
    // Only a newer authoritative status may replace a confirmed pending alarm.
    if (evaluation?.status === "alarm") confirmedAlert = evaluation;
    current = null;
    summaryPending = true;
    paintBadge(element("#core-overall"), statusCopy(confirmedAlert?.status ?? "not_evaluated"));
    setText(element("#core-escalation"), confirmedAlert ? escalationCopy(confirmedAlert.escalation) : "Summary update pending");
    setText(element("#core-worst"), confirmedAlert ? indicatorName(confirmedAlert.indicator) : "Not evaluated");
    setText(element("#core-sync"), "Update pending");
    onChange(null, profile, false, confirmedAlert);
  }
  async function refresh(force = false): Promise<StatusResponse | null> {
    if (!available || stopped) return null;
    if (pending) {
      const previous = await pending;
      if (!force) return previous;
    }
    // Recheck after awaiting: another caller may already have started a refresh.
    if (pending) return pending;
    const request = read();
    pending = request;
    try { return await request; } finally { if (pending === request) pending = null; }
  }
  function demoButtonText(): string { return current?.is_demo ? "Load scenario" : "Load demo data"; }
  function showDemoChooser(hidden: boolean, demo: boolean): void {
    element("#demo-empty").hidden = hidden;
    setText(element("#demo-empty-title"), demo ? "DEMO scenario" : "No on-device records");
    setText(element("#demo-empty-copy"), demo
      ? "Load another simulated scenario. It replaces the current DEMO data."
      : "Explore the app with clearly marked simulated records.");
    if (!actionBusy) setText(demoButton, demoButtonText());
  }
  retry.addEventListener("click", () => { void refresh(true); });
  demoButton.disabled = !available;
  demoButton.addEventListener("click", async () => {
    if (!available || stopped || actionBusy) return;
    actionBusy = true; demoButton.disabled = true; demoScenario.disabled = true;
    demoButton.setAttribute("aria-busy", "true"); setText(demoButton, "Loading demo data…");
    setText(element("#core-action-result"), "");
    try {
      // Wait out any older poll before mutating the store.
      if (pending) await pending;
      const scenario = demoScenario.value as DemoScenario;
      const status = await core.loadDemo(scenario);
      revision += 1;
      render(status);
      const name = demoScenario.selectedOptions[0]?.textContent?.split(" · ")[0] ?? "Demo";
      setText(element("#core-action-result"), name + " demo data loaded. Simulated records are not uploaded to ground.");
    } catch (error) {
      if (isCoreError(error) && error.code === "real_data_present") {
        demoBlocked = true;
        element("#demo-empty").hidden = true;
        setText(element("#core-action-result"), "Demo data was not loaded. Real data is present on this device.");
      } else setText(element("#core-action-result"), "Demo data could not be loaded. Try again.");
    } finally {
      await refresh(true);
      actionBusy = false; demoButton.disabled = false; demoScenario.disabled = false;
      demoButton.setAttribute("aria-busy", "false"); setText(demoButton, demoButtonText());
    }
  });
  if (available) { void refresh(); }
  else {
    setText(element("#core-availability-title"), "Local service unavailable");
    setText(element("#core-availability-copy"), "Open the Merken app to use on-device records.");
    onChange(null, null, false);
  }
  const timer = available ? window.setInterval(() => { if (!actionBusy) void refresh(); }, 10000) : null;
  return {
    refresh, snapshot: () => current, profile: () => profile,
    acceptRecord(result): void {
      revision += 1;
      if (current?.is_demo && !result.record.is_demo) {
        current = null; profile = null; record.hidden = true;
        element("#demo-badge").hidden = true;
        indicatorViews.forEach(view => view.node.remove()); indicatorViews.clear();
        symptomViews.forEach(view => view.node.remove()); symptomViews.clear();
        alertViews.forEach(view => view.remove()); alertViews.clear(); alertSignatures.clear(); persistentAlerts = []; confirmedAlert = undefined;
        announced.clear(); knownEvents.clear();
        setText(element("#core-profile"), "Refreshing profile…");
        paintBadge(element("#core-overall"), statusCopy("not_evaluated"));
      }
      // Show the evaluation the core confirmed with this save right away, so a
      // failed refresh never leaves an older status on screen. The aggregate
      // summary is not recomputed here: it waits for the next getStatus().
      const evaluation = result.evaluation;
      const symptom = evaluation.indicator === "symptom";
      const views = symptom ? symptomViews : indicatorViews;
      const key = evaluation.indicator + ":" + (evaluation.body_zone ?? "");
      let view = views.get(key);
      if (!view) {
        view = evaluationView(chartIndicators.includes(evaluation.indicator as Indicator), available);
        views.set(key, view);
        element(symptom ? "#core-symptoms" : "#core-indicators").append(view.node);
      }
      view.update(evaluation, result.record.is_demo);
      record.hidden = false;
      setText(element("#core-evaluated"), "Latest saved evaluation shown. Summary update pending.");
      if (evaluation.status === "alarm") setText(element("#alarm-announcement"), "Alarm. " + indicatorName(evaluation.indicator) + ". Review the on-device record.");
      invalidateSummary(evaluation);
    },
    acceptHelp(result): void {
      revision += 1;
      knownEvents.set(result.event.event_id, result.event);
      if (current?.is_demo && !result.event.is_demo) {
        // A real event removes demo records. Do not keep showing that old profile.
        current = null; profile = null; record.hidden = true; element("#demo-badge").hidden = true;
        indicatorViews.forEach(view => view.node.remove()); indicatorViews.clear();
        symptomViews.forEach(view => view.node.remove()); symptomViews.clear();
        alertViews.forEach(view => view.remove()); alertViews.clear(); alertSignatures.clear(); persistentAlerts = []; confirmedAlert = undefined;
        if (result.evaluation) renderAlerts([result.evaluation], result.event.is_demo);
      } else if (result.evaluation) {
        const alerts = [...persistentAlerts];
        if (!alerts.some(alert => alert.input_event_ids.includes(result.event.event_id))) alerts.push(result.evaluation);
        renderAlerts(alerts, result.event.is_demo);
      }
      invalidateSummary(result.evaluation);
    },
    stop(): void { stopped = true; if (timer !== null) window.clearInterval(timer); },
  };
}
