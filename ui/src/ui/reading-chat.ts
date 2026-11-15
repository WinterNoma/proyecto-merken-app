import { el, element } from "./dom";
import { readPulse, readUtcTime } from "./reading-language";

export type PulseProposal = { bpm: number; measuredAt: string };

export function createReadingChat(onReview: (proposal: PulseProposal) => void) {
  const host = element("#reading-chat");
  const log = element("#chat-messages");
  const input = element<HTMLTextAreaElement>("#chat-input");
  let pending: number | null = null;

  function message(author: "assistant" | "text", text: string) {
    const bubble = el("div", "chat-message");
    bubble.dataset.author = author;
    const label = author === "assistant" ? "Onboard assistant" : "You";
    bubble.append(el("span", "chat-author", label), el("p", "", text));
    log.append(bubble);
    log.scrollTop = log.scrollHeight;
    return bubble;
  }

  function propose(bpm: number, measuredAt: string): void {
    pending = null;
    const date = measuredAt.slice(0, 10);
    const time = measuredAt.slice(11, 19);
    const bubble = message("assistant", `Pulse · ${bpm} bpm\nMeasured ${date} at ${time} UTC`);
    const review = el("button", "button secondary chat-review", "Review reading");
    review.type = "button";
    review.addEventListener("click", () => onReview({ bpm, measuredAt }));
    bubble.append(review);
    log.scrollTop = log.scrollHeight;
  }

  element<HTMLFormElement>("#chat-compose").addEventListener("submit", event => {
    event.preventDefault();
    const text = input.value.trim();
    if (!text) return;
    message("text", text);
    input.value = "";
    const now = new Date();
    const pulse = readPulse(text);
    if (pulse) {
      const measuredAt = pulse.when && readUtcTime(pulse.when, now);
      if (measuredAt) {
        propose(pulse.bpm, measuredAt);
      } else {
        pending = pulse.bpm;
        message("assistant", "When was it measured? Enter a UTC time or type “now.” For an earlier date, include YYYY-MM-DD.");
      }
    } else if (pending !== null) {
      const measuredAt = readUtcTime(text, now);
      if (measuredAt) propose(pending, measuredAt);
      else message("assistant", "I still need the measurement time. Try “13:32 UTC” or “now.”");
    } else {
      message("assistant", "I can prepare a pulse reading. Try “Pulse 76 bpm” or use the form for another reading.");
    }
  });

  return {
    show() {
      host.hidden = false;
      pending = null;
      input.value = "";
      log.replaceChildren();
      message("assistant", "Type a pulse reading. I’ll ask for any missing measurement time.");
      input.focus({ preventScroll: true });
    },
    hide() { host.hidden = true; },
  };
}
