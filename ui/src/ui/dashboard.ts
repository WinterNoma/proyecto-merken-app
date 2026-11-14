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
export function setupPages() {
  const pages = [element("#overview"), element("#core-panel")];
  const dots = Array.from(document.querySelectorAll<HTMLButtonElement>("[data-page]"));
  let active = 0;
  function show(index: number, focus = false): void {
    active = Math.max(0, Math.min(pages.length - 1, index));
    pages.forEach((page, i) => { page.hidden = i !== active; });
    dots.forEach((dot, i) => { if (i === active) dot.setAttribute("aria-current", "page"); else dot.removeAttribute("aria-current"); });
    setText(element("#page-name"), active === 0 ? "Overview" : "History / On-device record");
    if (focus) pages[active].focus();
  }
  dots.forEach((dot, index) => dot.addEventListener("click", () => show(index)));
  element("#sync-open").addEventListener("click", () => show(1, true));
  element("#overview-alerts").addEventListener("click", () => show(1, true));
  document.querySelector(".wordmark")?.addEventListener("click", () => show(0, true));
  let start: { x: number; y: number; id: number } | null = null;
  const container = element("#screens");
  container.addEventListener("pointerdown", event => {
    if (event.pointerType !== "touch" || (event.target as Element).closest('button, input, select, textarea, summary, a, [role="button"]')) return;
    start = { x: event.clientX, y: event.clientY, id: event.pointerId };
  });
  container.addEventListener("pointerup", event => {
    if (!start || event.pointerId !== start.id) return;
    const dx = event.clientX - start.x; const dy = event.clientY - start.y; start = null;
    if (Math.abs(dx) > 60 && Math.abs(dx) > Math.abs(dy) * 1.5) show(active + (dx < 0 ? 1 : -1), true);
  });
  container.addEventListener("pointercancel", () => { start = null; });
}
export function createDashboard(available: boolean) {
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
  let feed: FeedState | null = null;
  let detailRevision = 0;
  const dialog = element<HTMLDialogElement>("#reading-dialog");
  const close = element("#reading-close");
  const controls = modal(dialog);
  close.addEventListener("click", controls.close);
  dialog.addEventListener("close", () => { detailRevision += 1; });
  function highlight(zone: BodyZone): void {
    for (const [key, link] of Object.entries(readingLinks)) element("#vital-" + key).classList.toggle("is-linked", zone === link.zone);
  }
  function overall(): void {
    const copy = available ? status ? statusCopy(status.overall.status) : statusCopy(confirmedAlert?.status ?? "not_evaluated")
      : member ? crewStatus(member.telemetry) : { tone: "neutral" as const, label: "No data", icon: "no_data" as const };
    paintBadge(element("#overall-status"), copy);
    element("#overall-box").dataset.tone = copy.tone;
    element("#overview-demo").hidden = !status?.is_demo;
    setText(element("#overall-scope"), available
      ? "On-device · " + (profile?.profile?.astronaut.display_name ?? "Profile unavailable") + (!online ? " · Local service unavailable" : "")
      : "MySQL · Reading-level assessment");
    if (available) setText(element("#feed-status"), status ? (online ? "Live · " : "Last update · ") + utc(status.evaluated_at, true) : "Awaiting local record");
    const extra = element("#overall-extra");
    const unknown = status?.indicators.filter(evaluation => evaluation.status === "not_evaluated" && evaluation.reason !== "not_applicable").length ?? 0;
    setText(extra, status ? [unknown ? unknown + " indicators not evaluated" : "", status.overall.worst.length ? "Review: " + status.overall.worst.map(key => key.replace(/_/g, " ")).join(", ") : ""].filter(Boolean).join(" · ") : "");
    extra.hidden = !extra.textContent;
    element("#overview-alerts").hidden = !status?.alerts.length && !confirmedAlert;
    setText(element("#overview-alerts"), "View persistent alerts (" + (status?.alerts.length || (confirmedAlert ? 1 : 0)) + ")");
    setText(element("#overview-sync"), status ? status.sync.pending + " pending · " + status.sync.failed + " failed" : "Unavailable");
    setText(element("#body-source"), available ? "Cards: MySQL crew · Zones: on-device symptoms" + (status?.is_demo ? " · DEMO" : "") : "Local service unavailable · Body zones not evaluated");
  }
  async function detail(key: string): Promise<void> {
    const link = readingLinks[key]; if (!link) return;
    map.select(link.zone); highlight(link.zone);
    const revision = ++detailRevision;
    const card = element("#vital-" + key);
    setText(element("#reading-title"), card.querySelector(".card-label")?.textContent ?? "Reading");
    const content = element("#reading-content");
    const feedSection = el("section", "detail-feed");
    feedSection.append(el("p", "eyebrow", "MySQL crew · " + (member?.name ?? "No crew available")),
      el("p", "record-value data", card.querySelector(".reading")?.textContent ?? "--"),
      el("p", "", card.querySelector(".vital-status")?.textContent ?? "No data"),
      el("p", "caption", card.querySelector(".reference")?.textContent ?? "No reference supplied"),
      el("p", "caption", feed?.lastSuccess ? "Received " + utc(feed.lastSuccess) + ". Measurement time is not supplied by this feed." : "No receipt or measurement time available."),
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
    const view = evaluationView(true);
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
      status = value; profile = identity; online = live; map.update(value?.body_zones ?? []); overall();
    },
  };
}
