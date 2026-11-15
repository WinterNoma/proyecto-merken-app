const pulseName = /^(?:(?:my|mi)\s+)?(?:pulse|heart rate|pulso|frecuencia cardiaca)\b\s*(?:(?:is|was|es|fue|de)\s+)?[:=]?\s*/;
const pulseUnit = /^(?:b\s*p\s*m|l\s*p\s*m|beats? per minute|latidos? por minuto)\b/;

function normalize(text: string): string {
  return text.trim().toLowerCase().normalize("NFD").replace(/\p{M}/gu, "").replace(/[.!?]+$/, "").trim();
}

export function readPulse(text: string): { bpm: number; when: string } | null {
  let value = normalize(text);
  const named = pulseName.test(value);
  value = value.replace(pulseName, "");
  const reading = value.match(/^(\d{1,3})(?![\d.,])\s*(.*)$/);
  if (!reading || !reading[1] || reading[2] === undefined) return null;
  const hasUnit = pulseUnit.test(reading[2]);
  if (!named && !hasUnit) return null;
  const suffix = reading[2].replace(pulseUnit, "").trim();
  if (!suffix) return { bpm: Number(reading[1]), when: "" };
  const time = suffix.match(/^,?\s*(?:at|a las)\s+(.+)$/)?.[1];
  return time ? { bpm: Number(reading[1]), when: time } : null;
}

export function readUtcTime(text: string, now: Date): string | null {
  const value = normalize(text);
  if (value === "now" || value === "ahora") return now.toISOString();
  const match = value.match(/^(?:(\d{4}-\d{2}-\d{2})[t\s]+)?(\d{1,2})(?::|\s)(\d{2})(?::(\d{2}))?\s*(?:u\s*t\s*c|z)?$/);
  if (!match?.[2] || !match[3]) return null;
  const [, date = now.toISOString().slice(0, 10), hour, minute, second = "00"] = match;
  const input = `${date}T${hour.padStart(2, "0")}:${minute}:${second}.000Z`;
  const parsed = new Date(input);
  return Number.isFinite(parsed.getTime()) && parsed.toISOString() === input ? input : null;
}
