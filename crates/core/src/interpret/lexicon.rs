//! Tokenización, normalización y léxico público del intérprete determinista.

use super::{
    MAX_INPUT_CHARS,
    numbers::{NumberScan, parse_simple_number, scan_number},
};
use crate::catalog::{BodyZone, Indicator, SleepQuality};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TokenKind {
    Word,
    Number,
    InvalidNumber,
    Slash,
    Percent,
    Hyphen,
    ClauseSep,
    Other,
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub(super) norm: String,
    pub(super) kind: TokenKind,
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnitKind {
    HeartRate,
    BloodPressure,
    TemperatureCelsius,
    Glucose,
    SleepHours,
    SleepMinutes,
    OxygenPercent,
    ScaleTen,
    SpaceRadiation,
    DistanceKm,
    NutritionKcal,
    Incompatible,
}

impl UnitKind {
    pub(super) const fn matches_indicator(self, indicator: Indicator) -> bool {
        matches!(
            (self, indicator),
            (Self::HeartRate, Indicator::HeartRate)
                | (Self::BloodPressure, Indicator::BloodPressure)
                | (Self::TemperatureCelsius, Indicator::BodyTemperature)
                | (Self::Glucose, Indicator::Glucose)
                | (Self::SleepHours, Indicator::Sleep)
                | (Self::OxygenPercent, Indicator::OxygenInBlood)
                | (Self::ScaleTen, Indicator::Mood | Indicator::Symptom)
                | (Self::SpaceRadiation, Indicator::SpaceRadiation)
                | (Self::DistanceKm, Indicator::DistanceFromEarth)
                | (Self::NutritionKcal, Indicator::Nutrition)
        )
    }
}

fn is_combining_mark(ch: char) -> bool {
    ('\u{0300}'..='\u{036F}').contains(&ch)
        || ('\u{1AB0}'..='\u{1AFF}').contains(&ch)
        || ('\u{1DC0}'..='\u{1DFF}').contains(&ch)
        || ('\u{FE20}'..='\u{FE2F}').contains(&ch)
}

fn normalize_latin_char(ch: char) -> char {
    match ch {
        'A'..='Z' => ch.to_ascii_lowercase(),
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' | 'Å' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        'µ' | 'μ' => 'u',
        _ => ch,
    }
}

fn is_apostrophe(ch: char) -> bool {
    matches!(ch, '\'' | '’')
}

fn is_sign_char(ch: char) -> bool {
    matches!(ch, '-' | '–' | '—' | '−' | '+')
}

pub(crate) fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let Some(&(byte_idx, ch)) = chars.get(i) else {
            break;
        };
        let ch_end = byte_idx + ch.len_utf8();

        if ch.is_whitespace() {
            if matches!(ch, '\n' | '\r') {
                tokens.push(Token {
                    norm: "\n".to_owned(),
                    kind: TokenKind::ClauseSep,
                    start: byte_idx,
                    end: ch_end,
                });
            }
            i += 1;
            continue;
        }

        if is_combining_mark(ch) {
            if let Some(last) = tokens.last_mut() {
                last.end = ch_end;
            }
            i += 1;
            continue;
        }

        // Detecta signos pegados al inicio de un número (p. ej. "-6", "+3", "−1.5")
        // siempre que no estén pegados directamente a un número/palabra anterior.
        if is_sign_char(ch)
            && chars.get(i + 1).is_some_and(|&(_, c)| c.is_ascii_digit())
            && tokens.last().is_none_or(|prev| {
                prev.end < byte_idx || matches!(prev.kind, TokenKind::ClauseSep | TokenKind::Other)
            })
        {
            let start = byte_idx;
            let mut end = ch_end;
            let mut norm = String::new();
            norm.push(ch);
            i += 1;
            while let Some(&(next_byte, next_ch)) = chars.get(i) {
                if next_ch.is_ascii_digit()
                    || (matches!(next_ch, '.' | ',')
                        && chars.get(i + 1).is_some_and(|&(_, c)| c.is_ascii_digit()))
                {
                    norm.push(next_ch);
                    end = next_byte + next_ch.len_utf8();
                    i += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                norm,
                kind: TokenKind::InvalidNumber,
                start,
                end,
            });
            continue;
        }

        if ch.is_ascii_digit() {
            let start = byte_idx;
            let mut end = ch_end;
            let mut norm = String::new();
            norm.push(ch);
            i += 1;

            while let Some(&(next_byte, next_ch)) = chars.get(i) {
                if next_ch.is_ascii_digit() {
                    norm.push(next_ch);
                    end = next_byte + next_ch.len_utf8();
                    i += 1;
                } else {
                    break;
                }
            }

            let mut invalid = false;
            if let (Some(&(_, sep_ch)), Some(&(_, after_ch))) = (chars.get(i), chars.get(i + 1))
                && matches!(sep_ch, '.' | ',')
                && after_ch.is_ascii_digit()
            {
                norm.push('.');
                i += 1;
                while let Some(&(next_byte, next_ch)) = chars.get(i) {
                    if next_ch.is_ascii_digit() {
                        norm.push(next_ch);
                        end = next_byte + next_ch.len_utf8();
                        i += 1;
                    } else {
                        break;
                    }
                }
                // Si sigue otro separador decimal con dígitos (p. ej. "36.8.1"), invalida todo.
                while let (Some(&(_, extra_sep)), Some(&(_, extra_dig))) =
                    (chars.get(i), chars.get(i + 1))
                    && matches!(extra_sep, '.' | ',')
                    && extra_dig.is_ascii_digit()
                {
                    invalid = true;
                    norm.push(extra_sep);
                    i += 1;
                    while let Some(&(next_byte, next_ch)) = chars.get(i) {
                        if next_ch.is_ascii_digit() {
                            norm.push(next_ch);
                            end = next_byte + next_ch.len_utf8();
                            i += 1;
                        } else {
                            break;
                        }
                    }
                }
            }

            tokens.push(Token {
                norm,
                kind: if invalid {
                    TokenKind::InvalidNumber
                } else {
                    TokenKind::Number
                },
                start,
                end,
            });
            continue;
        }

        let norm_ch = normalize_latin_char(ch);
        // Reconoce "a.m." y "p.m." como un único token Word("am" | "pm") para no partir cláusulas.
        if matches!(norm_ch, 'a' | 'p')
            && !tokens
                .last()
                .is_some_and(|t| t.end == byte_idx && t.kind == TokenKind::Word)
            && chars.get(i + 1).is_some_and(|&(_, c)| c == '.')
            && chars
                .get(i + 2)
                .is_some_and(|&(_, c)| normalize_latin_char(c) == 'm')
            && chars.get(i + 3).is_some_and(|&(_, c)| c == '.')
            && !chars
                .get(i + 4)
                .is_some_and(|&(_, c)| normalize_latin_char(c).is_alphabetic())
            && let Some(&(dot_end_byte, dot_ch)) = chars.get(i + 3)
        {
            let mut am_pm = String::new();
            am_pm.push(norm_ch);
            am_pm.push('m');
            tokens.push(Token {
                norm: am_pm,
                kind: TokenKind::Word,
                start: byte_idx,
                end: dot_end_byte + dot_ch.len_utf8(),
            });
            i += 4;
            continue;
        }

        if norm_ch.is_alphabetic() {
            let start = byte_idx;
            let mut end = ch_end;
            let mut norm = String::new();
            if norm_ch.is_ascii_lowercase() {
                norm.push(norm_ch);
            } else {
                for lc in norm_ch.to_lowercase() {
                    norm.push(lc);
                }
            }
            i += 1;

            while let Some(&(next_byte, next_ch)) = chars.get(i) {
                let next_end = next_byte + next_ch.len_utf8();
                if is_combining_mark(next_ch) {
                    end = next_end;
                    i += 1;
                    continue;
                }
                let next_norm = normalize_latin_char(next_ch);
                if next_norm.is_alphabetic() {
                    if next_norm.is_ascii_lowercase() {
                        norm.push(next_norm);
                    } else {
                        for lc in next_norm.to_lowercase() {
                            norm.push(lc);
                        }
                    }
                    end = next_end;
                    i += 1;
                    continue;
                }
                if is_apostrophe(next_ch)
                    && let Some(&(_, after_ch)) = chars.get(i + 1)
                    && normalize_latin_char(after_ch).is_alphabetic()
                {
                    end = next_end;
                    i += 1;
                    continue;
                }
                if (next_ch == '2' || next_ch == '₂') && matches!(norm.as_str(), "spo" | "o" | "co")
                {
                    norm.push('2');
                    end = next_end;
                    i += 1;
                    continue;
                }
                break;
            }

            tokens.push(Token {
                norm,
                kind: TokenKind::Word,
                start,
                end,
            });
            continue;
        }

        let kind = match ch {
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '-' | '–' | '—' | '−' => TokenKind::Hyphen,
            '.' | ',' | ';' | '!' | '?' => TokenKind::ClauseSep,
            _ => TokenKind::Other,
        };
        tokens.push(Token {
            norm: ch.to_string(),
            kind,
            start: byte_idx,
            end: ch_end,
        });
        i += 1;
    }

    tokens
}

pub(super) fn slice_tokens(
    orig: &str,
    tokens: &[Token],
    start_idx: usize,
    end_idx: usize,
) -> Option<String> {
    if start_idx >= end_idx {
        return None;
    }
    let first = tokens.get(start_idx)?;
    let last = tokens.get(end_idx.checked_sub(1)?)?;
    if first.start >= last.end {
        return None;
    }
    let raw = orig.get(first.start..last.end)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub(super) fn slice_from_bytes(orig: &str, start_byte: usize, end_byte: usize) -> Option<String> {
    if start_byte >= end_byte {
        return None;
    }
    let raw = orig.get(start_byte..end_byte)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub(super) fn truncate_chars(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => s.get(..byte_idx).unwrap_or(s).to_owned(),
        None => s.to_owned(),
    }
}

pub(super) fn word_at(tokens: &[Token], idx: usize) -> Option<&str> {
    tokens
        .get(idx)
        .filter(|t| t.kind == TokenKind::Word)
        .map(|t| t.norm.as_str())
}

pub(super) fn starts_with_words(tokens: &[Token], idx: usize, words: &[&str]) -> bool {
    for (offset, expected) in words.iter().enumerate() {
        if word_at(tokens, idx + offset) != Some(*expected) {
            return false;
        }
    }
    true
}

pub(super) fn is_sleep_word(w: &str) -> bool {
    matches!(
        w,
        "dormi"
            | "dormir"
            | "dormido"
            | "sueno"
            | "slept"
            | "sleep"
            | "sleeping"
            | "descanse"
            | "descanso"
    )
}

pub(super) fn is_hour_unit(w: &str) -> bool {
    matches!(w, "h" | "hr" | "hrs" | "hora" | "horas" | "hour" | "hours")
}

pub(super) fn is_minute_unit(w: &str) -> bool {
    matches!(
        w,
        "min" | "mins" | "minuto" | "minutos" | "minute" | "minutes"
    )
}

pub(super) fn is_symptom_cue(w: &str) -> bool {
    matches!(
        w,
        "duele"
            | "duelen"
            | "dolia"
            | "dolor"
            | "molestia"
            | "molesta"
            | "pain"
            | "hurt"
            | "hurts"
            | "hurting"
            | "ache"
            | "aches"
            | "aching"
            | "sore"
            | "headache"
            | "backache"
            | "migraine"
            | "migrana"
            | "jaqueca"
            | "cefalea"
            | "cefalalgia"
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
            | "sintoma"
            | "sintomas"
            | "symptom"
            | "symptoms"
    )
}

pub(super) fn is_general_symptom_zone_word(w: &str) -> bool {
    matches!(
        w,
        "nausea"
            | "nauseas"
            | "mareo"
            | "mareos"
            | "dizzy"
            | "dizziness"
            | "vertigo"
            | "fatiga"
            | "fatigue"
            | "cansancio"
    )
}

pub(super) fn is_bp_keyword_at(tokens: &[Token], idx: usize, end: usize) -> Option<usize> {
    if idx + 1 < end
        && (starts_with_words(tokens, idx, &["blood", "pressure"])
            || starts_with_words(tokens, idx, &["presion", "arterial"])
            || starts_with_words(tokens, idx, &["tension", "arterial"]))
    {
        return Some(2);
    }
    if matches!(
        word_at(tokens, idx),
        Some("presion" | "tension" | "bp" | "pressure")
    ) {
        return Some(1);
    }
    None
}

pub(super) fn n1_n2_bp_plausible(sys: f64, dia: f64) -> bool {
    sys >= 30.0 && dia >= 20.0 && dia != 10.0
}

pub(super) fn match_body_zone_phrase(
    tokens: &[Token],
    idx: usize,
    end: usize,
) -> Option<(BodyZone, usize)> {
    if idx >= end {
        return None;
    }

    let side_at = |k: usize| -> Option<bool> {
        match word_at(tokens, k)? {
            "left" | "izquierdo" | "izquierda" | "izq" => Some(true),
            "right" | "derecho" | "derecha" | "der" | "dcho" | "dcha" => Some(false),
            _ => None,
        }
    };
    let limb_at = |k: usize| -> Option<u8> {
        match word_at(tokens, k)? {
            "arm" | "shoulder" | "elbow" | "forearm" | "brazo" | "hombro" | "codo"
            | "antebrazo" => Some(0),
            "hand" | "finger" | "fingers" | "thumb" | "wrist" | "palm" | "mano" | "dedo"
            | "dedos" | "pulgar" | "muneca" | "palma" => Some(1),
            "leg" | "knee" | "thigh" | "calf" | "hip" | "shin" | "pierna" | "rodilla" | "muslo"
            | "gemelo" | "pantorrilla" | "cadera" => Some(2),
            "foot" | "ankle" | "heel" | "toe" | "toes" | "pie" | "tobillo" | "talon" | "planta" => {
                Some(3)
            }
            _ => None,
        }
    };

    if idx + 1 < end {
        let matched = match (side_at(idx), limb_at(idx + 1)) {
            (Some(left), Some(limb)) => Some((left, limb)),
            _ => match (limb_at(idx), side_at(idx + 1)) {
                (Some(limb), Some(left)) => Some((left, limb)),
                _ => None,
            },
        };
        if let Some((left, limb)) = matched {
            let zone = match (left, limb) {
                (true, 0) => BodyZone::LeftArm,
                (false, 0) => BodyZone::RightArm,
                (true, 1) => BodyZone::LeftHand,
                (false, 1) => BodyZone::RightHand,
                (true, 2) => BodyZone::LeftLeg,
                (false, 2) => BodyZone::RightLeg,
                (true, 3) => BodyZone::LeftFoot,
                _ => BodyZone::RightFoot,
            };
            return Some((zone, idx + 2));
        }
    }

    if idx + 1 < end
        && (starts_with_words(tokens, idx, &["lower", "back"])
            || starts_with_words(tokens, idx, &["upper", "back"])
            || starts_with_words(tokens, idx, &["espalda", "baja"])
            || starts_with_words(tokens, idx, &["espalda", "alta"]))
    {
        return Some((BodyZone::Back, idx + 2));
    }

    let w = word_at(tokens, idx)?;
    let zone = match w {
        "cabeza" | "head" | "headache" | "migraine" | "migrana" | "jaqueca" | "cefalea"
        | "cefalalgia" | "craneo" | "frente" | "forehead" | "sien" | "sienes" | "temple"
        | "temples" | "nuca" | "skull" => Some(BodyZone::Head),
        "cuello" | "neck" | "garganta" | "throat" | "cervical" | "cervicales" => {
            Some(BodyZone::Neck)
        }
        "pecho" | "chest" | "torax" | "thorax" | "esternon" | "sternum" | "costilla"
        | "costillas" | "rib" | "ribs" | "ribcage" => Some(BodyZone::Chest),
        "abdomen" | "abdominal" | "estomago" | "stomach" | "barriga" | "belly" | "tummy"
        | "panza" | "tripa" | "vientre" | "gut" => Some(BodyZone::Abdomen),
        "espalda" | "back" | "backache" | "lumbar" | "lumbares" | "lomo" | "columna" | "spine"
        | "dorsal" | "cintura" => Some(BodyZone::Back),
        "ojo" | "ojos" | "eye" | "eyes" | "oido" | "oidos" | "oreja" | "ear" | "ears" | "muela"
        | "muelas" | "diente" | "tooth" | "piel" | "skin" => Some(BodyZone::Other),
        _ => None,
    }?;
    Some((zone, idx + 1))
}

pub(super) fn parse_body_zone(
    tokens: &[Token],
    start: usize,
    end: usize,
) -> Option<(BodyZone, usize, usize)> {
    let mut i = start;
    while i < end {
        if let Some((zone, z_end)) = match_body_zone_phrase(tokens, i, end) {
            return Some((zone, i, z_end));
        }
        i += 1;
    }
    None
}

/// Reconoce una unidad inmediatamente posterior a un número en `tokens[idx..end]`.
pub(super) fn match_unit_at(tokens: &[Token], idx: usize, end: usize) -> Option<(UnitKind, usize)> {
    let tk = tokens.get(idx)?;
    if idx >= end {
        return None;
    }

    if tk.kind == TokenKind::Percent {
        return Some((UnitKind::OxygenPercent, idx + 1));
    }

    if tk.norm == "℃" {
        return Some((UnitKind::TemperatureCelsius, idx + 1));
    }
    if matches!(tk.norm.as_str(), "℉" | "K" | "‰") {
        return Some((UnitKind::Incompatible, idx + 1));
    }

    // Grados con símbolo ° / º
    if matches!(tk.norm.as_str(), "°" | "º") {
        if idx + 1 < end
            && let Some(w) = word_at(tokens, idx + 1)
        {
            return match w {
                "c" | "celsius" | "centigrados" | "centigrado" => {
                    Some((UnitKind::TemperatureCelsius, idx + 2))
                }
                "f" | "fahrenheit" | "farenheit" | "k" | "kelvin" | "kelvins" | "r" | "rankine"
                | "reaumur" => Some((UnitKind::Incompatible, idx + 2)),
                _ => Some((UnitKind::Incompatible, idx + 1)),
            };
        }
        return Some((UnitKind::Incompatible, idx + 1));
    }

    // Barra seguida de letras tras número ("/min", "/h", "/kg"): unidad o tasa no admitida.
    if tk.kind == TokenKind::Slash && idx + 1 < end && word_at(tokens, idx + 1).is_some() {
        return Some((UnitKind::Incompatible, idx + 2));
    }

    let w = word_at(tokens, idx)?;

    // Unidades compuestas con barra: "<w1>/<w2>" (p. ej. "mg/dL", "mmol/L", "uSv/h", "km/h")
    if idx + 2 < end
        && tokens
            .get(idx + 1)
            .is_some_and(|s| s.kind == TokenKind::Slash)
        && let Some(w2) = word_at(tokens, idx + 2)
    {
        let kind = match (w, w2) {
            ("mg", "dl") => UnitKind::Glucose,
            ("usv", "h" | "hr" | "hora" | "hour") => UnitKind::SpaceRadiation,
            _ => UnitKind::Incompatible,
        };
        return Some((kind, idx + 3));
    }

    if w == "por" && idx + 1 < end && word_at(tokens, idx + 1) == Some("ciento") {
        return Some((UnitKind::OxygenPercent, idx + 2));
    }
    if matches!(w, "porciento" | "percent" | "pct") {
        return Some((UnitKind::OxygenPercent, idx + 1));
    }

    if matches!(w, "bpm" | "lpm") {
        return Some((UnitKind::HeartRate, idx + 1));
    }
    if matches!(w, "pulsaciones" | "latidos") {
        if idx + 2 < end && starts_with_words(tokens, idx + 1, &["por", "minuto"]) {
            return Some((UnitKind::HeartRate, idx + 3));
        }
        return Some((UnitKind::HeartRate, idx + 1));
    }
    if idx + 2 < end && starts_with_words(tokens, idx, &["beats", "per", "minute"]) {
        return Some((UnitKind::HeartRate, idx + 3));
    }

    if w == "mmhg" {
        return Some((UnitKind::BloodPressure, idx + 1));
    }
    if w == "mm" && idx + 1 < end && word_at(tokens, idx + 1) == Some("hg") {
        return Some((UnitKind::BloodPressure, idx + 2));
    }

    if matches!(w, "grados" | "degrees") {
        if idx + 1 < end
            && let Some(next) = word_at(tokens, idx + 1)
        {
            return match next {
                "c" | "celsius" | "centigrados" | "centigrado" => {
                    Some((UnitKind::TemperatureCelsius, idx + 2))
                }
                "f" | "fahrenheit" | "farenheit" | "k" | "kelvin" | "kelvins" | "r" | "rankine"
                | "reaumur" => Some((UnitKind::Incompatible, idx + 2)),
                _ => Some((UnitKind::TemperatureCelsius, idx + 1)),
            };
        }
        return Some((UnitKind::TemperatureCelsius, idx + 1));
    }
    if matches!(w, "deg" | "degs") {
        if idx + 1 < end
            && word_at(tokens, idx + 1).is_some_and(|next| {
                matches!(
                    next,
                    "f" | "fahrenheit"
                        | "farenheit"
                        | "k"
                        | "kelvin"
                        | "kelvins"
                        | "r"
                        | "rankine"
                        | "reaumur"
                        | "c"
                        | "celsius"
                        | "centigrados"
                        | "centigrado"
                )
            })
        {
            return Some((UnitKind::Incompatible, idx + 2));
        }
        return Some((UnitKind::Incompatible, idx + 1));
    }
    if matches!(w, "celsius" | "centigrados" | "centigrado" | "c") {
        return Some((UnitKind::TemperatureCelsius, idx + 1));
    }
    if matches!(
        w,
        "f" | "fahrenheit"
            | "farenheit"
            | "k"
            | "kelvin"
            | "kelvins"
            | "degf"
            | "degc"
            | "degk"
            | "degr"
    ) {
        return Some((UnitKind::Incompatible, idx + 1));
    }

    if w == "mgdl" {
        return Some((UnitKind::Glucose, idx + 1));
    }

    if is_hour_unit(w) {
        return Some((UnitKind::SleepHours, idx + 1));
    }
    if is_minute_unit(w) {
        return Some((UnitKind::SleepMinutes, idx + 1));
    }

    if matches!(w, "usv" | "microsieverts" | "microsievert") {
        return Some((UnitKind::SpaceRadiation, idx + 1));
    }
    if matches!(
        w,
        "km" | "kilometro" | "kilometros" | "kilometer" | "kilometers"
    ) {
        return Some((UnitKind::DistanceKm, idx + 1));
    }
    if matches!(w, "kcal" | "calorias" | "caloria" | "calories" | "calorie") {
        return Some((UnitKind::NutritionKcal, idx + 1));
    }

    if matches!(
        w,
        "mmol"
            | "mmoll"
            | "millimol"
            | "millimoles"
            | "milimol"
            | "milimoles"
            | "umol"
            | "nmol"
            | "pmol"
            | "mol"
            | "moles"
            | "meq"
            | "osm"
            | "mosm"
            | "osmol"
            | "mosmol"
            | "mg"
            | "ug"
            | "mcg"
            | "ng"
            | "pg"
            | "kg"
            | "g"
            | "gr"
            | "gramo"
            | "gramos"
            | "gram"
            | "grams"
            | "kilogramo"
            | "kilogramos"
            | "kilogram"
            | "kilograms"
            | "kilo"
            | "kilos"
            | "ml"
            | "dl"
            | "cl"
            | "fl"
            | "cc"
            | "l"
            | "lt"
            | "litros"
            | "litro"
            | "liters"
            | "liter"
            | "cm"
            | "mm"
            | "um"
            | "nm"
            | "m"
            | "metro"
            | "metros"
            | "meter"
            | "meters"
            | "milla"
            | "millas"
            | "mile"
            | "miles"
            | "mi"
            | "pie"
            | "pies"
            | "foot"
            | "feet"
            | "ft"
            | "pulgada"
            | "pulgadas"
            | "inch"
            | "inches"
            | "yarda"
            | "yardas"
            | "yard"
            | "yards"
            | "yd"
            | "lb"
            | "lbs"
            | "libra"
            | "libras"
            | "pound"
            | "pounds"
            | "oz"
            | "onza"
            | "onzas"
            | "ounce"
            | "ounces"
            | "cal"
            | "kpa"
            | "pa"
            | "hpa"
            | "torr"
            | "atm"
            | "bar"
            | "mbar"
            | "psi"
            | "hz"
            | "khz"
            | "mhz"
            | "ghz"
            | "hertz"
            | "watt"
            | "watts"
            | "kw"
            | "mw"
            | "joule"
            | "joules"
            | "kj"
            | "rpm"
            | "mph"
            | "kmh"
            | "fps"
            | "db"
            | "gy"
            | "mgy"
            | "sv"
            | "msv"
            | "rem"
            | "rad"
            | "bq"
            | "ci"
            | "mci"
            | "ppm"
            | "ppb"
            | "ui"
            | "iu"
            | "unidad"
            | "unidades"
            | "unit"
            | "units"
            | "rankine"
            | "reaumur"
            | "ms"
            | "msec"
            | "milisegundo"
            | "milisegundos"
            | "millisecond"
            | "milliseconds"
            | "segundo"
            | "segundos"
            | "second"
            | "seconds"
            | "seg"
            | "s"
            | "dia"
            | "dias"
            | "day"
            | "days"
            | "semana"
            | "semanas"
            | "week"
            | "weeks"
            | "mes"
            | "meses"
            | "month"
            | "months"
            | "ano"
            | "anos"
            | "year"
            | "years"
            | "ejercicio"
            | "ejercicios"
            | "exercise"
            | "exercises"
            | "flexion"
            | "flexiones"
            | "pushup"
            | "pushups"
            | "sentadilla"
            | "sentadillas"
            | "squat"
            | "squats"
            | "abdominal"
            | "abdominales"
            | "situp"
            | "situps"
            | "rep"
            | "reps"
            | "repeticion"
            | "repeticiones"
            | "serie"
            | "series"
            | "set"
            | "sets"
            | "ronda"
            | "rondas"
            | "round"
            | "rounds"
            | "vuelta"
            | "vueltas"
            | "lap"
            | "laps"
            | "paso"
            | "pasos"
            | "step"
            | "steps"
            | "vez"
            | "veces"
            | "time"
            | "times"
            | "pastilla"
            | "pastillas"
            | "pill"
            | "pills"
            | "tableta"
            | "tabletas"
            | "tablet"
            | "tablets"
            | "capsula"
            | "capsulas"
            | "capsule"
            | "capsules"
            | "gota"
            | "gotas"
            | "drop"
            | "drops"
            | "dosis"
            | "dose"
            | "capitulo"
            | "chapter"
            | "pagina"
            | "page"
            | "piso"
            | "floor"
    ) {
        return Some((UnitKind::Incompatible, idx + 1));
    }

    // Cualquier otra abreviatura suelta de 1 letra ASCII que no sea conjunción/preposición ES/EN.
    // "u" solo actúa como conjunción disyuntiva ante "o..." / "ho..." / número.
    if w.len() == 1 && !matches!(w, "y" | "e" | "o" | "a" | "i") {
        let spanish_u_conj = w == "u"
            && idx + 1 < end
            && (tokens
                .get(idx + 1)
                .is_some_and(|n| n.kind == TokenKind::Number)
                || word_at(tokens, idx + 1)
                    .is_some_and(|nw| nw.starts_with('o') || nw.starts_with("ho")));
        if !spanish_u_conj {
            return Some((UnitKind::Incompatible, idx + 1));
        }
    }

    None
}

fn is_closing_punct(tk: &Token) -> bool {
    matches!(
        tk.norm.as_str(),
        ")" | "]" | "}" | "\"" | "'" | "”" | "’" | "»"
    )
}

fn is_allowed_spaced_other_after_number(tk: &Token) -> bool {
    matches!(
        tk.norm.as_str(),
        "(" | ")" | "[" | "]" | "{" | "}" | "\"" | "'" | "“" | "”" | "‘" | "’" | "«" | "»"
    )
}

fn has_unit_mixed_case(raw: &str) -> bool {
    let mut saw_lower = false;
    for ch in raw.chars() {
        if ch.is_ascii_lowercase() {
            saw_lower = true;
        } else if ch.is_ascii_uppercase() && saw_lower {
            return true;
        }
    }
    false
}

pub(super) fn has_post_unit_unknown_tail(tokens: &[Token], check_idx: usize, end: usize) -> bool {
    if check_idx >= end {
        return false;
    }
    let attached = check_idx
        .checked_sub(1)
        .and_then(|p| tokens.get(p))
        .zip(tokens.get(check_idx))
        .is_some_and(|(prev, next)| prev.end == next.start);
    if attached
        && tokens
            .get(check_idx)
            .is_some_and(|t| t.kind != TokenKind::ClauseSep && !is_closing_punct(t))
    {
        return true;
    }
    if tokens
        .get(check_idx)
        .is_some_and(|t| t.kind == TokenKind::Slash)
        && word_at(tokens, check_idx + 1).is_some()
    {
        return true;
    }
    false
}
pub(super) fn is_unknown_unit_after_number(
    orig: &str,
    tokens: &[Token],
    idx: usize,
    end: usize,
) -> bool {
    let Some(tk) = tokens.get(idx) else {
        return false;
    };
    if idx >= end {
        return false;
    }

    let attached = idx
        .checked_sub(1)
        .and_then(|p| tokens.get(p))
        .is_some_and(|prev| prev.end == tk.start);

    if attached {
        let is_range_hyphen = tk.kind == TokenKind::Hyphen
            && tokens
                .get(idx + 1)
                .is_some_and(|n| matches!(n.kind, TokenKind::Number | TokenKind::InvalidNumber));
        if tk.kind != TokenKind::ClauseSep && !is_closing_punct(tk) && !is_range_hyphen {
            return true;
        }
    }

    if tk.kind == TokenKind::Other && !is_allowed_spaced_other_after_number(tk) {
        return true;
    }

    if tk.kind == TokenKind::Word
        && !matches!(tk.norm.as_str(), "spo2")
        && let Some(raw) = orig.get(tk.start..tk.end)
        && has_unit_mixed_case(raw)
    {
        return true;
    }

    false
}

pub(super) fn sleep_quality_word(w: &str) -> Option<SleepQuality> {
    match w {
        "mal" | "mala" | "malo" | "pesimo" | "pesima" | "fatal" | "horrible" | "terrible"
        | "poor" | "poorly" | "bad" | "badly" | "terribly" | "awful" | "awfully" | "restless" => {
            Some(SleepQuality::Poor)
        }
        "regular" | "aceptable" | "pasable" | "mediocre" | "fair" | "fairly" | "okay" | "ok"
        | "average" | "alright" | "decent" => Some(SleepQuality::Fair),
        "bien" | "bueno" | "buena" | "genial" | "excelente" | "estupendo" | "perfecto"
        | "profundo" | "profundamente" | "good" | "well" | "great" | "excellent" | "fine"
        | "deeply" | "soundly" => Some(SleepQuality::Good),
        _ => None,
    }
}

fn is_pre_sleep_aux_or_pronoun(w: &str) -> bool {
    matches!(
        w,
        "he" | "ha"
            | "han"
            | "habia"
            | "pude"
            | "pudo"
            | "podido"
            | "podia"
            | "puedo"
            | "poder"
            | "me"
            | "se"
            | "te"
            | "i"
            | "we"
            | "you"
            | "did"
            | "do"
            | "does"
            | "have"
            | "has"
            | "had"
            | "could"
            | "can"
            | "will"
            | "would"
            | "was"
            | "were"
            | "am"
            | "is"
            | "are"
            | "been"
            | "able"
            | "to"
            | "really"
            | "very"
            | "muy"
            | "nada"
            | "at"
            | "all"
            | "a"
            | "para"
    )
}

pub(super) fn is_negated_good_sleep(tokens: &[Token], start: usize, end: usize) -> bool {
    for s in start..end {
        if !word_at(tokens, s).is_some_and(is_sleep_word) {
            continue;
        }
        let mut found_neg = false;
        let mut has_can = false;
        let mut k = s;
        while k > start {
            let prev = k - 1;
            let Some(w) = word_at(tokens, prev) else {
                break;
            };
            if matches!(
                w,
                "no" | "not"
                    | "didnt"
                    | "dont"
                    | "doesnt"
                    | "havent"
                    | "hasnt"
                    | "hadnt"
                    | "couldnt"
                    | "cant"
                    | "cannot"
                    | "wont"
                    | "wouldnt"
                    | "never"
                    | "nunca"
                    | "tampoco"
            ) {
                found_neg = true;
                if matches!(w, "couldnt" | "cant" | "cannot") {
                    has_can = true;
                }
                break;
            }
            if is_pre_sleep_aux_or_pronoun(w) {
                if matches!(
                    w,
                    "pude"
                        | "pudo"
                        | "podido"
                        | "podia"
                        | "puedo"
                        | "poder"
                        | "could"
                        | "can"
                        | "able"
                ) {
                    has_can = true;
                }
                k = prev;
            } else {
                break;
            }
        }
        if !found_neg {
            continue;
        }

        let mut has_good_or_nada = false;
        let mut m = s + 1;
        while m < end {
            let Some(w) = word_at(tokens, m) else {
                break;
            };
            if matches!(w, "bien" | "well" | "good" | "nada") {
                has_good_or_nada = true;
                break;
            }
            if m + 1 < end && starts_with_words(tokens, m, &["at", "all"]) {
                has_good_or_nada = true;
                break;
            }
            if matches!(w, "muy" | "very" | "really" | "tan" | "so" | "del" | "todo") {
                m += 1;
            } else {
                break;
            }
        }

        if has_can || has_good_or_nada {
            return true;
        }
    }
    false
}

pub(super) fn is_negation_word(w: &str) -> bool {
    matches!(
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
            | "gone"
            | "anymore"
            | "desaparecio"
    )
}

pub(super) fn is_negation_cue_in_words(words: &[&str]) -> bool {
    words.iter().any(|w| is_negation_word(w))
        || words.windows(2).any(|w| {
            matches!(
                w,
                ["ya", "no"]
                    | ["nada", "de"]
                    | ["no", "longer"]
                    | ["no", "more"]
                    | ["me", "quito"]
                    | ["ha", "quitado"]
                    | ["pain", "free"]
            )
        })
}

fn parse_quote_number_with_unit(quote: &str) -> Option<(f64, Option<UnitKind>)> {
    let trimmed = quote.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_INPUT_CHARS {
        return None;
    }
    let tokens = tokenize(trimmed);
    let len = tokens.len();
    if len == 0 {
        return None;
    }

    // Casos especiales de sueño: "media hora", "half hour", "half an hour"
    if len == 2
        && (starts_with_words(&tokens, 0, &["media", "hora"])
            || starts_with_words(&tokens, 0, &["half", "hour"]))
    {
        return Some((0.5, Some(UnitKind::SleepHours)));
    }
    if len == 3 && starts_with_words(&tokens, 0, &["half", "an", "hour"]) {
        return Some((0.5, Some(UnitKind::SleepHours)));
    }

    // Permite sufijo "/10" solo si toda la cita es ese formato; rechaza cualquier otra fracción.
    let num = match scan_number(&tokens, 0, len, true)? {
        NumberScan::Valid(n) => n,
        NumberScan::Rejected { .. } => return None,
    };

    if num.end_idx == len {
        // Detecta si el propio número incluyó "/10"
        let has_slash_10 =
            (0..len).any(|k| tokens.get(k).is_some_and(|tk| tk.kind == TokenKind::Slash));
        let unit = if has_slash_10 {
            Some(UnitKind::ScaleTen)
        } else {
            None
        };
        return Some((num.value, unit));
    }

    // "<entero> horas y media" / "<entero> hours and a half" / "<entero> horas y <mins> minutos"
    if let Some(int_v) = num.int_value
        && word_at(&tokens, num.end_idx).is_some_and(is_hour_unit)
    {
        let u_idx = num.end_idx;
        if u_idx + 1 == len {
            return Some((num.value, Some(UnitKind::SleepHours)));
        }
        if (starts_with_words(&tokens, u_idx + 1, &["y", "media"])
            || starts_with_words(&tokens, u_idx + 1, &["y", "medio"]))
            && u_idx + 3 == len
        {
            let val = format!("{int_v}.5").parse::<f64>().ok()?;
            return Some((val, Some(UnitKind::SleepHours)));
        }
        if starts_with_words(&tokens, u_idx + 1, &["and", "a", "half"]) && u_idx + 4 == len {
            let val = format!("{int_v}.5").parse::<f64>().ok()?;
            return Some((val, Some(UnitKind::SleepHours)));
        }
        if starts_with_words(&tokens, u_idx + 1, &["and", "half"]) && u_idx + 3 == len {
            let val = format!("{int_v}.5").parse::<f64>().ok()?;
            return Some((val, Some(UnitKind::SleepHours)));
        }
        if matches!(word_at(&tokens, u_idx + 1), Some("y" | "and"))
            && let Some(mins) = parse_simple_number(&tokens, u_idx + 2)
            && mins.end_idx + 1 == len
            && word_at(&tokens, mins.end_idx).is_some_and(is_minute_unit)
            && mins
                .int_value
                .is_some_and(|m| m > 0 && m < 60 && m % 3 == 0)
        {
            let val = num.value + mins.value / 60.0;
            return Some((val, Some(UnitKind::SleepHours)));
        }
    }

    // Unidad única al final que consume exactamente el resto de tokens
    if let Some((unit, u_end)) = match_unit_at(&tokens, num.end_idx, len)
        && u_end == len
        && !matches!(unit, UnitKind::SleepMinutes | UnitKind::Incompatible)
    {
        return Some((num.value, Some(unit)));
    }

    None
}

/// Valor numérico si `quote` es ENTERA una expresión numérica (cifras o palabras ES/EN,
/// decimales con coma/punto, "y media"/"and a half", sufijo "/10" opcional y, opcionalmente,
/// la unidad del indicador). `None` si sobra o falta algo.
pub fn quote_number(quote: &str) -> Option<f64> {
    parse_quote_number_with_unit(quote).map(|(v, _)| v)
}

/// Comprueba que la unidad explícita de `quote` (si la hay) sea compatible con `indicator`.
pub(crate) fn quote_unit_compatible(indicator: Indicator, quote: &str) -> bool {
    let Some((_, unit)) = parse_quote_number_with_unit(quote) else {
        return false;
    };
    match unit {
        None => true,
        Some(u) => u.matches_indicator(indicator),
    }
}

/// Zona corporal que nombra la cita `quote`, o `None` si no nombra ninguna o es ambigua.
pub fn quote_zone(quote: &str) -> Option<BodyZone> {
    let trimmed = quote.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_INPUT_CHARS {
        return None;
    }
    let tokens = tokenize(trimmed);
    let len = tokens.len();
    let mut found: Option<BodyZone> = None;
    let mut i = 0;
    while i < len {
        if let Some((zone, next)) = match_body_zone_phrase(&tokens, i, len) {
            if let Some(prev) = found
                && prev != zone
            {
                return None;
            }
            found = Some(zone);
            i = next;
            continue;
        }
        if word_at(&tokens, i).is_some_and(is_general_symptom_zone_word) {
            if let Some(prev) = found
                && prev != BodyZone::Other
            {
                return None;
            }
            found = Some(BodyZone::Other);
        }
        i += 1;
    }
    found
}

/// Calidad de sueño que respalda la cita `quote`, o `None` si no hay respaldo claro o hay conflicto.
pub fn quote_sleep_quality(quote: &str) -> Option<SleepQuality> {
    let trimmed = quote.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_INPUT_CHARS {
        return None;
    }
    let tokens = tokenize(trimmed);
    let len = tokens.len();

    if is_negated_good_sleep(&tokens, 0, len) {
        return Some(SleepQuality::Poor);
    }

    let words: Vec<&str> = (0..len).filter_map(|i| word_at(&tokens, i)).collect();
    if is_negation_cue_in_words(&words) {
        return None;
    }

    let mut found: Option<SleepQuality> = None;
    let mut i = 0;
    while i < len {
        if starts_with_words(&tokens, i, &["mas", "o", "menos"]) && i + 3 <= len {
            if found.is_some_and(|q| q != SleepQuality::Fair) {
                return None;
            }
            found = Some(SleepQuality::Fair);
            i += 3;
            continue;
        }
        if starts_with_words(&tokens, i, &["so", "so"]) && i + 2 <= len {
            if found.is_some_and(|q| q != SleepQuality::Fair) {
                return None;
            }
            found = Some(SleepQuality::Fair);
            i += 2;
            continue;
        }
        if let Some(w) = word_at(&tokens, i)
            && let Some(q) = sleep_quality_word(w)
        {
            if found.is_some_and(|prev| prev != q) {
                return None;
            }
            found = Some(q);
        }
        i += 1;
    }
    found
}

/// Indica si `quote` nombra `indicator` (o un síntoma/zona corporal en el caso de `Symptom`).
pub fn mentions_indicator(indicator: Indicator, quote: &str) -> bool {
    let trimmed = quote.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_INPUT_CHARS {
        return false;
    }
    let tokens = tokenize(trimmed);
    let len = tokens.len();

    for i in 0..len {
        let matched = match indicator {
            Indicator::HeartRate => {
                starts_with_words(&tokens, i, &["heart", "rate"])
                    || starts_with_words(&tokens, i, &["frecuencia", "cardiaca"])
                    || starts_with_words(&tokens, i, &["ritmo", "cardiaco"])
                    || starts_with_words(&tokens, i, &["beats", "per", "minute"])
                    || word_at(&tokens, i).is_some_and(|w| {
                        matches!(
                            w,
                            "pulso"
                                | "pulse"
                                | "hr"
                                | "bpm"
                                | "lpm"
                                | "pulsaciones"
                                | "latidos"
                                | "frecuencia"
                        )
                    })
            }
            Indicator::BloodPressure => {
                is_bp_keyword_at(&tokens, i, len).is_some()
                    || word_at(&tokens, i).is_some_and(|w| w == "mmhg")
                    || (word_at(&tokens, i) == Some("mm") && word_at(&tokens, i + 1) == Some("hg"))
                    || parse_simple_number(&tokens, i).is_some_and(|n1| {
                        n1.end_idx + 1 < len
                            && tokens
                                .get(n1.end_idx)
                                .is_some_and(|tk| tk.kind == TokenKind::Slash)
                            && parse_simple_number(&tokens, n1.end_idx + 1)
                                .is_some_and(|n2| n1_n2_bp_plausible(n1.value, n2.value))
                    })
            }
            Indicator::BodyTemperature => {
                starts_with_words(&tokens, i, &["body", "temperature"])
                    || starts_with_words(&tokens, i, &["body", "temp"])
                    || word_at(&tokens, i).is_some_and(|w| {
                        matches!(
                            w,
                            "temperatura"
                                | "temp"
                                | "fiebre"
                                | "fever"
                                | "febricula"
                                | "celsius"
                                | "centigrados"
                                | "centigrado"
                        )
                    })
                    || tokens.get(i).is_some_and(|tk| tk.norm == "℃")
                    || (tokens
                        .get(i)
                        .is_some_and(|tk| matches!(tk.norm.as_str(), "°" | "º"))
                        && word_at(&tokens, i + 1) == Some("c"))
                    || (matches!(word_at(&tokens, i), Some("grados" | "degrees"))
                        && !matches!(
                            word_at(&tokens, i + 1),
                            Some(
                                "f" | "fahrenheit"
                                    | "farenheit"
                                    | "k"
                                    | "kelvin"
                                    | "kelvins"
                                    | "r"
                                    | "rankine"
                                    | "reaumur"
                            )
                        ))
            }
            Indicator::Glucose => {
                starts_with_words(&tokens, i, &["blood", "glucose"])
                    || starts_with_words(&tokens, i, &["blood", "sugar"])
                    || starts_with_words(&tokens, i, &["azucar", "en", "sangre"])
                    || (word_at(&tokens, i) == Some("mg")
                        && tokens
                            .get(i + 1)
                            .is_some_and(|tk| tk.kind == TokenKind::Slash)
                        && word_at(&tokens, i + 2) == Some("dl"))
                    || word_at(&tokens, i).is_some_and(|w| {
                        matches!(w, "glucosa" | "glucemia" | "glucose" | "azucar" | "mgdl")
                    })
            }
            Indicator::Sleep => word_at(&tokens, i).is_some_and(is_sleep_word),
            Indicator::Mood => {
                starts_with_words(&tokens, i, &["estado", "de", "animo"])
                    || word_at(&tokens, i).is_some_and(|w| matches!(w, "animo" | "mood" | "humor"))
            }
            Indicator::OxygenInBlood => {
                starts_with_words(&tokens, i, &["oxygen", "saturation"])
                    || starts_with_words(&tokens, i, &["blood", "oxygen"])
                    || starts_with_words(&tokens, i, &["saturacion", "de", "oxigeno"])
                    || starts_with_words(&tokens, i, &["oxigeno", "en", "sangre"])
                    || starts_with_words(&tokens, i, &["o2", "sat"])
                    || word_at(&tokens, i).is_some_and(|w| {
                        matches!(
                            w,
                            "saturacion" | "saturation" | "oxigeno" | "oxygen" | "spo2" | "o2"
                        )
                    })
            }
            Indicator::SpaceRadiation => {
                starts_with_words(&tokens, i, &["space", "radiation"])
                    || starts_with_words(&tokens, i, &["radiacion", "espacial"])
                    || word_at(&tokens, i).is_some_and(|w| {
                        matches!(
                            w,
                            "radiacion"
                                | "radiation"
                                | "usv"
                                | "sievert"
                                | "sieverts"
                                | "microsievert"
                                | "microsieverts"
                                | "dosimetro"
                                | "dosimeter"
                        )
                    })
            }
            Indicator::DistanceFromEarth => {
                starts_with_words(&tokens, i, &["distance", "from", "earth"])
                    || starts_with_words(&tokens, i, &["distancia", "a", "la", "tierra"])
                    || starts_with_words(&tokens, i, &["distancia", "desde", "la", "tierra"])
                    || word_at(&tokens, i).is_some_and(|w| {
                        matches!(
                            w,
                            "distancia"
                                | "distance"
                                | "km"
                                | "kilometro"
                                | "kilometros"
                                | "kilometer"
                                | "kilometers"
                        )
                    })
            }
            Indicator::Nutrition => word_at(&tokens, i).is_some_and(|w| {
                matches!(
                    w,
                    "nutricion"
                        | "nutrition"
                        | "kcal"
                        | "calorias"
                        | "caloria"
                        | "calories"
                        | "calorie"
                        | "comida"
                        | "alimento"
                        | "food"
                        | "meal"
                )
            }),
            Indicator::Symptom => {
                word_at(&tokens, i).is_some_and(is_symptom_cue)
                    || match_body_zone_phrase(&tokens, i, len).is_some()
            }
        };
        if matched {
            return true;
        }
    }

    false
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BoundedOccurrence {
    pub(crate) start_byte: usize,
    pub(crate) end_byte: usize,
    pub(crate) first_tok: usize,
    pub(crate) last_tok: usize,
}

fn valid_numeric_occurrence_edges(
    tokens: &[Token],
    f_idx: usize,
    l_idx: usize,
    allow_bp_slash: bool,
) -> bool {
    let starts_with_num = tokens
        .get(f_idx)
        .is_some_and(|t| t.kind == TokenKind::Number);
    let ends_with_num = tokens
        .get(l_idx)
        .is_some_and(|t| t.kind == TokenKind::Number);

    if starts_with_num
        && f_idx > 0
        && let Some(prev) = tokens.get(f_idx - 1)
    {
        if prev.kind == TokenKind::Hyphen || prev.norm == "+" {
            return false;
        }
        if prev.kind == TokenKind::Slash {
            if !allow_bp_slash {
                return false;
            }
            let two_part_bp = f_idx >= 2
                && tokens
                    .get(f_idx - 2)
                    .is_some_and(|n| n.kind == TokenKind::Number)
                && !tokens
                    .get(l_idx + 1)
                    .is_some_and(|n| n.kind == TokenKind::Slash);
            if !two_part_bp {
                return false;
            }
        }
        if prev.kind == TokenKind::Other
            && prev.norm == ":"
            && f_idx >= 2
            && tokens
                .get(f_idx - 2)
                .is_some_and(|n| n.kind == TokenKind::Number)
        {
            return false;
        }
    }

    if ends_with_num && let Some(next) = tokens.get(l_idx + 1) {
        if next.kind == TokenKind::Hyphen
            && tokens
                .get(l_idx + 2)
                .is_some_and(|n| matches!(n.kind, TokenKind::Number | TokenKind::InvalidNumber))
        {
            return false;
        }
        if next.kind == TokenKind::Slash
            && tokens
                .get(l_idx + 2)
                .is_some_and(|n| matches!(n.kind, TokenKind::Number | TokenKind::InvalidNumber))
        {
            if !allow_bp_slash {
                return false;
            }
            let two_part_bp = !tokens
                .get(l_idx + 3)
                .is_some_and(|n| n.kind == TokenKind::Slash);
            if !two_part_bp {
                return false;
            }
        }
        if next.kind == TokenKind::Other && next.norm == ":" {
            let attached = tokens.get(l_idx).is_some_and(|t| t.end == next.start);
            let followed_by_num = tokens
                .get(l_idx + 2)
                .is_some_and(|n| matches!(n.kind, TokenKind::Number | TokenKind::InvalidNumber));
            if attached || followed_by_num {
                return false;
            }
        }
    }

    true
}

pub(crate) fn find_token_bounded_occurrences_with_tokens(
    haystack: &str,
    tokens: &[Token],
    needle: &str,
    allow_bp_slash: bool,
) -> Vec<BoundedOccurrence> {
    if needle.trim().is_empty() {
        return Vec::new();
    }
    let lead_ws = needle.len() - needle.trim_start().len();
    let trail_ws = needle.len() - needle.trim_end().len();
    let mut out = Vec::new();

    for (start, _) in haystack.match_indices(needle) {
        let end = start + needle.len();
        let t_start = start + lead_ws;
        let t_end = end.saturating_sub(trail_ws);
        if t_start >= t_end {
            continue;
        }
        let Some(first_tok) = tokens.iter().position(|t| t.start == t_start) else {
            continue;
        };
        let Some(last_tok) = tokens.iter().position(|t| t.end == t_end) else {
            continue;
        };
        if first_tok > last_tok {
            continue;
        }
        if !valid_numeric_occurrence_edges(tokens, first_tok, last_tok, allow_bp_slash) {
            continue;
        }
        out.push(BoundedOccurrence {
            start_byte: start,
            end_byte: end,
            first_tok,
            last_tok,
        });
    }
    out
}

pub(crate) fn has_token_bounded_occurrence(
    haystack: &str,
    needle: &str,
    allow_bp_slash: bool,
) -> bool {
    let tokens = tokenize(haystack);
    !find_token_bounded_occurrences_with_tokens(haystack, &tokens, needle, allow_bp_slash)
        .is_empty()
}

pub(crate) fn check_numeric_unit_around_occurrence(
    input: &str,
    input_tokens: &[Token],
    indicator: Indicator,
    fq: &str,
    fq_last_tok: usize,
) -> bool {
    let Some((_, fq_unit)) = parse_quote_number_with_unit(fq) else {
        return false;
    };
    if let Some(u) = fq_unit
        && !u.matches_indicator(indicator)
    {
        return false;
    }

    let mut check_idx = fq_last_tok + 1;
    let len = input_tokens.len();
    if check_idx >= len {
        return true;
    }

    // Para presión arterial sistólica ("120" en "120/80"), la unidad sigue tras el segundo número.
    if indicator == Indicator::BloodPressure
        && fq_unit.is_none()
        && input_tokens
            .get(check_idx)
            .is_some_and(|t| t.kind == TokenKind::Slash)
        && input_tokens
            .get(check_idx + 1)
            .is_some_and(|t| t.kind == TokenKind::Number)
    {
        check_idx += 2;
        if check_idx >= len {
            return true;
        }
    }

    if fq_unit.is_some() {
        return !has_post_unit_unknown_tail(input_tokens, check_idx, len);
    }

    if let Some((unit, _)) = match_unit_at(input_tokens, check_idx, len) {
        return unit.matches_indicator(indicator);
    }

    !is_unknown_unit_after_number(input, input_tokens, check_idx, len)
}
