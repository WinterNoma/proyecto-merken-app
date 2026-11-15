import { core, type BodyZone, type Evaluation, type Indicator, type ProfileResponse, type StatusResponse } from "../core-api";
import type { AstronautProfile } from "../types";
import type { FeedState } from "./telemetry";
import { element, el, setText, utc } from "./dom";
import { createBodyMap, zoneName } from "./body-map";
import { modal } from "./dialog";
import { crewStatus, paintBadge, statusCopy } from "./status-copy";
import { evaluationView } from "./core-panel";

// Navigation associations only. Never used to derive a symptom or zone status.
export const readingLinks: Record<string, { indicator: Indicator; zone: BodyZone }> = {
  heart: { indicator: "heart_rate", zone: "chest" }, pressure: { indicator: "blood_pressure", zone: "left_arm" },
  oxygen: { indicator: "oxygen_in_blood", zone: "chest" }, mood: { indicator: "mood", zone: "head" },
  temperature: { indicator: "body_temperature", zone: "head" }, glucose: { indicator: "glucose", zone: "abdomen" },
  sleep: { indicator: "sleep", zone: "head" }, nutrition: { indicator: "nutrition", zone: "abdomen" },
  radiation: { indicator: "space_radiation", zone: "other" }, distance: { indicator: "distance_from_earth", zone: "other" },
};
export function createDashboard(available: boolean, onZoneSelection?: (zone: BodyZone) => void) {
  const map = createBodyMap(zone => {
    highlight(zone);
    const linked = Object.entries(readingLinks).filter(([, link]) => link.zone === zone);
    setText(element("#zone-selection"), zoneName(zone) + (linked.length ? " · " + linked.length + " linked readings" : " · No linked card"));
  });
  let member: AstronautProfile | null = null;
  let status: StatusResponse | null = null;
  let profile: ProfileResponse | null = null;
  let online = false;
  let confirmedAlert: Evaluation | null = null;
  let persistentAlerts: Evaluation[] = [];
  let feed: FeedState | null = null;
  let detailRevision = 0;
  const dialog = element<HTMLDialogElement>("#reading-dialog");
  const close = element("#reading-close");
  const controls = modal(dialog);
  close.addEventListener("click", controls.close);
  dialog.addEventListener("close", () => { detailRevision += 1; });
  function highlight(zone: BodyZone): void {
    for (const [key, link] of Object.entries(readingLinks)) element("#vital-" + key).classList.toggle("is-linked", zone === link.zone);
    onZoneSelection?.(zone);
  }
  function overall(): void {
    const copy = available ? confirmedAlert ? statusCopy(confirmedAlert.status) : status ? statusCopy(status.overall.status) : statusCopy("not_evaluated")
      : member ? crewStatus(member.telemetry) : { tone: "neutral" as const, label: "No data", icon: "no_data" as const };
    paintBadge(element("#overall-status"), copy);
    element("#overall-box").dataset.tone = copy.tone;
    element("#overview-demo").hidden = !status?.is_demo && !feed?.preview;
    setText(element("#overall-scope"), available
      ? "On-device · " + (profile?.profile?.astronaut.display_name ?? "Profile unavailable") + (!online ? " · Local service unavailable" : "")
      : feed?.preview ? "Synthetic crew · Reading-level assessment" : "MySQL · Reading-level assessment");
    if (available) setText(element("#feed-status"), status ? (online ? "Live · " : "Last update · ") + utc(status.evaluated_at, true) : confirmedAlert ? "Summary update pending" : "Awaiting local record");
    const extra = element("#overall-extra");
    const unknown = status?.indicators.filter(evaluation => evaluation.status === "not_evaluated" && evaluation.reason !== "not_applicable").length ?? 0;
    setText(extra, status ? [unknown ? unknown + " indicators not evaluated" : "", status.overall.worst.length ? "Review: " + status.overall.worst.map(key => key.replace(/_/g, " ")).join(", ") : ""].filter(Boolean).join(" · ") : "");
    extra.hidden = !extra.textContent;
    element("#overview-alerts").hidden = persistentAlerts.length === 0;
    setText(element("#overview-alerts"), "View persistent alerts (" + persistentAlerts.length + ")");
    setText(element("#overview-sync"), status ? status.sync.pending + " pending · " + status.sync.failed + " failed" : "Unavailable");
    setText(element("#body-source"), available ? "Cards: MySQL crew · Zones: on-device symptoms" + (status?.is_demo ? " · DEMO" : "") : feed?.preview ? "DEMO readings · Body navigation only · Zones not evaluated" : "Local service unavailable · Body zones not evaluated");
  }
  async function detail(key: string): Promise<void> {
    const link = readingLinks[key]; if (!link) return;
    map.select(link.zone); highlight(link.zone);
    const revision = ++detailRevision;
    const card = element("#vital-" + key);
    setText(element("#reading-title"), card.querySelector(".card-label")?.textContent ?? "Reading");
    const content = element("#reading-content");
    const feedSection = el("section", "detail-feed");
    feedSection.append(el("p", "eyebrow", (feed?.preview ? "DEMO crew · " : "MySQL crew · ") + (member?.name ?? "No crew available")),
      el("p", "record-value data", card.querySelector(".reading")?.textContent ?? "--"),
      el("p", "", card.querySelector<HTMLElement>(".vital-status")?.dataset.label ?? "No data"),
      el("p", "caption", card.querySelector(".reference")?.textContent ?? "No reference supplied"),
      el("p", "caption", feed?.preview ? "Fixed synthetic sample. Not a live measurement." : feed?.lastSuccess ? "Received " + utc(feed.lastSuccess) + ". Measurement time is not supplied by this feed." : "No receipt or measurement time available."),
      el("p", "caption", "Navigation zone: " + zoneName(link.zone) + ". This link does not indicate a symptom."));
    if (feed && !feed.live) feedSection.append(el("p", "caption", "Connection unavailable. Last received values only."));
    const local = el("section", "detail-local");
    local.append(el("h3", "", "On-device evaluation"), el("p", "caption", available ? "Loading local record…" : "Local service unavailable. Open the Merken app to view recorded history."));
    content.replaceChildren(feedSection, local); controls.open(close);
    if (!available) return;
    const results = await Promise.allSettled([core.getStatus(), core.getProfile(), core.listRecords({ indicator: link.indicator, limit: 40 })]);
    if (revision !== detailRevision || !dialog.open) return;
    const [stateResult, profileResult, records] = results;
    local.replaceChildren(el("h3", "", "On-device evaluation"));
    local.append(el("p", "caption", "Separate profile · " + (profileResult.status === "fulfilled" ? profileResult.value.profile?.astronaut.display_name ?? "No profile available" : "Profile unavailable")));
    if (stateResult.status !== "fulfilled") { local.append(el("p", "", "Local service unavailable. Evaluation and history could not be confirmed.")); return; }
    const evaluation = stateResult.value.indicators.find(entry => entry.indicator === link.indicator);
    if (!evaluation) { local.append(el("p", "caption", "No on-device evaluation or reading available.")); return; }
    const view = evaluationView(true, available);
    view.update(evaluation, stateResult.value.is_demo);
    view.history(records.status === "fulfilled" ? records.value.records : null, evaluation.reason === "stale");
    const history = view.node.querySelector("details"); if (history) history.open = true;
    local.append(view.node);
  }
  for (const key of Object.keys(readingLinks)) element("#vital-" + key).addEventListener("click", () => { void detail(key); });
  return {
    member(value: AstronautProfile | null): void { member = value; overall(); },
    feed(value: FeedState): void { feed = value; overall(); },
    core(value: StatusResponse | null, identity: ProfileResponse | null, live: boolean, alert?: Evaluation): void {
      if (live) confirmedAlert = null;
      if (alert) confirmedAlert = alert;
      if (value) persistentAlerts = value.alerts;
      else if (identity === null && (!alert || status?.is_demo)) persistentAlerts = [];
      if (alert?.input_event_ids.length && !persistentAlerts.some(entry => entry.input_event_ids.some(id => alert.input_event_ids.includes(id)))) {
        persistentAlerts = [...persistentAlerts, alert];
      }
      status = value; profile = identity; online = live; map.update(value?.body_zones ?? []); overall();
    },
  };
}
