import { Channel, invoke } from "@tauri-apps/api/core";
import { dictate, isCoreError, recordDictation, type DictationRecording } from "../core-api";
import { element } from "./dom";

type SpeechEvent = { state: "listening" | "processing" } | { state: "partial"; text: string };
type Phase = "preparing" | "listening" | "processing";
type Capture = { phase: Phase; previous: string } & (
  | { mode: "live"; id: string }
  | { mode: "recorded"; recording: DictationRecording }
);

export function setupDictation(available: boolean, onReview: (text: string) => void, onListening?: (active: boolean) => void) {
  const panel = element("#voice-capture");
  const mic = element<HTMLButtonElement>("#voice-microphone");
  const label = element("#voice-microphone-label");
  const status = element("#voice-status");
  const transcript = element<HTMLTextAreaElement>("#voice-transcript");
  const review = element<HTMLButtonElement>("#voice-review");
  const fallback = element<HTMLButtonElement>("#voice-recorded");
  let mode: Capture["mode"] = /Android/i.test(navigator.userAgent) ? "live" : "recorded";
  let capture: Capture | null = null;
  let sequence = 0;

  function render(message: string): void {
    const phase = capture?.phase;
    mic.setAttribute("aria-pressed", String(phase === "listening"));
    mic.disabled = !available || phase === "preparing" || phase === "processing";
    label.textContent = phase === "preparing" ? "Opening microphone…"
      : phase === "listening" ? "Stop dictation"
      : phase === "processing" ? "Transcribing…"
      : "Start dictation";
    status.textContent = message;
    transcript.readOnly = capture !== null || !transcript.value;
    review.disabled = !available || capture !== null || !transcript.value.trim();
    onListening?.(phase === "listening");
  }

  function phase(session: Capture, phase: Phase, message: string): void {
    session.phase = phase;
    render(message);
  }

  function complete(session: Capture, text: string, message: string): void {
    if (capture !== session) return;
    transcript.value = text.slice(0, transcript.maxLength);
    capture = null;
    render(message);
  }

  function stopLive(session: Extract<Capture, { mode: "live" }>, discard: boolean): Promise<void> {
    return invoke("plugin:local-speech|finish", { session: session.id, discard });
  }

  function cancel(): void {
    const session = capture;
    if (!session) return;
    complete(session, session.previous, "Dictation canceled.");
    if (session.mode === "live") void stopLive(session, true).catch(() => {});
    else session.recording.cancel();
  }

  function finish(session: Capture): void {
    if (capture !== session || session.phase !== "listening") return;
    phase(session, "processing", "Transcribing on this device…");
    if (session.mode === "live") {
      void stopLive(session, false).catch(() => {
        if (capture !== session) return;
        cancel();
        render("The microphone could not finish. Try again.");
      });
      return;
    }
    void dictate(session.recording, null).then(result => {
      const text = result.text.trim();
      complete(session, text || session.previous, text ? "Edit the transcript before reviewing." : "No speech was recognized. Try again.");
    }, error => {
      complete(session, session.previous, isCoreError(error) ? error.message : "Dictation failed. Try again or open the text chat.");
    });
  }

  function startLive(): void {
    const session: Extract<Capture, { mode: "live" }> = {
      mode: "live", id: String(++sequence), phase: "preparing", previous: transcript.value,
    };
    capture = session;
    fallback.hidden = true;
    let partial = "";
    render("Preparing the on-device microphone…");
    const events = new Channel<SpeechEvent>(event => {
      if (capture !== session) return;
      if (event.state === "listening" && session.phase === "preparing") {
        phase(session, "listening", "Listening on this device.");
      } else if (event.state === "partial" && event.text.trim()) {
        partial = event.text.slice(0, transcript.maxLength);
        transcript.value = partial;
      } else if (event.state === "processing") {
        phase(session, "processing", "Transcribing on this device…");
      }
    });
    void invoke<string>("plugin:local-speech|start", {
      session: session.id, language: navigator.language || "en-US", events,
    }).then(text => {
      complete(session, text, "Edit the transcript before reviewing.");
    }, error => {
      if (capture !== session) return;
      const message = typeof error === "string" ? error : "Live dictation could not start.";
      complete(session, partial || session.previous, partial ? message + " Only a partial transcript is available." : message);
      fallback.hidden = false;
    });
  }

  function startRecorded(): void {
    const recording = recordDictation();
    const session: Extract<Capture, { mode: "recorded" }> = {
      mode: "recorded", recording, phase: "preparing", previous: transcript.value,
    };
    capture = session;
    render("Preparing the microphone…");
    void recording.ready.then(() => {
      if (capture === session) phase(session, "listening", "Recording on this device. Stop to see the transcript.");
    }, () => {
      if (capture !== session) return;
      recording.cancel();
      complete(session, session.previous, "Microphone unavailable. Check permission or open the text chat.");
    });
    void recording.limitReached.then(() => finish(session));
  }

  function show(): void {
    panel.hidden = false;
    transcript.value = "";
    fallback.hidden = true;
    render(!available ? "Open the installed app to use the microphone."
      : mode === "live" ? "Start dictation to see recognized words here."
      : "Record a reading, then stop to transcribe it on this device.");
    (available ? mic : element("#voice-type")).focus({ preventScroll: true });
  }

  mic.addEventListener("click", () => {
    if (!available) return;
    if (capture) finish(capture);
    else if (mode === "live") startLive();
    else startRecorded();
  });
  fallback.addEventListener("click", () => {
    mode = "recorded";
    fallback.hidden = true;
    render("Record a reading, then stop to transcribe it on this device.");
    mic.focus({ preventScroll: true });
  });
  transcript.addEventListener("input", () => { review.disabled = !available || capture !== null || !transcript.value.trim(); });
  review.addEventListener("click", () => {
    const text = transcript.value.trim();
    if (available && !capture && text) onReview(text);
  });
  window.addEventListener("pagehide", cancel);
  document.addEventListener("visibilitychange", () => { if (document.hidden) cancel(); });
  return {
    show,
    hide() { cancel(); panel.hidden = true; },
  };
}
