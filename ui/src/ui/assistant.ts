import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  core,
  isCoreError,
  type InterpretResponse,
  type Layer2Status,
  type ProposedRecord,
  type SlmProgress,
} from "../core-api";
import { indicatorName } from "./core-format";
import { el, element, setText } from "./dom";

export type AssistSource = "text" | "voice";

export interface AssistantController {
  interpretInto: (text: string, source: AssistSource) => Promise<void>;
  reset: () => void;
}

export interface AssistantOptions {
  available: boolean;
  onUseProposal: (record: ProposedRecord, source: AssistSource) => void;
}

let activeController: AssistantController | null = null;

/** Formats on-device SLM progress events into user-facing English status copy. */
export function formatSlmProgress(progress: SlmProgress): string {
  switch (progress.phase) {
    case "loading":
      return "Loading the on-device model…";
    case "generating":
      return `Interpreting… ${progress.tokens}/${progress.max_tokens}`;
    case "validating":
      return "Checking the proposal…";
    case "done":
      return "";
  }
}

/** Subscribes to `slm-progress` for a single request ID and runs `task`, always unlistening afterwards. */
export async function runWithSlmProgress<T>(
  requestId: string,
  onProgress: (message: string) => void,
  task: () => Promise<T>,
): Promise<T> {
  let unlisten: UnlistenFn | null = null;
  try {
    unlisten = await listen<SlmProgress>("slm-progress", event => {
      if (event.payload.request_id !== requestId) return;
      onProgress(formatSlmProgress(event.payload));
    });
    return await task();
  } finally {
    unlisten?.();
  }
}

/** Interprets text into the mounted assistant dialog when available. */
export async function interpretInto(text: string, source: AssistSource): Promise<void> {
  if (!activeController) return;
  await activeController.interpretInto(text, source);
}

function layer2Message(status: Layer2Status): string | null {
  switch (status) {
    case "not_needed":
    case "used":
      return null;
    case "unavailable":
      return "AI interpreter not included in this build.";
    case "model_unavailable":
    case "timeout":
    case "slm_error":
    case "discarded":
    case "cancelled":
      return `AI interpreter unavailable (${status}); only the rules were used.`;
  }
}

function proposalCard(
  record: ProposedRecord,
  originLabel: string,
  onUse: () => void,
): HTMLElement {
  const card = el("article", "panel assist-card");
  const header = el("div", "assist-card-header");
  header.append(
    el("h4", "", indicatorName(record.indicator)),
    el("span", "caption assist-origin", originLabel),
  );
  card.append(header);

  if (record.fields.length > 0) {
    const list = el("ul", "assist-fields");
    for (const field of record.fields) {
      const valueText = field.value === null ? "—" : String(field.value);
      list.append(el("li", "", `${field.key}: ${valueText}`));
    }
    card.append(list);
  }

  card.append(el("p", "caption assist-quote", `From: “${record.quote}”`));
  if (record.time.kind === "ambiguous") {
    card.append(
      el(
        "p",
        "caption assist-time-hint",
        `Time mentioned: “${record.time.quote}” · set the time below`,
      ),
    );
  }

  const useButton = el("button", "button secondary", "Use this");
  useButton.type = "button";
  useButton.addEventListener("click", onUse);
  card.append(useButton);
  return card;
}

export function setupAssistant(options: AssistantOptions): AssistantController {
  const { available, onUseProposal } = options;
  const textarea = element<HTMLTextAreaElement>("#assist-text");
  const interpretButton = element<HTMLButtonElement>("#assist-interpret");
  const cancelButton = element<HTMLButtonElement>("#assist-cancel");
  const progress = element("#assist-progress");
  const results = element("#assist-results");

  let activeRequestId: string | null = null;
  interpretButton.disabled = !available;
  cancelButton.hidden = true;

  function renderResults(response: InterpretResponse, source: AssistSource): void {
    const nodes: HTMLElement[] = [];

    for (const record of response.proposal.records) {
      nodes.push(proposalCard(record, "Rules", () => onUseProposal(record, source)));
    }
    for (const record of response.layer2_proposal?.records ?? []) {
      nodes.push(
        proposalCard(record, "AI-generated suggestion · check carefully", () =>
          onUseProposal(record, source),
        ),
      );
    }

    if (nodes.length === 0) {
      nodes.push(el("p", "caption assist-meta", "No reading recognized. Use the form below."));
    }

    const notUnderstood = Array.from(
      new Set([...response.unresolved, ...response.proposal.uninterpreted].filter(Boolean)),
    );
    if (notUnderstood.length > 0) {
      nodes.push(el("p", "caption assist-meta", "Not understood: " + notUnderstood.join(" · ")));
    }

    if (response.proposal.negated.length > 0) {
      nodes.push(
        el("p", "caption assist-meta", "Negated: " + response.proposal.negated.join(" · ")),
      );
    }

    const statusLine = layer2Message(response.layer2);
    if (statusLine) {
      nodes.push(el("p", "caption assist-meta", statusLine));
    }

    results.replaceChildren(...nodes);
  }

  async function runInterpret(text: string, source: AssistSource): Promise<void> {
    if (textarea.value !== text) textarea.value = text;
    if (!available) {
      setText(progress, "Local service unavailable. Use the form below.");
      return;
    }
    if (!text.trim()) {
      results.replaceChildren();
      setText(progress, "Enter a description first.");
      textarea.focus();
      return;
    }
    if (activeRequestId !== null) return;

    const requestId = crypto.randomUUID();
    activeRequestId = requestId;
    interpretButton.disabled = true;
    interpretButton.setAttribute("aria-busy", "true");
    cancelButton.hidden = false;
    cancelButton.disabled = false;
    results.replaceChildren();
    setText(progress, "Interpreting…");

    try {
      const response = await runWithSlmProgress(
        requestId,
        message => {
          if (activeRequestId === requestId && message) setText(progress, message);
        },
        () => core.interpretText({ request_id: requestId, text, source }),
      );
      if (activeRequestId !== requestId) return;
      setText(progress, "");
      renderResults(response, source);
    } catch (error) {
      if (activeRequestId !== requestId) return;
      setText(
        progress,
        isCoreError(error) ? error.message : "Local service unavailable. Use the form below.",
      );
    } finally {
      if (activeRequestId === requestId) {
        activeRequestId = null;
        interpretButton.disabled = !available;
        interpretButton.setAttribute("aria-busy", "false");
        cancelButton.hidden = true;
        cancelButton.disabled = false;
      }
    }
  }

  function reset(): void {
    if (activeRequestId !== null) {
      const pendingId = activeRequestId;
      activeRequestId = null;
      void core.slmCancel(pendingId).catch(() => {});
    }
    interpretButton.disabled = !available;
    interpretButton.setAttribute("aria-busy", "false");
    cancelButton.hidden = true;
    cancelButton.disabled = false;
    setText(progress, "");
    results.replaceChildren();
  }

  interpretButton.addEventListener("click", () => {
    void runInterpret(textarea.value, "text");
  });

  cancelButton.addEventListener("click", () => {
    if (!activeRequestId) return;
    cancelButton.disabled = true;
    setText(progress, "Cancelling…");
    void core.slmCancel(activeRequestId).catch(() => {});
  });

  const controller: AssistantController = {
    interpretInto: runInterpret,
    reset,
  };
  activeController = controller;
  return controller;
}
