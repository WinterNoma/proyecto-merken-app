//! Segmentación de cláusulas, resolución de negaciones y extracción conservadora de registros.

use super::{
    RULES_INTERPRETER_VERSION,
    lexicon::{
        Token, TokenKind, UnitKind, is_bp_keyword_at, is_general_symptom_zone_word, is_hour_unit,
        is_minute_unit, is_negated_good_sleep, is_negation_cue_in_words, is_sleep_word,
        is_symptom_cue, match_body_zone_phrase, match_unit_at, n1_n2_bp_plausible, parse_body_zone,
        sleep_quality_word, slice_from_bytes, slice_tokens, starts_with_words, tokenize,
        truncate_chars, word_at,
    },
    numbers::{
        NumberScan, ParsedNumber, en_sub_hundred, es_ten, es_unit_in_compound, number_to_json,
        parse_simple_number, scan_number,
    },
    time::{MatchedTime, TimeMask, find_time_hints},
};
use crate::{
    catalog::{BodyZone, Indicator, SleepQuality},
    measurement::Measurement,
    proposal::{
        InterpreterKind, PROPOSAL_SCHEMA_VERSION, Proposal, ProposedField, ProposedRecord, TimeHint,
    },
    units::{Bpm, Celsius, Hours, MgDl, MmHg, Percent, Score},
};
use serde_json::{Number, Value, json};

#[derive(Clone, Copy, Debug)]
struct ClauseSpan {
    start: usize,
    end: usize,
    after_comma: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnchorKind {
    HeartRate,
    BloodPressure,
    BodyTemperature,
    Glucose,
    OxygenInBlood,
    Mood,
    Sleep,
    Symptom,
}

#[derive(Clone, Copy, Debug)]
struct RawAnchor {
    kind: AnchorKind,
    start: usize,
    end: usize,
    is_keyword: bool,
    zone: Option<BodyZone>,
}

#[derive(Clone, Copy, Debug)]
struct AnchorGroup {
    kind: AnchorKind,
    anchor_start: usize,
    anchor_end: usize,
    primary_zone: Option<BodyZone>,
    has_conflict: bool,
}

struct SegmentCtx<'a> {
    orig: &'a str,
    tokens: &'a [Token],
    start: usize,
    end: usize,
    anchor_start: usize,
    time_mask: &'a TimeMask,
    time_hint: TimeHint,
    has_conflict: bool,
}

#[derive(Default)]
struct ProposalOut {
    records: Vec<ProposedRecord>,
    record_start_bytes: Vec<usize>,
    negated: Vec<String>,
    uninterpreted: Vec<String>,
    last_was_open_sleep: bool,
}

fn field_some(key: &str, value: Value, quote: String) -> ProposedField {
    ProposedField {
        key: key.to_owned(),
        value: Some(value),
        quote: Some(quote),
    }
}

fn field_none(key: &str) -> ProposedField {
    ProposedField {
        key: key.to_owned(),
        value: None,
        quote: None,
    }
}

fn is_clause_break_word(w: &str) -> bool {
    matches!(
        w,
        "pero"
            | "but"
            | "aunque"
            | "although"
            | "mientras"
            | "while"
            | "ademas"
            | "also"
            | "however"
            | "then"
            | "luego"
    )
}

fn is_indicator_or_symptom_start(tokens: &[Token], idx: usize, end: usize) -> bool {
    if idx >= end {
        return false;
    }
    let Some(w) = word_at(tokens, idx) else {
        if let Some(num) = parse_simple_number(tokens, idx) {
            return num.end_idx < end
                && (tokens.get(num.end_idx).is_some_and(|tk| {
                    matches!(tk.kind, TokenKind::Slash | TokenKind::Percent)
                        || matches!(tk.norm.as_str(), "°" | "º")
                }) || matches!(
                    word_at(tokens, num.end_idx),
                    Some(
                        "bpm"
                            | "lpm"
                            | "pulsaciones"
                            | "latidos"
                            | "celsius"
                            | "centigrados"
                            | "grados"
                            | "degrees"
                            | "mg"
                            | "mmhg"
                    )
                ));
        }
        return false;
    };
    matches!(
        w,
        "pulso"
            | "pulse"
            | "frecuencia"
            | "heart"
            | "hr"
            | "bpm"
            | "lpm"
            | "pulsaciones"
            | "latidos"
            | "presion"
            | "tension"
            | "blood"
            | "bp"
            | "temperatura"
            | "temp"
            | "body"
            | "fiebre"
            | "fever"
            | "febricula"
            | "glucosa"
            | "glucemia"
            | "glucose"
            | "azucar"
            | "sugar"
            | "saturacion"
            | "saturation"
            | "oxigeno"
            | "oxygen"
            | "spo2"
            | "o2"
            | "animo"
            | "mood"
            | "dormi"
            | "dormir"
            | "sueno"
            | "slept"
            | "sleep"
            | "sleeping"
            | "descanse"
            | "descanso"
            | "me"
            | "dolor"
            | "duele"
            | "duelen"
            | "molestia"
            | "molesta"
            | "pain"
            | "hurt"
            | "hurts"
            | "hurting"
            | "ache"
            | "aches"
            | "aching"
            | "headache"
            | "backache"
            | "migraine"
            | "migrana"
            | "jaqueca"
            | "cefalea"
            | "nausea"
            | "nauseas"
            | "mareo"
            | "mareos"
            | "dizzy"
            | "dizziness"
            | "vertigo"
            | "fatiga"
            | "fatigue"
            | "cansancio"
            | "calambre"
            | "cramp"
            | "no"
            | "ya"
            | "sin"
            | "tampoco"
            | "ningun"
            | "ninguna"
            | "dont"
            | "doesnt"
            | "didnt"
            | "without"
            | "my"
            | "mi"
            | "tengo"
            | "siento"
            | "estoy"
            | "i"
            | "im"
            | "ive"
    )
}

fn should_split_conjunction(
    tokens: &[Token],
    idx: usize,
    sub_start: usize,
    clause_start: usize,
    clause_end: usize,
    has_anchor_after: &[bool],
) -> bool {
    let Some(w) = word_at(tokens, idx) else {
        return false;
    };
    if !matches!(w, "y" | "e" | "and") {
        return false;
    }
    if idx == sub_start || idx + 1 >= clause_end {
        return false;
    }

    // Mantener "cinco y media" / "five and a half"
    if w == "y" && matches!(word_at(tokens, idx + 1), Some("media" | "medio")) {
        return false;
    }
    if w == "and"
        && (starts_with_words(tokens, idx + 1, &["a", "half"])
            || word_at(tokens, idx + 1) == Some("half"))
    {
        return false;
    }

    // Mantener decenas + unidad en español: "setenta y dos"
    if w == "y"
        && let Some(prev) = word_at(tokens, idx - 1)
        && (es_ten(prev).is_some() || prev == "diez")
        && let Some(next) = word_at(tokens, idx + 1)
        && es_unit_in_compound(next).is_some()
    {
        return false;
    }

    // Mantener inglés "one hundred and seventy two"
    if w == "and"
        && word_at(tokens, idx - 1) == Some("hundred")
        && en_sub_hundred(tokens, idx + 1).is_some()
    {
        return false;
    }

    // Mantener "5 horas y 30 minutos" / "5 hours and 30 minutes"
    if matches!(
        word_at(tokens, idx - 1),
        Some("hora" | "horas" | "hour" | "hours" | "h" | "hr" | "hrs")
    ) && let Some(num) = parse_simple_number(tokens, idx + 1)
        && num.end_idx < clause_end
        && word_at(tokens, num.end_idx).is_some_and(is_minute_unit)
    {
        return false;
    }

    let rel_next = (idx + 1).saturating_sub(clause_start);
    has_anchor_after.get(rel_next).copied().unwrap_or(false)
}

fn split_clauses(tokens: &[Token]) -> Vec<ClauseSpan> {
    let mut rough: Vec<ClauseSpan> = Vec::new();
    let mut start = 0;
    let mut next_after_comma = false;
    let n = tokens.len();

    for (idx, tk) in tokens.iter().enumerate() {
        let is_sep = tk.kind == TokenKind::ClauseSep
            || (tk.kind == TokenKind::Word && is_clause_break_word(&tk.norm));
        if is_sep {
            if start < idx {
                rough.push(ClauseSpan {
                    start,
                    end: idx,
                    after_comma: next_after_comma,
                });
            }
            next_after_comma = tk.kind == TokenKind::ClauseSep && tk.norm == ",";
            start = idx + 1;
        }
    }
    if start < n {
        rough.push(ClauseSpan {
            start,
            end: n,
            after_comma: next_after_comma,
        });
    }

    let mut refined = Vec::new();
    for c in rough {
        let c_len = c.end.saturating_sub(c.start);
        let mut has_anchor_after = vec![false; c_len + 1];
        for rel in (0..c_len).rev() {
            let abs = c.start + rel;
            let here = is_indicator_or_symptom_start(tokens, abs, c.end)
                || match_body_zone_phrase(tokens, abs, c.end).is_some();
            let next = has_anchor_after.get(rel + 1).copied().unwrap_or(false);
            if let Some(slot) = has_anchor_after.get_mut(rel) {
                *slot = here || next;
            }
        }

        let mut sub_start = c.start;
        let mut sub_after_comma = c.after_comma;
        let mut i = c.start;
        while i < c.end {
            if should_split_conjunction(tokens, i, sub_start, c.start, c.end, &has_anchor_after) {
                if sub_start < i {
                    refined.push(ClauseSpan {
                        start: sub_start,
                        end: i,
                        after_comma: sub_after_comma,
                    });
                }
                sub_start = i + 1;
                sub_after_comma = false;
            }
            i += 1;
        }
        if sub_start < c.end {
            refined.push(ClauseSpan {
                start: sub_start,
                end: c.end,
                after_comma: sub_after_comma,
            });
        }
    }
    refined
}

fn detect_anchor(tokens: &[Token], idx: usize, end: usize) -> Option<RawAnchor> {
    if let Some(bp_len) = is_bp_keyword_at(tokens, idx, end) {
        return Some(RawAnchor {
            kind: AnchorKind::BloodPressure,
            start: idx,
            end: idx + bp_len,
            is_keyword: true,
            zone: None,
        });
    }
    if starts_with_words(tokens, idx, &["heart", "rate"])
        || starts_with_words(tokens, idx, &["frecuencia", "cardiaca"])
        || starts_with_words(tokens, idx, &["ritmo", "cardiaco"])
    {
        return Some(RawAnchor {
            kind: AnchorKind::HeartRate,
            start: idx,
            end: idx + 2,
            is_keyword: true,
            zone: None,
        });
    }
    if starts_with_words(tokens, idx, &["body", "temperature"])
        || starts_with_words(tokens, idx, &["body", "temp"])
    {
        return Some(RawAnchor {
            kind: AnchorKind::BodyTemperature,
            start: idx,
            end: idx + 2,
            is_keyword: true,
            zone: None,
        });
    }
    if starts_with_words(tokens, idx, &["blood", "glucose"])
        || starts_with_words(tokens, idx, &["blood", "sugar"])
    {
        return Some(RawAnchor {
            kind: AnchorKind::Glucose,
            start: idx,
            end: idx + 2,
            is_keyword: true,
            zone: None,
        });
    }
    if starts_with_words(tokens, idx, &["azucar", "en", "sangre"]) {
        return Some(RawAnchor {
            kind: AnchorKind::Glucose,
            start: idx,
            end: idx + 3,
            is_keyword: true,
            zone: None,
        });
    }
    if starts_with_words(tokens, idx, &["oxygen", "saturation"])
        || starts_with_words(tokens, idx, &["blood", "oxygen"])
        || starts_with_words(tokens, idx, &["o2", "sat"])
    {
        return Some(RawAnchor {
            kind: AnchorKind::OxygenInBlood,
            start: idx,
            end: idx + 2,
            is_keyword: true,
            zone: None,
        });
    }
    if starts_with_words(tokens, idx, &["saturacion", "de", "oxigeno"])
        || starts_with_words(tokens, idx, &["oxigeno", "en", "sangre"])
    {
        return Some(RawAnchor {
            kind: AnchorKind::OxygenInBlood,
            start: idx,
            end: idx + 3,
            is_keyword: true,
            zone: None,
        });
    }
    if starts_with_words(tokens, idx, &["estado", "de", "animo"]) {
        return Some(RawAnchor {
            kind: AnchorKind::Mood,
            start: idx,
            end: idx + 3,
            is_keyword: true,
            zone: None,
        });
    }

    if let Some(w) = word_at(tokens, idx) {
        if matches!(w, "pulso" | "pulse" | "hr") {
            return Some(RawAnchor {
                kind: AnchorKind::HeartRate,
                start: idx,
                end: idx + 1,
                is_keyword: true,
                zone: None,
            });
        }
        if matches!(w, "bpm" | "lpm" | "pulsaciones" | "latidos") {
            return Some(RawAnchor {
                kind: AnchorKind::HeartRate,
                start: idx,
                end: idx + 1,
                is_keyword: false,
                zone: None,
            });
        }
        if matches!(w, "temperatura" | "temp" | "fiebre" | "fever" | "febricula") {
            return Some(RawAnchor {
                kind: AnchorKind::BodyTemperature,
                start: idx,
                end: idx + 1,
                is_keyword: true,
                zone: None,
            });
        }
        if matches!(w, "celsius" | "centigrados" | "centigrado") {
            return Some(RawAnchor {
                kind: AnchorKind::BodyTemperature,
                start: idx,
                end: idx + 1,
                is_keyword: false,
                zone: None,
            });
        }
        if matches!(w, "glucosa" | "glucemia" | "glucose" | "azucar") {
            return Some(RawAnchor {
                kind: AnchorKind::Glucose,
                start: idx,
                end: idx + 1,
                is_keyword: true,
                zone: None,
            });
        }
        if matches!(
            w,
            "saturacion" | "saturation" | "oxigeno" | "oxygen" | "spo2" | "o2"
        ) {
            return Some(RawAnchor {
                kind: AnchorKind::OxygenInBlood,
                start: idx,
                end: idx + 1,
                is_keyword: true,
                zone: None,
            });
        }
        if matches!(w, "animo" | "mood" | "humor") {
            return Some(RawAnchor {
                kind: AnchorKind::Mood,
                start: idx,
                end: idx + 1,
                is_keyword: true,
                zone: None,
            });
        }
        if is_sleep_word(w) {
            return Some(RawAnchor {
                kind: AnchorKind::Sleep,
                start: idx,
                end: idx + 1,
                is_keyword: true,
                zone: None,
            });
        }
        if is_symptom_cue(w) {
            let zone = if is_general_symptom_zone_word(w) {
                Some(BodyZone::Other)
            } else {
                match_body_zone_phrase(tokens, idx, end).map(|(z, _)| z)
            };
            return Some(RawAnchor {
                kind: AnchorKind::Symptom,
                start: idx,
                end: idx + 1,
                is_keyword: true,
                zone,
            });
        }
    }

    if let Some((zone, z_end)) = match_body_zone_phrase(tokens, idx, end) {
        return Some(RawAnchor {
            kind: AnchorKind::Symptom,
            start: idx,
            end: z_end,
            is_keyword: true,
            zone: Some(zone),
        });
    }

    // Número independiente con unidad explícita (NO incluye horas sueltas para no inferir sueño):
    // "120/80", "36.8°C", "95 mg/dl", "72 bpm"
    if let Some(num) = parse_simple_number(tokens, idx)
        && num.end_idx < end
    {
        let next = num.end_idx;
        if tokens
            .get(next)
            .is_some_and(|tk| tk.kind == TokenKind::Slash)
            && let Some(n2) = parse_simple_number(tokens, next + 1)
            && n2.end_idx <= end
            && n1_n2_bp_plausible(num.value, n2.value)
        {
            return Some(RawAnchor {
                kind: AnchorKind::BloodPressure,
                start: idx,
                end: n2.end_idx,
                is_keyword: false,
                zone: None,
            });
        }
        if let Some((unit, u_end)) = match_unit_at(tokens, next, end) {
            match unit {
                UnitKind::HeartRate => {
                    return Some(RawAnchor {
                        kind: AnchorKind::HeartRate,
                        start: idx,
                        end: u_end,
                        is_keyword: false,
                        zone: None,
                    });
                }
                UnitKind::TemperatureCelsius => {
                    // No usar una "c" suelta sin símbolo de grado ni "celsius"/"grados" como ancla independiente.
                    if word_at(tokens, next) != Some("c") {
                        return Some(RawAnchor {
                            kind: AnchorKind::BodyTemperature,
                            start: idx,
                            end: u_end,
                            is_keyword: false,
                            zone: None,
                        });
                    }
                }
                UnitKind::Glucose => {
                    return Some(RawAnchor {
                        kind: AnchorKind::Glucose,
                        start: idx,
                        end: u_end,
                        is_keyword: false,
                        zone: None,
                    });
                }
                _ => {}
            }
        }
    }

    None
}

fn is_correction_word(w: &str) -> bool {
    matches!(
        w,
        "perdon" | "sorry" | "digo" | "corrijo" | "rectifico" | "actually" | "rather"
    )
}

fn has_correction_between(tokens: &[Token], from: usize, to: usize) -> bool {
    for k in from..to {
        if word_at(tokens, k).is_some_and(is_correction_word) {
            return true;
        }
        if k + 1 < to
            && (starts_with_words(tokens, k, &["mejor", "dicho"])
                || starts_with_words(tokens, k, &["quise", "decir"])
                || starts_with_words(tokens, k, &["quiero", "decir"])
                || starts_with_words(tokens, k, &["o", "sea"])
                || starts_with_words(tokens, k, &["en", "realidad"])
                || starts_with_words(tokens, k, &["i", "mean"]))
        {
            return true;
        }
    }
    false
}

fn is_non_clinical_prefix_word(w: Option<&str>) -> bool {
    matches!(
        w,
        Some(
            "capitulo"
                | "chapter"
                | "pagina"
                | "page"
                | "ejercicio"
                | "ejercicios"
                | "exercise"
                | "exercises"
                | "piso"
                | "floor"
                | "sala"
                | "room"
                | "vuelo"
                | "flight"
                | "modulo"
                | "module"
                | "fase"
                | "phase"
                | "sesion"
                | "session"
                | "dia"
                | "day"
                | "vez"
                | "veces"
                | "times"
        )
    )
}

struct NumberSearchResult {
    chosen: Option<ParsedNumber>,
    ambiguous: bool,
}

fn find_unique_number(
    tokens: &[Token],
    seg_start: usize,
    seg_end: usize,
    anchor_start: usize,
    time_mask: &TimeMask,
    target: Indicator,
) -> NumberSearchResult {
    let allow_fraction_10 = matches!(target, Indicator::Mood | Indicator::Symptom);
    let mut candidates: Vec<ParsedNumber> = Vec::new();
    let mut had_rejected = false;
    let mut range_ambiguous = false;
    let mut i = seg_start;

    while i < seg_end {
        if time_mask.is_masked(i) {
            i += 1;
            continue;
        }
        match scan_number(tokens, i, seg_end, allow_fraction_10) {
            Some(NumberScan::Rejected { end_idx }) => {
                had_rejected = true;
                i = end_idx.max(i + 1);
            }
            Some(NumberScan::Valid(num)) => {
                if time_mask.any_masked(i, num.end_idx) {
                    i = num.end_idx;
                    continue;
                }
                let (has_matching_unit, compatible, next_i) =
                    match match_unit_at(tokens, num.end_idx, seg_end) {
                        Some((unit, u_end)) => (
                            unit.matches_indicator(target),
                            unit.matches_indicator(target),
                            u_end,
                        ),
                        None => (false, true, num.end_idx),
                    };

                let has_slash_10 = allow_fraction_10
                    && (i..num.end_idx)
                        .any(|k| tokens.get(k).is_some_and(|tk| tk.kind == TokenKind::Slash));
                let explicit_unit = has_matching_unit || has_slash_10;

                let bad_prefix =
                    i > seg_start && is_non_clinical_prefix_word(word_at(tokens, i - 1));
                let local_ok = i >= anchor_start || explicit_unit;

                if compatible && !bad_prefix && local_ok {
                    if next_i + 1 < seg_end
                        && tokens
                            .get(next_i)
                            .is_some_and(|tk| tk.kind == TokenKind::Hyphen)
                        && tokens.get(next_i + 1).is_some_and(|tk| {
                            matches!(tk.kind, TokenKind::Number | TokenKind::InvalidNumber)
                        })
                    {
                        range_ambiguous = true;
                    }
                    candidates.push(num);
                } else if !compatible {
                    had_rejected = true;
                }
                i = next_i;
            }
            None => {
                i += 1;
            }
        }
    }

    if candidates.len() >= 2 || range_ambiguous {
        return NumberSearchResult {
            chosen: None,
            ambiguous: true,
        };
    }
    if candidates.len() == 1 && had_rejected && has_correction_between(tokens, seg_start, seg_end) {
        return NumberSearchResult {
            chosen: None,
            ambiguous: true,
        };
    }
    NumberSearchResult {
        chosen: candidates.first().copied(),
        ambiguous: false,
    }
}

fn parse_sleep_hours_at(
    tokens: &[Token],
    idx: usize,
    end: usize,
    anchor_start: usize,
    time_mask: &TimeMask,
) -> Option<(f64, Option<u64>, usize, bool)> {
    if time_mask.is_masked(idx) {
        return None;
    }

    // "media hora" / "half hour" / "half an hour"
    if (starts_with_words(tokens, idx, &["media", "hora"])
        || starts_with_words(tokens, idx, &["half", "hour"]))
        && idx + 2 <= end
        && !time_mask.any_masked(idx, idx + 2)
    {
        return Some((0.5, None, idx + 2, true));
    }
    if starts_with_words(tokens, idx, &["half", "an", "hour"])
        && idx + 3 <= end
        && !time_mask.any_masked(idx, idx + 3)
    {
        return Some((0.5, None, idx + 3, true));
    }

    match scan_number(tokens, idx, end, false)? {
        NumberScan::Rejected { end_idx } => Some((f64::NAN, None, end_idx, false)),
        NumberScan::Valid(num) => {
            if time_mask.any_masked(idx, num.end_idx) {
                return Some((f64::NAN, None, num.end_idx, false));
            }
            let mut val = num.value;
            let mut num_end = num.end_idx;
            let mut is_int = num.int_value;
            let mut has_hour_unit = false;

            if num_end < end
                && let Some((unit, u_end)) = match_unit_at(tokens, num_end, end)
            {
                if unit != UnitKind::SleepHours {
                    // p. ej. "20 minutos", "6 ejercicios": rechazar sin volver a intentar.
                    return Some((f64::NAN, None, u_end, false));
                }
                has_hour_unit = true;
                let unit_idx = num_end;
                num_end = u_end;

                if (starts_with_words(tokens, unit_idx + 1, &["y", "media"])
                    || starts_with_words(tokens, unit_idx + 1, &["y", "medio"]))
                    && unit_idx + 3 <= end
                    && let Some(iv) = is_int
                    && let Ok(parsed) = format!("{iv}.5").parse::<f64>()
                {
                    val = parsed;
                    is_int = None;
                    num_end = unit_idx + 3;
                } else if starts_with_words(tokens, unit_idx + 1, &["and", "a", "half"])
                    && unit_idx + 4 <= end
                    && let Some(iv) = is_int
                    && let Ok(parsed) = format!("{iv}.5").parse::<f64>()
                {
                    val = parsed;
                    is_int = None;
                    num_end = unit_idx + 4;
                } else if starts_with_words(tokens, unit_idx + 1, &["and", "half"])
                    && unit_idx + 3 <= end
                    && let Some(iv) = is_int
                    && let Ok(parsed) = format!("{iv}.5").parse::<f64>()
                {
                    val = parsed;
                    is_int = None;
                    num_end = unit_idx + 3;
                } else if matches!(word_at(tokens, unit_idx + 1), Some("y" | "and"))
                    && let Some(mins) = parse_simple_number(tokens, unit_idx + 2)
                    && mins.end_idx < end
                    && word_at(tokens, mins.end_idx).is_some_and(is_minute_unit)
                {
                    // Solo convierte minutos añadidos a horas si la fracción es decimal exacta (múltiplo de 3 min)
                    if mins
                        .int_value
                        .is_some_and(|m| m > 0 && m < 60 && m % 3 == 0)
                    {
                        val += mins.value / 60.0;
                        is_int = None;
                        num_end = mins.end_idx + 1;
                    } else {
                        return Some((f64::NAN, None, mins.end_idx + 1, false));
                    }
                }
            }

            let bad_prefix = idx > 0 && is_non_clinical_prefix_word(word_at(tokens, idx - 1));
            let local_ok = idx >= anchor_start || has_hour_unit;
            if bad_prefix || !local_ok {
                return Some((f64::NAN, None, num_end, false));
            }
            Some((val, is_int, num_end, true))
        }
    }
}

fn extract_sleep_record(ctx: &SegmentCtx<'_>) -> (Option<ProposedRecord>, bool) {
    let Some(quote) = slice_tokens(ctx.orig, ctx.tokens, ctx.start, ctx.end) else {
        return (None, false);
    };
    let mut hours_field = field_none("hours");
    let mut quality_field = field_none("quality");
    let mut ambiguous = ctx.has_conflict;

    if !ctx.has_conflict {
        // 1. Extracción de horas (conservadora: rechaza unidades ajenas y números competidores)
        let mut candidates: Vec<(f64, Option<u64>, usize, usize)> = Vec::new();
        let mut i = ctx.start;
        while i < ctx.end {
            if let Some((val, is_int, next_i, valid)) =
                parse_sleep_hours_at(ctx.tokens, i, ctx.end, ctx.anchor_start, ctx.time_mask)
            {
                if valid && val.is_finite() {
                    candidates.push((val, is_int, i, next_i));
                }
                i = next_i.max(i + 1);
            } else {
                i += 1;
            }
        }
        if candidates.len() >= 2 {
            ambiguous = true;
        } else if let Some(&(val, is_int, h_start, h_end)) = candidates.first()
            && val > 0.0
            && Hours::try_from(val).is_ok()
            && let Some(val_quote) = slice_tokens(ctx.orig, ctx.tokens, h_start, h_end)
        {
            let json_val = if let Some(iv) = is_int {
                Some(Value::from(iv))
            } else {
                Number::from_f64(val).map(Value::Number)
            };
            if let Some(jv) = json_val {
                hours_field = field_some("hours", jv, val_quote);
            }
        }

        // 2. Extracción de calidad
        if is_negated_good_sleep(ctx.tokens, ctx.start, ctx.end) {
            quality_field =
                field_some("quality", json!(SleepQuality::Poor.as_str()), quote.clone());
        } else {
            let mut found_q: Option<(SleepQuality, String)> = None;
            let mut q_conflict = false;
            let mut q_idx = ctx.start;
            while q_idx < ctx.end {
                if ctx.time_mask.is_masked(q_idx) {
                    q_idx += 1;
                    continue;
                }
                if starts_with_words(ctx.tokens, q_idx, &["mas", "o", "menos"])
                    && q_idx + 3 <= ctx.end
                    && let Some(q_quote) = slice_tokens(ctx.orig, ctx.tokens, q_idx, q_idx + 3)
                {
                    if found_q
                        .as_ref()
                        .is_some_and(|(prev, _)| *prev != SleepQuality::Fair)
                    {
                        q_conflict = true;
                    } else {
                        found_q = Some((SleepQuality::Fair, q_quote));
                    }
                    q_idx += 3;
                    continue;
                }
                if starts_with_words(ctx.tokens, q_idx, &["so", "so"])
                    && q_idx + 2 <= ctx.end
                    && let Some(q_quote) = slice_tokens(ctx.orig, ctx.tokens, q_idx, q_idx + 2)
                {
                    if found_q
                        .as_ref()
                        .is_some_and(|(prev, _)| *prev != SleepQuality::Fair)
                    {
                        q_conflict = true;
                    } else {
                        found_q = Some((SleepQuality::Fair, q_quote));
                    }
                    q_idx += 2;
                    continue;
                }
                if let Some(w) = word_at(ctx.tokens, q_idx)
                    && let Some(quality) = sleep_quality_word(w)
                    && let Some(q_quote) = slice_tokens(ctx.orig, ctx.tokens, q_idx, q_idx + 1)
                {
                    if found_q.as_ref().is_some_and(|(prev, _)| *prev != quality) {
                        q_conflict = true;
                    } else {
                        found_q = Some((quality, q_quote));
                    }
                }
                q_idx += 1;
            }
            if q_conflict {
                ambiguous = true;
            } else if let Some((quality, q_quote)) = found_q {
                quality_field = field_some("quality", json!(quality.as_str()), q_quote);
            }
        }
    }

    (
        Some(ProposedRecord {
            indicator: Indicator::Sleep,
            fields: vec![
                hours_field,
                quality_field,
                field_none("window_start"),
                field_none("window_end"),
            ],
            time: ctx.time_hint.clone(),
            quote,
        }),
        ambiguous,
    )
}

fn extract_symptom_record(ctx: &SegmentCtx<'_>) -> (Option<ProposedRecord>, bool) {
    let Some(quote) = slice_tokens(ctx.orig, ctx.tokens, ctx.start, ctx.end) else {
        return (None, false);
    };

    let mut zone_field = field_none("body_zone");
    if !ctx.has_conflict {
        if let Some((zone, z_start, z_end)) = parse_body_zone(ctx.tokens, ctx.start, ctx.end)
            && let Some(z_quote) = slice_tokens(ctx.orig, ctx.tokens, z_start, z_end)
        {
            zone_field = field_some("body_zone", json!(zone.as_str()), z_quote);
        } else {
            let other_idx = (ctx.start..ctx.end)
                .find(|&idx| word_at(ctx.tokens, idx).is_some_and(is_general_symptom_zone_word));
            if let Some(idx) = other_idx
                && let Some(z_quote) = slice_tokens(ctx.orig, ctx.tokens, idx, idx + 1)
            {
                zone_field = field_some("body_zone", json!(BodyZone::Other.as_str()), z_quote);
            }
        }
    }

    let mut score_field = field_none("score");
    let mut ambiguous = ctx.has_conflict;
    if !ctx.has_conflict {
        let search = find_unique_number(
            ctx.tokens,
            ctx.start,
            ctx.end,
            ctx.anchor_start,
            ctx.time_mask,
            Indicator::Symptom,
        );
        ambiguous = search.ambiguous;
        if let Some(num) = search.chosen
            && num.value.fract() == 0.0
            && (0.0..=10.0).contains(&num.value)
            && let Some(int_v) = num.int_value.or_else(|| {
                (0_u64..=10).find(|&x| f64::from(u32::try_from(x).unwrap_or(u32::MAX)) == num.value)
            })
            && let Ok(sc_u8) = u8::try_from(int_v)
            && Score::try_from(sc_u8).is_ok()
            && let Some(sc_quote) = slice_from_bytes(ctx.orig, num.start_byte, num.end_byte)
        {
            score_field = field_some("score", json!(sc_u8), sc_quote);
        }
    }

    let desc_quote = truncate_chars(&quote, 2000);
    let desc_field = field_some("description", json!(desc_quote), desc_quote);

    (
        Some(ProposedRecord {
            indicator: Indicator::Symptom,
            fields: vec![zone_field, score_field, desc_field],
            time: ctx.time_hint.clone(),
            quote,
        }),
        ambiguous,
    )
}

fn find_bp_pairs(
    tokens: &[Token],
    start: usize,
    end: usize,
    anchor_start: usize,
    time_mask: &TimeMask,
    has_keyword: bool,
) -> (Option<(ParsedNumber, ParsedNumber)>, bool) {
    let mut pairs: Vec<(ParsedNumber, ParsedNumber)> = Vec::new();
    let mut i = start;

    while i < end {
        if time_mask.is_masked(i) {
            i += 1;
            continue;
        }
        if tokens
            .get(i)
            .is_some_and(|tk| tk.kind == TokenKind::InvalidNumber)
        {
            // No saltar al denominador de una presión inválida (p. ej. "-120/80").
            if i + 2 < end
                && tokens
                    .get(i + 1)
                    .is_some_and(|tk| tk.kind == TokenKind::Slash)
                && let Some(n2) = parse_simple_number(tokens, i + 2)
            {
                i = n2.end_idx;
            } else {
                i += 1;
            }
            continue;
        }
        if let Some(n1) = parse_simple_number(tokens, i)
            && n1.end_idx < end
        {
            let sep_idx = n1.end_idx;
            let is_slash = tokens
                .get(sep_idx)
                .is_some_and(|tk| tk.kind == TokenKind::Slash);
            let is_over = matches!(word_at(tokens, sep_idx), Some("sobre" | "over"));
            if (is_slash || is_over)
                && let Some(n2) = parse_simple_number(tokens, sep_idx + 1)
                && n2.end_idx <= end
            {
                let any_time = time_mask.any_masked(i, n2.end_idx);
                if !any_time {
                    let is_score_slash =
                        is_slash && n2.value == 10.0 && n1.value <= 10.0 && !has_keyword;
                    let plausible_standalone = n1_n2_bp_plausible(n1.value, n2.value);
                    let unit_ok = match match_unit_at(tokens, n2.end_idx, end) {
                        Some((u, _)) => u == UnitKind::BloodPressure,
                        None => true,
                    };
                    let local_ok = i >= anchor_start || unit_ok;
                    if !is_score_slash
                        && (has_keyword || plausible_standalone)
                        && unit_ok
                        && local_ok
                    {
                        pairs.push((n1, n2));
                    }
                }
                i = n2.end_idx;
                continue;
            }
            i = n1.end_idx;
            continue;
        }
        i += 1;
    }

    if pairs.len() >= 2 {
        (None, true)
    } else {
        (pairs.first().copied(), false)
    }
}

fn extract_bp_record(ctx: &SegmentCtx<'_>, has_keyword: bool) -> (Option<ProposedRecord>, bool) {
    let Some(quote) = slice_tokens(ctx.orig, ctx.tokens, ctx.start, ctx.end) else {
        return (None, false);
    };
    if ctx.has_conflict {
        return (
            Some(ProposedRecord {
                indicator: Indicator::BloodPressure,
                fields: vec![field_none("systolic"), field_none("diastolic")],
                time: ctx.time_hint.clone(),
                quote,
            }),
            true,
        );
    }

    let (pair, pair_ambiguous) = find_bp_pairs(
        ctx.tokens,
        ctx.start,
        ctx.end,
        ctx.anchor_start,
        ctx.time_mask,
        has_keyword,
    );
    if pair_ambiguous {
        return (
            Some(ProposedRecord {
                indicator: Indicator::BloodPressure,
                fields: vec![field_none("systolic"), field_none("diastolic")],
                time: ctx.time_hint.clone(),
                quote,
            }),
            true,
        );
    }

    if let Some((sys, dia)) = pair {
        let sys_field = if MmHg::try_from(sys.value).is_ok()
            && let Some(v) = number_to_json(sys)
            && let Some(q) = slice_from_bytes(ctx.orig, sys.start_byte, sys.end_byte)
        {
            field_some("systolic", v, q)
        } else {
            field_none("systolic")
        };
        let dia_field = if MmHg::try_from(dia.value).is_ok()
            && let Some(v) = number_to_json(dia)
            && let Some(q) = slice_from_bytes(ctx.orig, dia.start_byte, dia.end_byte)
        {
            field_some("diastolic", v, q)
        } else {
            field_none("diastolic")
        };
        return (
            Some(ProposedRecord {
                indicator: Indicator::BloodPressure,
                fields: vec![sys_field, dia_field],
                time: ctx.time_hint.clone(),
                quote,
            }),
            false,
        );
    }

    if has_keyword {
        let search = find_unique_number(
            ctx.tokens,
            ctx.start,
            ctx.end,
            ctx.anchor_start,
            ctx.time_mask,
            Indicator::BloodPressure,
        );
        let sys_field = if !search.ambiguous
            && let Some(sys) = search.chosen
            && MmHg::try_from(sys.value).is_ok()
            && let Some(v) = number_to_json(sys)
            && let Some(q) = slice_from_bytes(ctx.orig, sys.start_byte, sys.end_byte)
        {
            field_some("systolic", v, q)
        } else {
            field_none("systolic")
        };
        return (
            Some(ProposedRecord {
                indicator: Indicator::BloodPressure,
                fields: vec![sys_field, field_none("diastolic")],
                time: ctx.time_hint.clone(),
                quote,
            }),
            search.ambiguous,
        );
    }

    (None, false)
}

fn extract_single_numeric_record(
    ctx: &SegmentCtx<'_>,
    indicator: Indicator,
    field_key: &'static str,
) -> (Option<ProposedRecord>, bool) {
    let Some(quote) = slice_tokens(ctx.orig, ctx.tokens, ctx.start, ctx.end) else {
        return (None, false);
    };
    let mut field = field_none(field_key);
    let mut ambiguous = ctx.has_conflict;

    if !ctx.has_conflict {
        let search = find_unique_number(
            ctx.tokens,
            ctx.start,
            ctx.end,
            ctx.anchor_start,
            ctx.time_mask,
            indicator,
        );
        ambiguous = search.ambiguous;
        if let Some(num) = search.chosen {
            let valid = match indicator {
                Indicator::HeartRate => Bpm::try_from(num.value).is_ok(),
                Indicator::BodyTemperature => Celsius::try_from(num.value).is_ok(),
                Indicator::Glucose => MgDl::try_from(num.value).is_ok(),
                Indicator::OxygenInBlood => Percent::try_from(num.value).is_ok(),
                Indicator::Mood => {
                    num.value.fract() == 0.0
                        && (0.0..=10.0).contains(&num.value)
                        && num
                            .int_value
                            .and_then(|u| u8::try_from(u).ok())
                            .is_some_and(|u| Score::try_from(u).is_ok())
                }
                _ => false,
            };
            if valid
                && let Some(val) = match indicator {
                    Indicator::Mood => num
                        .int_value
                        .and_then(|u| u8::try_from(u).ok())
                        .map(|u| json!(u)),
                    _ => number_to_json(num),
                }
                && let Some(q) = slice_from_bytes(ctx.orig, num.start_byte, num.end_byte)
            {
                field = field_some(field_key, val, q);
            }
        }
    }

    (
        Some(ProposedRecord {
            indicator,
            fields: vec![field],
            time: ctx.time_hint.clone(),
            quote,
        }),
        ambiguous,
    )
}

fn is_negation_start_at(tokens: &[Token], idx: usize, end: usize) -> bool {
    if let Some(w) = word_at(tokens, idx)
        && matches!(
            w,
            "no" | "ni"
                | "sin"
                | "tampoco"
                | "ningun"
                | "ninguna"
                | "dont"
                | "doesnt"
                | "didnt"
                | "isnt"
                | "arent"
                | "havent"
                | "wasnt"
                | "couldnt"
                | "not"
                | "without"
                | "never"
                | "nor"
                | "neither"
        )
    {
        return true;
    }
    idx + 1 < end
        && (starts_with_words(tokens, idx, &["ya", "no"])
            || starts_with_words(tokens, idx, &["nada", "de"]))
}

fn is_safe_introducer_before_anchor(w: &str) -> bool {
    matches!(
        w,
        "no" | "ni"
            | "sin"
            | "ya"
            | "tampoco"
            | "ningun"
            | "ninguna"
            | "nada"
            | "de"
            | "en"
            | "el"
            | "la"
            | "los"
            | "las"
            | "un"
            | "una"
            | "me"
            | "te"
            | "se"
            | "mi"
            | "mis"
            | "my"
            | "the"
            | "a"
            | "an"
            | "i"
            | "im"
            | "ive"
            | "dont"
            | "doesnt"
            | "didnt"
            | "isnt"
            | "arent"
            | "havent"
            | "wasnt"
            | "couldnt"
            | "not"
            | "without"
            | "never"
            | "nor"
            | "neither"
            | "tengo"
            | "siento"
            | "estoy"
            | "medi"
            | "tome"
            | "registre"
            | "chequee"
            | "have"
            | "feel"
            | "felt"
            | "measured"
            | "checked"
            | "took"
    )
}

/// Retrocede desde el ancla `anchor_start` del segmento `pos` (cuando `pos > 0`)
/// para incluir prefijos de negación locales ("sin dolor", "no me duele...", "no medí...").
fn pull_back_negation_boundary(
    tokens: &[Token],
    min_bound: usize,
    anchor_start: usize,
    time_mask: &TimeMask,
) -> usize {
    let mut k = anchor_start;
    let mut earliest_neg: Option<usize> = None;
    while k > min_bound {
        let prev = k - 1;
        if time_mask.is_masked(prev) {
            break;
        }
        let Some(w) = word_at(tokens, prev) else {
            break;
        };
        if !is_safe_introducer_before_anchor(w) {
            break;
        }
        if is_negation_start_at(tokens, prev, anchor_start) {
            earliest_neg = Some(prev);
        }
        k = prev;
    }
    earliest_neg.unwrap_or(anchor_start)
}

fn try_pure_sleep_duration_continuation(
    orig: &str,
    tokens: &[Token],
    c_start: usize,
    c_end: usize,
    time_mask: &TimeMask,
) -> Option<(f64, Value, String)> {
    let mut idx = c_start;
    if idx < c_end
        && matches!(
            word_at(tokens, idx),
            Some("unos" | "unas" | "como" | "durante" | "for" | "about" | "around" | "de")
        )
    {
        idx += 1;
    }
    let (val, is_int, num_end, valid) = parse_sleep_hours_at(tokens, idx, c_end, idx, time_mask)?;
    if !valid || !val.is_finite() || val <= 0.0 || Hours::try_from(val).is_err() {
        return None;
    }
    // Exige que sea duración explícita en horas (no un número pelado) y que consuma toda la cláusula.
    let consumed_tokens = num_end.saturating_sub(idx);
    let has_hour_token = (idx..num_end).any(|k| word_at(tokens, k).is_some_and(is_hour_unit))
        || (consumed_tokens >= 2
            && (starts_with_words(tokens, idx, &["media", "hora"])
                || starts_with_words(tokens, idx, &["half", "hour"])
                || starts_with_words(tokens, idx, &["half", "an", "hour"])));
    if !has_hour_token || num_end != c_end {
        return None;
    }
    let val_quote = slice_tokens(orig, tokens, idx, num_end)?;
    let jv = if let Some(iv) = is_int {
        Value::from(iv)
    } else {
        Number::from_f64(val).map(Value::Number)?
    };
    Some((val, jv, val_quote))
}

fn process_clause(
    orig: &str,
    tokens: &[Token],
    clause: ClauseSpan,
    carried_time: Option<&MatchedTime>,
    out: &mut ProposalOut,
) -> Option<MatchedTime> {
    let c_start = clause.start;
    let c_end = clause.end;
    let time_matches = find_time_hints(orig, tokens, c_start, c_end);
    let time_mask = TimeMask::from_matches(c_start, c_end, &time_matches);

    // Si toda la cláusula es solo una expresión temporal (p. ej. "ayer, dormí mal")
    let has_non_time_word_or_num = (c_start..c_end).any(|i| {
        !time_mask.is_masked(i)
            && tokens.get(i).is_some_and(|t| {
                matches!(
                    t.kind,
                    TokenKind::Word | TokenKind::Number | TokenKind::InvalidNumber
                )
            })
    });
    if !has_non_time_word_or_num {
        out.last_was_open_sleep = false;
        return time_matches.first().cloned();
    }

    // Buscar anclas sin enmascaramiento temporal
    let mut raw_anchors: Vec<RawAnchor> = Vec::new();
    let mut i = c_start;
    while i < c_end {
        if time_mask.is_masked(i) {
            i += 1;
            continue;
        }
        if let Some(a) = detect_anchor(tokens, i, c_end)
            && !time_mask.any_masked(a.start, a.end)
        {
            raw_anchors.push(a);
            i = a.end.max(i + 1);
        } else {
            i += 1;
        }
    }

    // Continuación inequívoca de sueño: "<dormí mal>, <5 horas>" sin otros verbos ni cambio de tiempo.
    if raw_anchors.is_empty()
        && clause.after_comma
        && time_matches.is_empty()
        && carried_time.is_none()
        && out.last_was_open_sleep
        && let Some(prev) = out.records.last_mut()
        && prev.indicator == Indicator::Sleep
        && prev
            .fields
            .iter()
            .any(|f| f.key == "hours" && f.value.is_none())
        && let Some((_, jv, val_quote)) =
            try_pure_sleep_duration_continuation(orig, tokens, c_start, c_end, &time_mask)
    {
        for f in &mut prev.fields {
            if f.key == "hours" {
                *f = field_some("hours", jv.clone(), val_quote.clone());
            }
        }
        if let Some(&prev_start) = out.record_start_bytes.last()
            && let Some(end_tk) = tokens.get(c_end.saturating_sub(1))
            && let Some(merged_quote) = slice_from_bytes(orig, prev_start, end_tk.end)
        {
            prev.quote = merged_quote;
        }
        out.last_was_open_sleep = false;
        return None;
    }

    if raw_anchors.is_empty() {
        out.last_was_open_sleep = false;
        if let Some(q) = slice_tokens(orig, tokens, c_start, c_end) {
            out.uninterpreted.push(q);
        }
        return None;
    }

    // Agrupar anclas contiguas del mismo indicador cuando pertenecen a la misma mención,
    // o marcar conflicto si se trata de una repetición/corrección explícita.
    let mut groups: Vec<AnchorGroup> = Vec::new();
    let mut prev_anchor_end = c_start;
    for a in raw_anchors {
        let can_merge_same = if let Some(last) = groups.last_mut()
            && last.kind == a.kind
        {
            last.anchor_end = a.end;
            if !a.is_keyword {
                if has_correction_between(tokens, prev_anchor_end, a.start) {
                    last.has_conflict = true;
                }
                true
            } else if has_correction_between(tokens, prev_anchor_end, a.start) {
                last.has_conflict = true;
                true
            } else {
                match a.kind {
                    AnchorKind::Symptom => match (last.primary_zone, a.zone) {
                        (None, Some(z)) => {
                            last.primary_zone = Some(z);
                            true
                        }
                        (Some(_), None) => true,
                        (Some(z1), Some(z2)) if z1 == z2 => true,
                        (None, None) => true,
                        (Some(_), Some(_)) => false,
                    },
                    AnchorKind::Sleep => true,
                    _ => {
                        // Dos palabras clave del mismo indicador numérico en la misma cláusula
                        last.has_conflict = true;
                        true
                    }
                }
            }
        } else {
            false
        };

        if !can_merge_same {
            groups.push(AnchorGroup {
                kind: a.kind,
                anchor_start: a.start,
                anchor_end: a.end,
                primary_zone: a.zone,
                has_conflict: false,
            });
        }
        prev_anchor_end = a.end;
    }

    // Calcular límites de cada segmento, retrocediendo por negaciones locales entre anclas.
    let group_count = groups.len();
    let mut seg_starts = Vec::with_capacity(group_count);
    for (pos, g) in groups.iter().enumerate() {
        if pos == 0 {
            seg_starts.push(c_start);
        } else {
            let prev_anchor_end = groups.get(pos - 1).map_or(c_start, |prev| prev.anchor_end);
            let pulled =
                pull_back_negation_boundary(tokens, prev_anchor_end, g.anchor_start, &time_mask);
            seg_starts.push(pulled);
        }
    }

    let mut emitted_open_sleep = false;
    for (pos, g) in groups.iter().enumerate() {
        let seg_start = seg_starts.get(pos).copied().unwrap_or(c_start);
        let seg_end = seg_starts.get(pos + 1).copied().unwrap_or(c_end);
        if seg_start >= seg_end {
            continue;
        }

        let local_time = time_matches
            .iter()
            .find(|tm| tm.start_idx >= seg_start && tm.end_idx <= seg_end)
            .or_else(|| {
                if pos == 0 {
                    time_matches
                        .first()
                        .filter(|tm| tm.end_idx <= seg_end)
                        .or(carried_time)
                } else {
                    None
                }
            });
        let time_hint = match local_time {
            Some(tm) => TimeHint::Ambiguous {
                quote: tm.quote.clone(),
            },
            None => TimeHint::Unspecified,
        };

        // Negación local por segmento (para todas las clases; excepción solo en Sleep bien/pude/nada).
        let seg_words: Vec<&str> = (seg_start..seg_end)
            .filter(|&k| !time_mask.is_masked(k))
            .filter_map(|k| word_at(tokens, k))
            .collect();
        let sleep_exception =
            g.kind == AnchorKind::Sleep && is_negated_good_sleep(tokens, seg_start, seg_end);
        if !sleep_exception && is_negation_cue_in_words(&seg_words) {
            if let Some(q) = slice_tokens(orig, tokens, seg_start, seg_end) {
                out.negated.push(q);
            }
            emitted_open_sleep = false;
            continue;
        }

        let ctx = SegmentCtx {
            orig,
            tokens,
            start: seg_start,
            end: seg_end,
            anchor_start: g.anchor_start,
            time_mask: &time_mask,
            time_hint,
            has_conflict: g.has_conflict,
        };
        let (rec, ambiguous) = match g.kind {
            AnchorKind::HeartRate => {
                extract_single_numeric_record(&ctx, Indicator::HeartRate, "bpm")
            }
            AnchorKind::BloodPressure => {
                let has_kw =
                    (seg_start..seg_end).any(|k| is_bp_keyword_at(tokens, k, seg_end).is_some());
                extract_bp_record(&ctx, has_kw)
            }
            AnchorKind::BodyTemperature => {
                extract_single_numeric_record(&ctx, Indicator::BodyTemperature, "celsius")
            }
            AnchorKind::Glucose => extract_single_numeric_record(&ctx, Indicator::Glucose, "mg_dl"),
            AnchorKind::OxygenInBlood => {
                extract_single_numeric_record(&ctx, Indicator::OxygenInBlood, "percentage")
            }
            AnchorKind::Mood => extract_single_numeric_record(&ctx, Indicator::Mood, "score"),
            AnchorKind::Sleep => extract_sleep_record(&ctx),
            AnchorKind::Symptom => extract_symptom_record(&ctx),
        };

        if let Some(r) = rec {
            emitted_open_sleep = r.indicator == Indicator::Sleep
                && r.fields
                    .iter()
                    .any(|f| f.key == "hours" && f.value.is_none())
                && !ambiguous;
            if ambiguous && let Some(q) = slice_tokens(orig, tokens, seg_start, seg_end) {
                out.uninterpreted.push(q);
            }
            let start_b = tokens.get(seg_start).map_or(0, |t| t.start);
            out.records.push(r);
            out.record_start_bytes.push(start_b);
        } else if let Some(q) = slice_tokens(orig, tokens, seg_start, seg_end) {
            emitted_open_sleep = false;
            out.uninterpreted.push(q);
        }
    }

    out.last_was_open_sleep = emitted_open_sleep && group_count == 1;
    None
}

pub(super) fn interpret_within_limit(text: &str) -> Proposal {
    let trimmed = text.trim();
    let tokens = tokenize(text);
    let clauses = split_clauses(&tokens);

    let mut out = ProposalOut::default();
    let mut carried_time: Option<MatchedTime> = None;

    for clause in clauses {
        let next_carried = process_clause(text, &tokens, clause, carried_time.as_ref(), &mut out);
        carried_time = next_carried;
    }

    if let Some(leftover_time) = carried_time {
        out.uninterpreted.push(leftover_time.quote);
    }

    // Defensa en profundidad: todo registro completo debe pasar Measurement::validate
    for rec in &mut out.records {
        if rec.fields.iter().all(|f| f.value.is_some())
            && Measurement::validate(rec.indicator, &rec.value(), None).is_err()
        {
            for f in &mut rec.fields {
                f.value = None;
                f.quote = None;
            }
        }
    }

    if out.records.is_empty() && out.negated.is_empty() {
        out.uninterpreted = vec![trimmed.to_owned()];
    }

    Proposal {
        schema_version: PROPOSAL_SCHEMA_VERSION,
        interpreter: InterpreterKind::Rules,
        interpreter_version: RULES_INTERPRETER_VERSION.to_owned(),
        records: out.records,
        negated: out.negated,
        uninterpreted: out.uninterpreted,
    }
}
