import { el, element } from "./dom";
import { readPulse, readUtcTime } from "./reading-language";

type EntrySource = "text" | "voice";
export type PulseProposal = { bpm: number; measuredAt: string; source: EntrySource };

export function createReadingChat(onReview: (proposal: PulseProposal) => void) {
  const host = element("#reading-chat");
  const log = element("#chat-messages");
  const input = element<HTMLTextAreaElement>("#chat-input");
  const voiceStatus = element("#chat-voice-status");
  let source: EntrySource = "text";
  let pending: { bpm: number; source: EntrySource } | null = null;

  function message(author: "assistant" | EntrySource, text: string) {
    const bubble = el("div", "chat-message");
    bubble.dataset.author = author;
    const label = author === "assistant" ? "Onboard assistant" : author === "voice" ? "You · Dictated" : "You";
    bubble.append(el("span", "chat-author", label), el("p", "", text));
    log.append(bubble);
    log.scrollTop = log.scrollHeight;
    return bubble;
  }

  function propose(bpm: number, measuredAt: string, entrySource: EntrySource): void {
    pending = null;
    const date = measuredAt.slice(0, 10);
    const time = measuredAt.slice(11, 19);
    const bubble = message("assistant", `Pulse · ${bpm} bpm\nMeasured ${date} at ${time} UTC`);
    const review = el("button", "button secondary chat-review", "Review reading");
    review.type = "button";
    review.addEventListener("click", () => onReview({ bpm, measuredAt, source: entrySource }));
    bubble.append(review);
    log.scrollTop = log.scrollHeight;
  }

  element<HTMLFormElement>("#chat-compose").addEventListener("submit", event => {
    event.preventDefault();
    const text = input.value.trim();
    if (!text) return;
    message(source, text);
    const entrySource = source;
    source = "text";
    input.value = "";
    voiceStatus.textContent = "";
    const now = new Date();
    const pulse = readPulse(text);
    if (pulse) {
      const measuredAt = pulse.when && readUtcTime(pulse.when, now);
      if (measuredAt) {
        propose(pulse.bpm, measuredAt, entrySource);
      } else {
        pending = { bpm: pulse.bpm, source: entrySource };
        message("assistant", "When was it measured? Enter a UTC time or type “now.” For an earlier date, include YYYY-MM-DD.");
      }
    } else if (pending) {
      const measuredAt = readUtcTime(text, now);
      if (measuredAt) propose(pending.bpm, measuredAt, entrySource === "voice" ? "voice" : pending.source);
      else message("assistant", "I still need the measurement time. Try “13:32 UTC” or “now.”");
    } else {
      message("assistant", "I can prepare a pulse reading. Try “Pulse 76 bpm” or use the form for another reading.");
    }
  });

  return {
    show(reset: boolean) {
      host.hidden = false;
      if (reset || !log.hasChildNodes()) {
        pending = null;
        source = "text";
        input.value = "";
        log.replaceChildren();
        message("assistant", "Type or dictate a pulse reading. I’ll ask for any missing measurement time.");
      }
    },
    hide() { host.hidden = true; },
    transcript(text: string) {
      const start = input.selectionStart;
      const end = input.selectionEnd;
      const prefix = start > 0 && !/\s$/.test(input.value.slice(0, start)) ? " " : "";
      const suffix = end < input.value.length && !/^\s/.test(input.value.slice(end)) ? " " : "";
      const room = input.maxLength - (input.value.length - (end - start));
      input.setRangeText((prefix + text.trim() + suffix).slice(0, room), start, end, "end");
      source = "voice";
    },
  };
}
