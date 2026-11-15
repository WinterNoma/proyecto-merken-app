import assert from "node:assert/strict";
import { test } from "node:test";
import { readPulse, readUtcTime } from "../src/ui/reading-language.ts";

test("reads English and Spanish pulse entries with an optional measurement time", () => {
  assert.deepEqual(readPulse("Pulse 76 bpm."), { bpm: 76, when: "" });
  assert.deepEqual(readPulse("My pulse was 76 beats per minute at 13:32 UTC."), { bpm: 76, when: "13:32 utc" });
  assert.deepEqual(readPulse("Mi frecuencia cardíaca es 76 latidos por minuto a las 13:32 UTC"), { bpm: 76, when: "13:32 utc" });
  assert.deepEqual(readPulse("76 BPM"), { bpm: 76, when: "" });
});

test("does not turn an ambiguous or unrelated input into a pulse reading", () => {
  for (const text of ["76", "13:32 UTC", "sleep 7 hours", "pulse -76", "pulse 76.5 bpm", "pulse 76 or 86 bpm", "pulse 1234 bpm"]) {
    assert.equal(readPulse(text), null, text);
  }
});

test("accepts only unambiguous UTC times and validates calendar dates", () => {
  const now = new Date("2026-11-15T16:30:20.000Z");
  assert.equal(readUtcTime("13:32 UTC", now), "2026-11-15T13:32:00.000Z");
  assert.equal(readUtcTime("13 32", now), "2026-11-15T13:32:00.000Z");
  assert.equal(readUtcTime("2026-11-14 3:05:01Z", now), "2026-11-14T03:05:01.000Z");
  assert.equal(readUtcTime("now", now), now.toISOString());
  for (const time of ["24:00 UTC", "13:60 UTC", "2026-02-30 13:32", "1:32 pm", "13:32 PST"]) {
    assert.equal(readUtcTime(time, now), null, time);
  }
});
