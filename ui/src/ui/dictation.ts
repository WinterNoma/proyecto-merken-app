import { Channel, invoke } from "@tauri-apps/api/core";

type SpeechEvent = { state: "listening" | "processing" } | { state: "partial"; text: string };
type Microphone = {
  button: HTMLButtonElement;
  canStart: () => boolean;
  status: (text: string) => void;
  listening?: (active: boolean) => void;
  transcript: (text: string) => void;
};
type Session = {
  id: string;
  mic: Microphone;
  pointer: number | null;
  stopping: boolean;
  toggle: boolean;
};

export function registerDictation(available: boolean, microphones: Microphone[]) {
  let active: Session | null = null;
  let sequence = 0;

  function showListening(mic: Microphone, listening: boolean): void {
    mic.button.setAttribute("aria-pressed", String(listening));
    mic.listening?.(listening);
  }

  function clear(session: Session): void {
    active = null;
    showListening(session.mic, false);
    const pointer = session.pointer;
    if (pointer !== null && session.mic.button.hasPointerCapture(pointer)) {
      session.mic.button.releasePointerCapture(pointer);
    }
  }

  async function finish(discard = false): Promise<void> {
    const session = active;
    if (!session || (session.stopping && !discard)) return;
    session.stopping = true;
    showListening(session.mic, false);
    if (discard) {
      clear(session);
      session.mic.status("Dictation canceled.");
    } else {
      session.mic.status("Finishing dictation…");
    }
    try {
      await invoke("plugin:local-speech|finish", { session: session.id, discard });
    } catch {
      if (active !== session) return;
      clear(session);
      session.mic.status("Dictation could not finish. You can type instead.");
    }
  }

  function begin(mic: Microphone, pointer: number | null, toggle = false): boolean {
    if (active || !mic.canStart()) return false;
    if (!available) {
      mic.status("On-device dictation requires a supported Android device. You can type instead.");
      return false;
    }
    const session = { id: String(++sequence), mic, pointer, stopping: false, toggle };
    active = session;
    mic.status("Preparing microphone…");
    const events = new Channel<SpeechEvent>(event => {
      if (active !== session) return;
      if (event.state === "listening" && !session.stopping) {
        showListening(mic, true);
        mic.status(toggle ? "Listening on this device. Activate the microphone again to stop." : "Listening on this device…");
      } else if (event.state === "partial" && !session.stopping) {
        mic.status(`Listening · ${event.text}`);
      } else if (event.state === "processing") {
        showListening(mic, false);
        mic.status("Transcribing on this device…");
      }
    });
    void invoke<string>("plugin:local-speech|start", {
      session: session.id,
      language: navigator.language || "en-US",
      events,
    }).then(transcript => {
      if (active !== session) return;
      clear(session);
      mic.status("Transcript ready to review.");
      mic.transcript(transcript);
    }, (error: unknown) => {
      if (active !== session) return;
      clear(session);
      mic.status(typeof error === "string" ? error : "On-device dictation could not start. You can type instead.");
    });
    return true;
  }

  for (const mic of microphones) {
    mic.button.addEventListener("pointerdown", event => {
      if (!event.isPrimary || event.button !== 0 || !begin(mic, event.pointerId)) return;
      event.preventDefault();
      mic.button.focus({ preventScroll: true });
      mic.button.setPointerCapture(event.pointerId);
    });
    mic.button.addEventListener("lostpointercapture", event => {
      if (active?.pointer === event.pointerId && !active.stopping) void finish(true);
    });
    mic.button.addEventListener("keydown", event => {
      if (event.key !== " " && event.key !== "Enter") return;
      event.preventDefault();
      if (!event.repeat) begin(mic, null);
    });
    mic.button.addEventListener("keyup", event => {
      if (event.key !== " " && event.key !== "Enter") return;
      event.preventDefault();
      if (active?.mic === mic && !active.toggle) void finish();
    });
    mic.button.addEventListener("click", event => {
      if (event.detail !== 0) return;
      if (active?.mic === mic && active.toggle) void finish();
      else begin(mic, null, true);
    });
  }
  window.addEventListener("pointerup", event => {
    if (active?.pointer === event.pointerId) void finish();
  }, true);
  window.addEventListener("pointercancel", event => {
    if (active?.pointer === event.pointerId) void finish(true);
  }, true);
  window.addEventListener("keydown", event => {
    if (active && event.key === "Escape") {
      event.preventDefault();
      void finish(true);
    }
  }, true);
  window.addEventListener("blur", () => void finish(true));
  window.addEventListener("pagehide", () => void finish(true));
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) void finish(true);
  });
  return { cancel: () => void finish(true) };
}
