//! Segmentación de cláusulas, resolución de negaciones y extracción conservadora de registros.

use super::{
    RULES_INTERPRETER_VERSION,
    lexicon::{
        Token, TokenKind, UnitKind, has_post_unit_unknown_tail, is_bp_keyword_at,
        is_general_symptom_zone_word, is_hour_unit, is_minute_unit, is_negated_good_sleep,
        is_negation_word, is_sleep_word, is_symptom_cue, is_unknown_unit_after_number,
        match_body_zone_phrase, match_unit_at, n1_n2_bp_plausible, parse_body_zone,
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
    anchor_end: usize,
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
    last_clause_rec: Option<(Indicator, usize, usize)>,
    pending_correction_from: Option<(Indicator, usize, usize, Option<usize>)>,
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
            | "nunca"
            | "jamas"
            | "dont"
            | "doesnt"
            | "didnt"
            | "isnt"
            | "arent"
            | "havent"
            | "hasnt"
            | "hadnt"
            | "wasnt"
            | "werent"
            | "couldnt"
            | "cant"
            | "cannot"
            | "wont"
            | "wouldnt"
            | "shouldnt"
            | "mustnt"
            | "not"
            | "never"
            | "neither"
            | "nor"
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

fn is_safe_attribution_word(w: &str) -> bool {
    matches!(
        w,
        "a" | "al"
            | "de"
            | "del"
            | "en"
            | "con"
            | "por"
            | "para"
            | "como"
            | "unos"
            | "unas"
            | "un"
            | "una"
            | "el"
            | "la"
            | "lo"
            | "los"
            | "las"
            | "es"
            | "fue"
            | "era"
            | "esta"
            | "estuvo"
            | "estan"
            | "sido"
            | "hay"
            | "tiene"
            | "tengo"
            | "tenia"
            | "tuve"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "at"
            | "of"
            | "in"
            | "on"
            | "around"
            | "about"
            | "like"
            | "for"
            | "to"
            | "with"
            | "as"
            | "the"
            | "an"
            | "had"
            | "have"
            | "got"
            | "aproximadamente"
            | "approx"
            | "approximately"
            | "casi"
            | "cerca"
            | "alrededor"
            | "solo"
            | "solamente"
            | "only"
            | "just"
            | "nivel"
            | "level"
            | "intensidad"
            | "intensity"
            | "valor"
            | "value"
            | "score"
            | "puntuacion"
            | "rating"
            | "grado"
            | "grade"
            | "scale"
            | "escala"
            | "hoy"
            | "today"
            | "ahora"
            | "now"
            | "actual"
            | "actualmente"
            | "currently"
            | "muy"
            | "bastante"
            | "algo"
            | "mucho"
            | "very"
            | "quite"
            | "pretty"
            | "reposo"
            | "resting"
            | "rest"
            | "ayunas"
            | "ayuno"
            | "fasting"
            | "postprandial"
            | "leve"
            | "ligero"
            | "ligera"
            | "suave"
            | "moderado"
            | "moderada"
            | "fuerte"
            | "intenso"
            | "intensa"
            | "severo"
            | "severa"
            | "agudo"
            | "aguda"
            | "mild"
            | "slight"
            | "moderate"
            | "severe"
            | "strong"
            | "intense"
            | "baja"
            | "alta"
            | "lower"
            | "upper"
            | "izquierda"
            | "izquierdo"
            | "derecha"
            | "derecho"
            | "left"
            | "right"
            | "bien"
            | "mal"
            | "regular"
            | "bueno"
            | "buena"
            | "malo"
            | "mala"
            | "terrible"
            | "horrible"
            | "genial"
            | "excelente"
            | "poco"
            | "nada"
            | "pesimo"
            | "pesima"
            | "good"
            | "well"
            | "bad"
            | "badly"
            | "poor"
            | "poorly"
            | "fair"
            | "ok"
            | "okay"
            | "great"
            | "fine"
            | "awful"
    )
}

fn is_tight_pre_anchor_connector(w: &str) -> bool {
    matches!(
        w,
        "de" | "del"
            | "of"
            | "en"
            | "in"
            | "para"
            | "for"
            | "con"
            | "with"
            | "por"
            | "el"
            | "la"
            | "los"
            | "las"
            | "mi"
            | "my"
            | "the"
            | "su"
            | "his"
            | "her"
    )
}

fn is_safe_pre_value_word(w: &str) -> bool {
    is_tight_pre_anchor_connector(w)
        || matches!(
            w,
            "unos"
                | "unas"
                | "como"
                | "about"
                | "around"
                | "durante"
                | "cerca"
                | "alrededor"
                | "aproximadamente"
                | "approx"
                | "approximately"
                | "tengo"
                | "tuve"
                | "tenia"
                | "siento"
                | "estoy"
                | "i"
                | "im"
                | "ive"
                | "had"
                | "have"
                | "got"
                | "a"
                | "an"
                | "un"
                | "una"
                | "al"
                | "hoy"
                | "today"
                | "anoche"
                | "ayer"
        )
}

#[allow(clippy::too_many_arguments)]
fn attribution_link_ok(
    tokens: &[Token],
    seg_start: usize,
    anchor_start: usize,
    anchor_end: usize,
    val_start: usize,
    val_end: usize,
    explicit_unit: bool,
    time_mask: &TimeMask,
) -> bool {
    // 1. El valor está contenido en el propio ancla (p. ej., "72 bpm", "36.8°C", "120/80").
    if val_start >= anchor_start && val_end <= anchor_end {
        return true;
    }

    // 2. Valor posterior al ancla: solo palabras de enlace seguras en anchor_end..val_start.
    if val_start >= anchor_end {
        for k in anchor_end..val_start {
            if time_mask.is_masked(k) {
                continue;
            }
            let Some(tk) = tokens.get(k) else {
                return false;
            };
            match tk.kind {
                TokenKind::Word => {
                    if !is_safe_attribution_word(tk.norm.as_str()) {
                        return false;
                    }
                }
                TokenKind::Other if matches!(tk.norm.as_str(), ":" | "=") => {}
                _ => return false,
            }
        }
        return true;
    }

    // 3. Valor previo al ancla: exige unidad explícita, conectores mínimos entre val_end..anchor_start
    //    y ausencia de verbos/predicados ajenos antes del valor (p. ej. "trabajé 5 horas antes de dormir").
    if !explicit_unit || val_end > anchor_start {
        return false;
    }
    for k in val_end..anchor_start {
        if time_mask.is_masked(k) {
            continue;
        }
        if let Some(w) = word_at(tokens, k) {
            if !is_tight_pre_anchor_connector(w) {
                return false;
            }
        } else {
            return false;
        }
    }
    for k in seg_start..val_start {
        if time_mask.is_masked(k) {
            continue;
        }
        if let Some(w) = word_at(tokens, k) {
            if !is_safe_pre_value_word(w) {
                return false;
            }
        } else if tokens
            .get(k)
            .is_some_and(|tk| !(tk.kind == TokenKind::Other && tk.norm == ":"))
        {
            return false;
        }
    }
    true
}

struct NumberSearchResult {
    chosen: Option<ParsedNumber>,
    ambiguous: bool,
}

#[allow(clippy::too_many_arguments)]
fn find_unique_number(
    orig: &str,
    tokens: &[Token],
    seg_start: usize,
    seg_end: usize,
    anchor_start: usize,
    anchor_end: usize,
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
                        Some((unit, u_end)) => {
                            let bad_tail = has_post_unit_unknown_tail(tokens, u_end, seg_end);
                            (
                                unit.matches_indicator(target),
                                unit.matches_indicator(target) && !bad_tail,
                                u_end,
                            )
                        }
                        None => {
                            let unknown_u =
                                is_unknown_unit_after_number(orig, tokens, num.end_idx, seg_end);
                            (false, !unknown_u, num.end_idx)
                        }
                    };

                let has_slash_10 = allow_fraction_10
                    && (i..num.end_idx)
                        .any(|k| tokens.get(k).is_some_and(|tk| tk.kind == TokenKind::Slash));
                let explicit_unit = has_matching_unit || has_slash_10;

                let bad_prefix =
                    i > seg_start && is_non_clinical_prefix_word(word_at(tokens, i - 1));
                let local_ok = attribution_link_ok(
                    tokens,
                    seg_start,
                    anchor_start,
                    anchor_end,
                    i,
                    next_i,
                    explicit_unit,
                    time_mask,
                );

                if compatible && !bad_prefix && local_ok {
                    let is_range_or_alt_sep = tokens
                        .get(next_i)
                        .is_some_and(|tk| tk.kind == TokenKind::Hyphen)
                        || matches!(
                            word_at(tokens, next_i),
                            Some("o" | "or" | "vs" | "y" | "and")
                        );
                    if next_i + 1 < seg_end
                        && is_range_or_alt_sep
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

#[allow(clippy::too_many_arguments)]
fn parse_sleep_hours_at(
    orig: &str,
    tokens: &[Token],
    seg_start: usize,
    idx: usize,
    end: usize,
    anchor_start: usize,
    anchor_end: usize,
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
        let ok = !has_post_unit_unknown_tail(tokens, idx + 2, end)
            && attribution_link_ok(
                tokens,
                seg_start,
                anchor_start,
                anchor_end,
                idx,
                idx + 2,
                true,
                time_mask,
            );
        return Some((if ok { 0.5 } else { f64::NAN }, None, idx + 2, ok));
    }
    if starts_with_words(tokens, idx, &["half", "an", "hour"])
        && idx + 3 <= end
        && !time_mask.any_masked(idx, idx + 3)
    {
        let ok = !has_post_unit_unknown_tail(tokens, idx + 3, end)
            && attribution_link_ok(
                tokens,
                seg_start,
                anchor_start,
                anchor_end,
                idx,
                idx + 3,
                true,
                time_mask,
            );
        return Some((if ok { 0.5 } else { f64::NAN }, None, idx + 3, ok));
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

            if num_end < end {
                if let Some((unit, u_end)) = match_unit_at(tokens, num_end, end) {
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
                } else if is_unknown_unit_after_number(orig, tokens, num_end, end) {
                    return Some((f64::NAN, None, num_end, false));
                }
            }

            let bad_tail = if has_hour_unit {
                has_post_unit_unknown_tail(tokens, num_end, end)
            } else {
                false
            };
            let bad_prefix = idx > 0 && is_non_clinical_prefix_word(word_at(tokens, idx - 1));
            let local_ok = attribution_link_ok(
                tokens,
                seg_start,
                anchor_start,
                anchor_end,
                idx,
                num_end,
                has_hour_unit,
                time_mask,
            );
            if bad_prefix || bad_tail || !local_ok {
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
            if let Some((val, is_int, next_i, valid)) = parse_sleep_hours_at(
                ctx.orig,
                ctx.tokens,
                ctx.start,
                i,
                ctx.end,
                ctx.anchor_start,
                ctx.anchor_end,
                ctx.time_mask,
            ) {
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
            ctx.orig,
            ctx.tokens,
            ctx.start,
            ctx.end,
            ctx.anchor_start,
            ctx.anchor_end,
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

#[allow(clippy::too_many_arguments)]
fn find_bp_pairs(
    orig: &str,
    tokens: &[Token],
    start: usize,
    end: usize,
    anchor_start: usize,
    anchor_end: usize,
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
                    let (unit_ok, next_after_unit) = match match_unit_at(tokens, n2.end_idx, end) {
                        Some((u, u_end)) => (u == UnitKind::BloodPressure, u_end),
                        None => (
                            !is_unknown_unit_after_number(orig, tokens, n2.end_idx, end),
                            n2.end_idx,
                        ),
                    };
                    let local_ok = attribution_link_ok(
                        tokens,
                        start,
                        anchor_start,
                        anchor_end,
                        i,
                        next_after_unit,
                        unit_ok && n2.end_idx < next_after_unit,
                        time_mask,
                    );
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
        ctx.orig,
        ctx.tokens,
        ctx.start,
        ctx.end,
        ctx.anchor_start,
        ctx.anchor_end,
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
            ctx.orig,
            ctx.tokens,
            ctx.start,
            ctx.end,
            ctx.anchor_start,
            ctx.anchor_end,
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
            ctx.orig,
            ctx.tokens,
            ctx.start,
            ctx.end,
            ctx.anchor_start,
            ctx.anchor_end,
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
                | "nunca"
                | "jamas"
                | "dont"
                | "doesnt"
                | "didnt"
                | "isnt"
                | "arent"
                | "havent"
                | "hasnt"
                | "hadnt"
                | "wasnt"
                | "werent"
                | "couldnt"
                | "cant"
                | "cannot"
                | "wont"
                | "wouldnt"
                | "shouldnt"
                | "mustnt"
                | "aint"
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
            | "nunca"
            | "jamas"
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
            | "to"
            | "able"
            | "i"
            | "im"
            | "ive"
            | "dont"
            | "doesnt"
            | "didnt"
            | "isnt"
            | "arent"
            | "havent"
            | "hasnt"
            | "hadnt"
            | "wasnt"
            | "werent"
            | "couldnt"
            | "cant"
            | "cannot"
            | "wont"
            | "wouldnt"
            | "shouldnt"
            | "mustnt"
            | "aint"
            | "not"
            | "without"
            | "never"
            | "nor"
            | "neither"
            | "can"
            | "will"
            | "would"
            | "should"
            | "could"
            | "did"
            | "do"
            | "does"
            | "tengo"
            | "siento"
            | "estoy"
            | "medi"
            | "tome"
            | "registre"
            | "chequee"
            | "medir"
            | "tomar"
            | "registrar"
            | "chequear"
            | "pude"
            | "puedo"
            | "have"
            | "has"
            | "had"
            | "feel"
            | "felt"
            | "measure"
            | "measured"
            | "check"
            | "checked"
            | "take"
            | "took"
            | "record"
            | "log"
            | "get"
    )
}

fn is_measurement_action_word(w: &str) -> bool {
    matches!(
        w,
        "medir"
            | "medido"
            | "medida"
            | "tomar"
            | "tomado"
            | "tomada"
            | "registrar"
            | "registrado"
            | "registrada"
            | "confirmar"
            | "confirmado"
            | "confirmada"
            | "chequear"
            | "chequeado"
            | "measure"
            | "measured"
            | "take"
            | "taken"
            | "check"
            | "checked"
            | "record"
            | "recorded"
            | "log"
            | "logged"
            | "detectado"
            | "detectada"
            | "detected"
            | "presente"
            | "present"
    )
}

fn is_post_anchor_absence_predicate(w: &str) -> bool {
    matches!(
        w,
        "gone" | "anymore" | "desaparecio" | "desaparecieron" | "subsided" | "resolved"
    )
}

fn is_segment_negated(
    tokens: &[Token],
    seg_start: usize,
    seg_end: usize,
    anchor_end: usize,
    time_mask: &TimeMask,
) -> bool {
    // 1. Negación previa o interna al ancla dentro del segmento (p. ej. "I can't measure pulse 72",
    //    "no medí pulso 72", "sin dolor", "my head doesn't hurt").
    for k in seg_start..anchor_end {
        if time_mask.is_masked(k) {
            continue;
        }
        if is_negation_start_at(tokens, k, anchor_end) {
            return true;
        }
    }

    // 2. Negación o resolución pospuesta que modifica al ancla/medición (p. ej. "pulso no medido",
    //    "headache is gone", "fiebre no", pero NO "dormí bien no tuve pesadillas" ni "dormí bien sin interrupciones").
    for k in anchor_end..seg_end {
        if time_mask.is_masked(k) {
            continue;
        }
        if word_at(tokens, k).is_some_and(is_post_anchor_absence_predicate) {
            return true;
        }
        if is_negation_start_at(tokens, k, seg_end) {
            let neg_len = if k + 1 < seg_end
                && (starts_with_words(tokens, k, &["ya", "no"])
                    || starts_with_words(tokens, k, &["nada", "de"]))
            {
                2
            } else {
                1
            };
            let after = k + neg_len;
            if after >= seg_end
                && matches!(
                    word_at(tokens, k),
                    Some("no" | "not" | "never" | "nunca" | "tampoco")
                )
            {
                return true;
            }
            if word_at(tokens, after).is_some_and(is_measurement_action_word) {
                return true;
            }
        }
    }

    false
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
    let (val, is_int, num_end, valid) =
        parse_sleep_hours_at(orig, tokens, idx, idx, c_end, idx, idx, time_mask)?;
    if !valid || !val.is_finite() || Hours::try_from(val).is_err() {
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

impl AnchorKind {
    const fn to_indicator(self) -> Indicator {
        match self {
            Self::HeartRate => Indicator::HeartRate,
            Self::BloodPressure => Indicator::BloodPressure,
            Self::BodyTemperature => Indicator::BodyTemperature,
            Self::Glucose => Indicator::Glucose,
            Self::OxygenInBlood => Indicator::OxygenInBlood,
            Self::Mood => Indicator::Mood,
            Self::Sleep => Indicator::Sleep,
            Self::Symptom => Indicator::Symptom,
        }
    }
}

fn starts_with_correction_at(tokens: &[Token], start: usize, end: usize) -> Option<usize> {
    if start >= end {
        return None;
    }
    if word_at(tokens, start).is_some_and(is_correction_word) {
        return Some(start + 1);
    }
    if start + 1 < end
        && (starts_with_words(tokens, start, &["mejor", "dicho"])
            || starts_with_words(tokens, start, &["quise", "decir"])
            || starts_with_words(tokens, start, &["quiero", "decir"])
            || starts_with_words(tokens, start, &["o", "sea"])
            || starts_with_words(tokens, start, &["en", "realidad"])
            || starts_with_words(tokens, start, &["i", "mean"]))
    {
        return Some(start + 2);
    }
    None
}

fn clear_record_values(rec: &mut ProposedRecord) {
    for f in &mut rec.fields {
        if rec.indicator == Indicator::Symptom && f.key == "description" {
            continue;
        }
        f.value = None;
        f.quote = None;
    }
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
        out.last_clause_rec = None;
        out.pending_correction_from = None;
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

    let corr_after = starts_with_correction_at(tokens, c_start, c_end);
    let clause_end_byte = tokens
        .get(c_end.saturating_sub(1))
        .map_or(orig.len(), |tk| tk.end);

    // Continuación inequívoca de sueño: "<dormí mal>, <5 horas>" sin otros verbos ni cambio de tiempo.
    if raw_anchors.is_empty()
        && clause.after_comma
        && corr_after.is_none()
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
        out.pending_correction_from = None;
        return None;
    }

    if raw_anchors.is_empty() {
        out.last_was_open_sleep = false;
        if let Some(after_corr_idx) = corr_after
            && let Some((prev_ind, rec_idx, prev_start_b)) = out.last_clause_rec
        {
            if let Some(prev_rec) = out.records.get_mut(rec_idx) {
                clear_record_values(prev_rec);
            }
            if after_corr_idx >= c_end {
                // "pulso 72, perdón, pulso 82": guardar inicio para combinarlo con la cláusula siguiente.
                out.pending_correction_from =
                    Some((prev_ind, rec_idx, prev_start_b, Some(clause_end_byte)));
            } else if let Some(combined) = slice_from_bytes(orig, prev_start_b, clause_end_byte) {
                out.uninterpreted.push(combined);
                out.pending_correction_from = None;
            }
            out.last_clause_rec = None;
            return None;
        }
        out.last_clause_rec = None;
        out.pending_correction_from = None;
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

    // Corrección contigua entre cláusulas del mismo indicador ("pulso 72, perdón pulso 82" / "pulso 72, perdón, pulso 82"):
    // consolida a un solo registro ambiguo (value: None) y el span completo en uninterpreted.
    if let Some(first_group) = groups.first() {
        let first_ind = first_group.kind.to_indicator();
        let active_corr = out
            .pending_correction_from
            .take()
            .map(|(ind, idx, start_b, _)| (ind, idx, start_b))
            .filter(|(ind, _, _)| *ind == first_ind)
            .or_else(|| {
                if corr_after.is_some() {
                    out.last_clause_rec.filter(|(ind, _, _)| *ind == first_ind)
                } else {
                    None
                }
            });
        if let Some((_, rec_idx, prev_start_b)) = active_corr {
            if let Some(prev_rec) = out.records.get_mut(rec_idx) {
                clear_record_values(prev_rec);
            }
            if let Some(combined) = slice_from_bytes(orig, prev_start_b, clause_end_byte) {
                out.uninterpreted.push(combined);
            }
            out.last_was_open_sleep = false;
            out.last_clause_rec = None;
            return None;
        }
    }
    if let Some((_, _, prev_start_b, Some(corr_end_b))) = out.pending_correction_from.take()
        && let Some(prev_uninterp) = slice_from_bytes(orig, prev_start_b, corr_end_b)
    {
        out.uninterpreted.push(prev_uninterp);
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
    let mut last_emitted_in_clause: Option<(Indicator, usize, usize)> = None;
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

        // Negación local posicional por segmento (para todas las clases; excepción solo en "no dormí bien").
        let sleep_exception =
            g.kind == AnchorKind::Sleep && is_negated_good_sleep(tokens, seg_start, seg_end);
        if !sleep_exception
            && is_segment_negated(tokens, seg_start, seg_end, g.anchor_end, &time_mask)
        {
            if let Some(q) = slice_tokens(orig, tokens, seg_start, seg_end) {
                out.negated.push(q);
            }
            emitted_open_sleep = false;
            last_emitted_in_clause = None;
            continue;
        }

        let ctx = SegmentCtx {
            orig,
            tokens,
            start: seg_start,
            end: seg_end,
            anchor_start: g.anchor_start,
            anchor_end: g.anchor_end,
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
            let rec_idx = out.records.len();
            let ind = r.indicator;
            out.records.push(r);
            out.record_start_bytes.push(start_b);
            last_emitted_in_clause = if group_count == 1 && !ambiguous {
                Some((ind, rec_idx, start_b))
            } else {
                None
            };
        } else if let Some(q) = slice_tokens(orig, tokens, seg_start, seg_end) {
            emitted_open_sleep = false;
            last_emitted_in_clause = None;
            out.uninterpreted.push(q);
        }
    }

    out.last_was_open_sleep = emitted_open_sleep && group_count == 1;
    out.last_clause_rec = last_emitted_in_clause;
    None
}

/// Comprueba que la cita del registro corresponda a una sola cláusula clínica:
/// sin `;`, `.` ni conectores de cambio de cláusula, y solo acepta `,` si la capa 1 la
/// interpreta como una única mención completa sin negaciones ni restos no interpretados
/// (p. ej. continuación de duración de sueño `"dormí mal, 5 horas"`).
pub(crate) fn is_single_clause_quote(quote: &str) -> bool {
    let trimmed = quote.trim();
    if trimmed.is_empty() {
        return false;
    }
    let tokens = tokenize(trimmed);
    if tokens.is_empty() {
        return false;
    }
    let mut has_comma = false;
    for (idx, tk) in tokens.iter().enumerate() {
        match tk.kind {
            TokenKind::ClauseSep => {
                if tk.norm == "," {
                    has_comma = true;
                } else {
                    return false;
                }
            }
            TokenKind::Word => {
                if is_clause_break_word(tk.norm.as_str()) {
                    return false;
                }
                if idx + 1 < tokens.len()
                    && (starts_with_words(&tokens, idx, &["y", "luego"])
                        || starts_with_words(&tokens, idx, &["and", "then"]))
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    if has_comma {
        let clauses = split_clauses(&tokens);
        if clauses.len() > 2 {
            return false;
        }
        let sub = interpret_within_limit(trimmed);
        return sub.records.len() == 1 && sub.negated.is_empty() && sub.uninterpreted.is_empty();
    }
    true
}

/// Comprueba si la ocurrencia `occ` de una cita en `input` está negada por su contexto
/// inmediato en la misma cláusula (antes o inmediatamente después) o en la propia mención.
pub(crate) fn occurrence_is_negated_in_input(
    input: &str,
    input_tokens: &[Token],
    occ: &super::lexicon::BoundedOccurrence,
    indicator: Indicator,
) -> bool {
    let clauses = split_clauses(input_tokens);
    let Some(clause) = clauses
        .iter()
        .copied()
        .find(|c| occ.first_tok >= c.start && occ.last_tok < c.end)
    else {
        return false;
    };

    let time_matches = find_time_hints(input, input_tokens, clause.start, clause.end);
    let time_mask = TimeMask::from_matches(clause.start, clause.end, &time_matches);

    // 1. Negación previa en la misma cláusula antes de occ.first_tok, deteniéndose ante un corte
    //    coordinado o ante otro ancla de distinto indicador ("no me duele pulso 72", "no tengo fiebre y pulso 72").
    let mut prefix_min = clause.start;
    let mut a_idx = clause.start;
    while a_idx < occ.first_tok {
        if time_mask.is_masked(a_idx) {
            a_idx += 1;
            continue;
        }
        if let Some(a) = detect_anchor(input_tokens, a_idx, occ.first_tok)
            && !time_mask.any_masked(a.start, a.end)
        {
            if a.kind.to_indicator() != indicator {
                prefix_min = prefix_min.max(a.end);
            }
            a_idx = a.end.max(a_idx + 1);
        } else {
            a_idx += 1;
        }
    }

    let mut k = occ.first_tok;
    while k > prefix_min {
        k -= 1;
        if time_mask.is_masked(k) {
            continue;
        }
        if is_negation_start_at(input_tokens, k, occ.first_tok) {
            return true;
        }
        if let Some(w) = word_at(input_tokens, k)
            && matches!(w, "y" | "e" | "and" | "or" | "o")
        {
            break;
        }
    }

    // 2. Negación o resolución pospuesta inmediatamente tras occ.last_tok que modifica al propio indicador.
    let mut j = occ.last_tok + 1;
    while j < clause.end {
        if time_mask.is_masked(j) {
            j += 1;
            continue;
        }
        if input_tokens
            .get(j)
            .is_some_and(|tk| tk.kind == TokenKind::Other && tk.norm == ":")
        {
            j += 1;
            continue;
        }
        if word_at(input_tokens, j).is_some_and(is_post_anchor_absence_predicate) {
            return true;
        }
        if is_negation_start_at(input_tokens, j, clause.end) {
            let neg_len = if j + 1 < clause.end
                && (starts_with_words(input_tokens, j, &["ya", "no"])
                    || starts_with_words(input_tokens, j, &["nada", "de"]))
            {
                2
            } else {
                1
            };
            let after = j + neg_len;
            if after >= clause.end
                && matches!(
                    word_at(input_tokens, j),
                    Some("no" | "not" | "never" | "nunca" | "tampoco")
                )
            {
                return true;
            }
            if word_at(input_tokens, after).is_some_and(is_measurement_action_word) {
                return true;
            }
        }
        if matches!(
            word_at(input_tokens, j),
            Some("es" | "esta" | "fue" | "se" | "is" | "was" | "are" | "were")
        ) && j + 1 < clause.end
            && (word_at(input_tokens, j + 1).is_some_and(is_post_anchor_absence_predicate)
                || (is_negation_start_at(input_tokens, j + 1, clause.end)
                    && word_at(input_tokens, j + 2).is_some_and(is_measurement_action_word)))
        {
            return true;
        }
        break;
    }

    // 3. Si el indicador no es Sleep ("no dormí bien"), una negación explícita dentro de la
    //    propia cita cuando no contiene otro predicado también cuenta.
    if indicator != Indicator::Sleep
        && let Some(sub_slice) = input.get(occ.start_byte..occ.end_byte)
    {
        let sub_tokens = tokenize(sub_slice);
        if sub_tokens
            .iter()
            .any(|tk| tk.kind == TokenKind::Word && is_negation_word(tk.norm.as_str()))
        {
            let mut matching_anchors = Vec::new();
            let mut sa = 0;
            while sa < sub_tokens.len() {
                if let Some(a) = detect_anchor(&sub_tokens, sa, sub_tokens.len()) {
                    if a.kind.to_indicator() == indicator {
                        matching_anchors.push(a);
                    }
                    sa = a.end.max(sa + 1);
                } else {
                    sa += 1;
                }
            }
            if let (Some(_first_a), Some(last_a)) =
                (matching_anchors.first(), matching_anchors.last())
            {
                let tm = TimeMask::from_matches(
                    0,
                    sub_tokens.len(),
                    &find_time_hints(sub_slice, &sub_tokens, 0, sub_tokens.len()),
                );
                if is_segment_negated(&sub_tokens, 0, sub_tokens.len(), last_a.end, &tm) {
                    return true;
                }
            }
        }
    }

    false
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

    if let Some((_, _, prev_start_b, Some(corr_end_b))) = out.pending_correction_from.take()
        && let Some(prev_uninterp) = slice_from_bytes(text, prev_start_b, corr_end_b)
    {
        out.uninterpreted.push(prev_uninterp);
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
