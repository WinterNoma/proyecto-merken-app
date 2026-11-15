//! Capa 1 del intérprete de texto de Merken (RF-14): reglas deterministas.
//!
//! Convierte texto libre en español, inglés o mezclado en una [`Proposal`]
//! determinista, pura (sin E/S ni reloj) e instantánea. Nunca guarda nada y
//! nunca inventa valores ni horas: lo ambiguo queda con `value: None` o
//! [`TimeHint::Ambiguous`], las negaciones van a `negated` y lo que no se
//! entiende va a `uninterpreted`.

use crate::{
    catalog::{BodyZone, Indicator, SleepQuality},
    measurement::Measurement,
    proposal::{
        InterpreterKind, PROPOSAL_SCHEMA_VERSION, Proposal, ProposedField, ProposedRecord, TimeHint,
    },
    units::{Bpm, Celsius, Hours, MgDl, MmHg, Percent, Score},
};
use serde_json::{Number, Value, json};

/// Versión del intérprete determinista de reglas.
pub const RULES_INTERPRETER_VERSION: &str = "rules-interp-2026.11.14";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenKind {
    Word,
    Number,
    Slash,
    Percent,
    Hyphen,
    ClauseSep,
    Other,
}

#[derive(Clone, Debug)]
struct Token {
    norm: String,
    kind: TokenKind,
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug)]
struct ParsedNumber {
    value: f64,
    int_value: Option<u64>,
    end_idx: usize,
    start_byte: usize,
    end_byte: usize,
}

#[derive(Clone, Debug)]
struct MatchedTime {
    start_idx: usize,
    end_idx: usize,
    quote: String,
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
        _ => ch,
    }
}

fn is_apostrophe(ch: char) -> bool {
    matches!(ch, '\'' | '’')
}

fn tokenize(text: &str) -> Vec<Token> {
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
            }

            tokens.push(Token {
                norm,
                kind: TokenKind::Number,
                start,
                end,
            });
            continue;
        }

        let norm_ch = normalize_latin_char(ch);
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
            '-' | '–' | '—' => TokenKind::Hyphen,
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

fn slice_tokens(orig: &str, tokens: &[Token], start_idx: usize, end_idx: usize) -> Option<String> {
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

fn slice_from_bytes(orig: &str, start_byte: usize, end_byte: usize) -> Option<String> {
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

fn truncate_chars(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => s.get(..byte_idx).unwrap_or(s).to_owned(),
        None => s.to_owned(),
    }
}

fn word_at(tokens: &[Token], idx: usize) -> Option<&str> {
    tokens
        .get(idx)
        .filter(|t| t.kind == TokenKind::Word)
        .map(|t| t.norm.as_str())
}

fn starts_with_words(tokens: &[Token], idx: usize, words: &[&str]) -> bool {
    for (offset, expected) in words.iter().enumerate() {
        if word_at(tokens, idx + offset) != Some(*expected) {
            return false;
        }
    }
    true
}

fn es_unit(w: &str) -> Option<u64> {
    match w {
        "uno" | "una" => Some(1),
        "dos" => Some(2),
        "tres" => Some(3),
        "cuatro" => Some(4),
        "cinco" => Some(5),
        "seis" => Some(6),
        "siete" => Some(7),
        "ocho" => Some(8),
        "nueve" => Some(9),
        _ => None,
    }
}

fn es_unit_in_compound(w: &str) -> Option<u64> {
    if w == "un" { Some(1) } else { es_unit(w) }
}

fn es_teen(w: &str) -> Option<u64> {
    match w {
        "diez" => Some(10),
        "once" => Some(11),
        "doce" => Some(12),
        "trece" => Some(13),
        "catorce" => Some(14),
        "quince" => Some(15),
        "dieciseis" => Some(16),
        "diecisiete" => Some(17),
        "dieciocho" => Some(18),
        "diecinueve" => Some(19),
        "veinte" => Some(20),
        "veintiuno" | "veintiuna" | "veintiun" => Some(21),
        "veintidos" => Some(22),
        "veintitres" => Some(23),
        "veinticuatro" => Some(24),
        "veinticinco" => Some(25),
        "veintiseis" => Some(26),
        "veintisiete" => Some(27),
        "veintiocho" => Some(28),
        "veintinueve" => Some(29),
        _ => None,
    }
}

fn es_ten(w: &str) -> Option<u64> {
    match w {
        "treinta" => Some(30),
        "cuarenta" => Some(40),
        "cincuenta" => Some(50),
        "sesenta" => Some(60),
        "setenta" => Some(70),
        "ochenta" => Some(80),
        "noventa" => Some(90),
        _ => None,
    }
}

fn es_sub_hundred(tokens: &[Token], idx: usize, allow_un: bool) -> Option<(u64, usize)> {
    let w = word_at(tokens, idx)?;
    if let Some(t) = es_ten(w) {
        if word_at(tokens, idx + 1) == Some("y")
            && let Some(u_w) = word_at(tokens, idx + 2)
            && let Some(u) = if allow_un {
                es_unit_in_compound(u_w)
            } else {
                es_unit(u_w)
            }
        {
            return Some((t + u, idx + 3));
        }
        return Some((t, idx + 1));
    }
    if w == "diez"
        && word_at(tokens, idx + 1) == Some("y")
        && let Some(u_w) = word_at(tokens, idx + 2)
        && let Some(u) = es_unit(u_w)
        && (6..=9).contains(&u)
    {
        return Some((10 + u, idx + 3));
    }
    if let Some(t) = es_teen(w) {
        return Some((t, idx + 1));
    }
    let u = if allow_un {
        es_unit_in_compound(w)
    } else {
        es_unit(w)
    };
    if let Some(u) = u {
        return Some((u, idx + 1));
    }
    None
}

fn parse_es_int(tokens: &[Token], idx: usize) -> Option<(u64, usize)> {
    let w = word_at(tokens, idx)?;
    if w == "cero" {
        return Some((0, idx + 1));
    }
    if w == "cien" {
        return Some((100, idx + 1));
    }
    if w == "ciento" {
        if let Some((rest, end)) = es_sub_hundred(tokens, idx + 1, true) {
            return Some((100 + rest, end));
        }
        return Some((100, idx + 1));
    }
    if matches!(w, "doscientos" | "doscientas") {
        if let Some((rest, end)) = es_sub_hundred(tokens, idx + 1, true) {
            return Some((200 + rest, end));
        }
        return Some((200, idx + 1));
    }
    es_sub_hundred(tokens, idx, false)
}

fn en_unit(w: &str) -> Option<u64> {
    match w {
        "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        _ => None,
    }
}

fn en_teen(w: &str) -> Option<u64> {
    match w {
        "ten" => Some(10),
        "eleven" => Some(11),
        "twelve" => Some(12),
        "thirteen" => Some(13),
        "fourteen" => Some(14),
        "fifteen" => Some(15),
        "sixteen" => Some(16),
        "seventeen" => Some(17),
        "eighteen" => Some(18),
        "nineteen" => Some(19),
        _ => None,
    }
}

fn en_ten(w: &str) -> Option<u64> {
    match w {
        "twenty" => Some(20),
        "thirty" => Some(30),
        "forty" => Some(40),
        "fifty" => Some(50),
        "sixty" => Some(60),
        "seventy" => Some(70),
        "eighty" => Some(80),
        "ninety" => Some(90),
        _ => None,
    }
}

fn en_sub_hundred(tokens: &[Token], idx: usize) -> Option<(u64, usize)> {
    let w = word_at(tokens, idx)?;
    if let Some(t) = en_ten(w) {
        let mut next = idx + 1;
        if tokens
            .get(next)
            .is_some_and(|tk| tk.kind == TokenKind::Hyphen)
        {
            next += 1;
        }
        if let Some(u_w) = word_at(tokens, next)
            && let Some(u) = en_unit(u_w)
        {
            return Some((t + u, next + 1));
        }
        return Some((t, idx + 1));
    }
    if let Some(t) = en_teen(w) {
        return Some((t, idx + 1));
    }
    if let Some(u) = en_unit(w) {
        return Some((u, idx + 1));
    }
    None
}

fn parse_en_int(tokens: &[Token], idx: usize) -> Option<(u64, usize)> {
    let w = word_at(tokens, idx)?;
    if w == "zero" {
        return Some((0, idx + 1));
    }
    let hundred_end = if matches!(w, "one" | "a") && word_at(tokens, idx + 1) == Some("hundred") {
        Some((100_u64, idx + 2))
    } else if w == "two" && word_at(tokens, idx + 1) == Some("hundred") {
        Some((200_u64, idx + 2))
    } else if w == "hundred" {
        Some((100_u64, idx + 1))
    } else {
        None
    };

    if let Some((base, mut after_h)) = hundred_end {
        if word_at(tokens, after_h) == Some("and") && en_sub_hundred(tokens, after_h + 1).is_some()
        {
            after_h += 1;
        }
        if let Some((rest, end)) = en_sub_hundred(tokens, after_h) {
            return Some((base + rest, end));
        }
        return Some((base, after_h));
    }

    en_sub_hundred(tokens, idx)
}

fn parse_decimal_frac(tokens: &[Token], idx: usize) -> Option<(String, usize)> {
    if let Some(tk) = tokens.get(idx)
        && tk.kind == TokenKind::Number
        && !tk.norm.contains('.')
    {
        return Some((tk.norm.clone(), idx + 1));
    }
    if let Some((v, end)) = parse_es_int(tokens, idx).or_else(|| parse_en_int(tokens, idx)) {
        if v == 0 {
            if let Some((v2, end2)) =
                parse_es_int(tokens, end).or_else(|| parse_en_int(tokens, end))
                && v2 < 10
            {
                return Some((format!("0{v2}"), end2));
            }
            return Some(("0".to_owned(), end));
        }
        if v < 100 {
            return Some((v.to_string(), end));
        }
    }
    None
}

fn parse_number(tokens: &[Token], idx: usize) -> Option<ParsedNumber> {
    let first = tokens.get(idx)?;

    let (base_int, mut end_idx, mut digit_f64, mut is_digit_int) =
        if first.kind == TokenKind::Number {
            let val: f64 = first.norm.parse().ok()?;
            if !val.is_finite() {
                return None;
            }
            let int_val = if !first.norm.contains('.') {
                first.norm.parse::<u64>().ok()
            } else {
                None
            };
            (int_val, idx + 1, Some(val), int_val.is_some())
        } else if let Some((v, end)) = parse_es_int(tokens, idx) {
            (Some(v), end, None, true)
        } else if let Some((v, end)) = parse_en_int(tokens, idx) {
            (Some(v), end, None, true)
        } else {
            return None;
        };

    // Decimal word separator: "coma", "punto", "con", "point", "dot"
    if is_digit_int
        && let Some(int_v) = base_int
        && let Some(sep) = word_at(tokens, end_idx)
        && matches!(sep, "coma" | "punto" | "con" | "point" | "dot")
        && let Some((frac_str, frac_end)) = parse_decimal_frac(tokens, end_idx + 1)
        && let Ok(val) = format!("{int_v}.{frac_str}").parse::<f64>()
        && val.is_finite()
    {
        end_idx = frac_end;
        digit_f64 = Some(val);
        is_digit_int = false;
    }

    // Half suffix: "y media", "y medio", "and a half", "and half"
    if is_digit_int && let Some(int_v) = base_int {
        if starts_with_words(tokens, end_idx, &["y", "media"])
            || starts_with_words(tokens, end_idx, &["y", "medio"])
        {
            let val = format!("{int_v}.5").parse::<f64>().ok()?;
            end_idx += 2;
            digit_f64 = Some(val);
            is_digit_int = false;
        } else if starts_with_words(tokens, end_idx, &["and", "a", "half"]) {
            let val = format!("{int_v}.5").parse::<f64>().ok()?;
            end_idx += 3;
            digit_f64 = Some(val);
            is_digit_int = false;
        } else if starts_with_words(tokens, end_idx, &["and", "half"]) {
            let val = format!("{int_v}.5").parse::<f64>().ok()?;
            end_idx += 2;
            digit_f64 = Some(val);
            is_digit_int = false;
        }
    }

    let last = tokens.get(end_idx.checked_sub(1)?)?;
    let value = match digit_f64 {
        Some(v) => v,
        None => f64::from(u32::try_from(base_int?).ok()?),
    };
    let int_value = if is_digit_int { base_int } else { None };

    Some(ParsedNumber {
        value,
        int_value,
        end_idx,
        start_byte: first.start,
        end_byte: last.end,
    })
}

fn number_to_json(num: ParsedNumber) -> Option<Value> {
    if let Some(u) = num.int_value {
        return Some(Value::from(u));
    }
    Number::from_f64(num.value).map(Value::Number)
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

fn find_time_hints(orig: &str, tokens: &[Token], start: usize, end: usize) -> Vec<MatchedTime> {
    let mut out = Vec::new();
    let mut i = start;

    while i < end {
        let matched_len: Option<usize> = 'm: {
            // "desde hace <num> horas/minutos/dias" or "desde hace un rato/tiempo"
            if starts_with_words(tokens, i, &["desde", "hace"]) && i + 2 < end {
                if let Some(num) = parse_number(tokens, i + 2)
                    && num.end_idx < end
                    && matches!(
                        word_at(tokens, num.end_idx),
                        Some(
                            "hora"
                                | "horas"
                                | "minuto"
                                | "minutos"
                                | "dia"
                                | "dias"
                                | "semana"
                                | "semanas"
                        )
                    )
                {
                    break 'm Some(num.end_idx + 1 - i);
                }
                if starts_with_words(tokens, i + 2, &["un", "rato"])
                    || starts_with_words(tokens, i + 2, &["rato"])
                    || starts_with_words(tokens, i + 2, &["tiempo"])
                    || starts_with_words(tokens, i + 2, &["unas", "horas"])
                {
                    let extra = if word_at(tokens, i + 2) == Some("un")
                        || word_at(tokens, i + 2) == Some("unas")
                    {
                        4
                    } else {
                        3
                    };
                    break 'm Some(extra);
                }
            }

            // "hace <num> horas/minutos/dias" / "hace un rato" / "hace unas horas"
            if word_at(tokens, i) == Some("hace") && i + 1 < end {
                if let Some(num) = parse_number(tokens, i + 1)
                    && num.end_idx < end
                    && matches!(
                        word_at(tokens, num.end_idx),
                        Some(
                            "hora"
                                | "horas"
                                | "minuto"
                                | "minutos"
                                | "dia"
                                | "dias"
                                | "semana"
                                | "semanas"
                        )
                    )
                {
                    break 'm Some(num.end_idx + 1 - i);
                }
                if starts_with_words(tokens, i + 1, &["un", "rato"])
                    || starts_with_words(tokens, i + 1, &["unas", "horas"])
                    || starts_with_words(tokens, i + 1, &["unos", "dias"])
                    || starts_with_words(tokens, i + 1, &["un", "momento"])
                {
                    break 'm Some(3);
                }
                if matches!(word_at(tokens, i + 1), Some("rato" | "horas" | "dias")) {
                    break 'm Some(2);
                }
            }

            // "<num> hours/minutes/days ago" / "a few hours ago" / "an hour ago" / "a while ago"
            if let Some(num) = parse_number(tokens, i)
                && num.end_idx + 1 < end
                && matches!(
                    word_at(tokens, num.end_idx),
                    Some("hour" | "hours" | "minute" | "minutes" | "day" | "days" | "hr" | "hrs")
                )
                && word_at(tokens, num.end_idx + 1) == Some("ago")
            {
                break 'm Some(num.end_idx + 2 - i);
            }
            for pat in [
                &["since", "this", "morning"][..],
                &["since", "this", "afternoon"],
                &["since", "this", "evening"],
                &["since", "last", "night"],
                &["since", "yesterday", "morning"],
                &["since", "yesterday", "night"],
                &["since", "i", "woke", "up"],
                &["when", "i", "woke", "up"],
                &["a", "few", "hours", "ago"],
                &["a", "while", "ago"],
                &["an", "hour", "ago"],
                &["desde", "esta", "manana"],
                &["desde", "esta", "tarde"],
                &["desde", "esta", "noche"],
                &["desde", "esta", "madrugada"],
                &["desde", "ayer", "por", "la", "manana"],
                &["desde", "ayer", "por", "la", "tarde"],
                &["desde", "ayer", "por", "la", "noche"],
                &["desde", "ayer", "en", "la", "manana"],
                &["desde", "ayer", "en", "la", "noche"],
                &["desde", "el", "despertar"],
                &["ayer", "por", "la", "manana"],
                &["ayer", "por", "la", "tarde"],
                &["ayer", "por", "la", "noche"],
                &["ayer", "en", "la", "manana"],
                &["ayer", "en", "la", "noche"],
                &["hoy", "por", "la", "manana"],
                &["hoy", "por", "la", "tarde"],
                &["hoy", "en", "la", "manana"],
                &["por", "la", "manana"],
                &["por", "la", "tarde"],
                &["por", "la", "noche"],
                &["en", "la", "manana"],
                &["en", "la", "tarde"],
                &["en", "la", "noche"],
                &["en", "la", "madrugada"],
                &["in", "the", "morning"],
                &["in", "the", "afternoon"],
                &["in", "the", "evening"],
                &["desde", "ayer"],
                &["desde", "anoche"],
                &["desde", "hoy"],
                &["since", "yesterday"],
                &["yesterday", "morning"],
                &["yesterday", "afternoon"],
                &["yesterday", "evening"],
                &["yesterday", "night"],
                &["this", "morning"],
                &["this", "afternoon"],
                &["this", "evening"],
                &["last", "night"],
                &["earlier", "today"],
                &["esta", "manana"],
                &["esta", "tarde"],
                &["esta", "noche"],
                &["esta", "madrugada"],
                &["al", "despertar"],
                &["al", "levantarme"],
                &["ayer"],
                &["anoche"],
                &["anteayer"],
                &["antier"],
                &["yesterday"],
                &["tonight"],
                &["earlier"],
                &["recently"],
                &["recientemente"],
            ] {
                if i + pat.len() <= end && starts_with_words(tokens, i, pat) {
                    break 'm Some(pat.len());
                }
            }

            // "a las <num>" / "at <num> (am|pm)?"
            if starts_with_words(tokens, i, &["a", "las"])
                && i + 2 < end
                && let Some(num) = parse_number(tokens, i + 2)
                && num.end_idx <= end
            {
                break 'm Some(num.end_idx - i);
            }
            if word_at(tokens, i) == Some("at")
                && i + 1 < end
                && let Some(num) = parse_number(tokens, i + 1)
                && num.end_idx <= end
            {
                let mut total = num.end_idx - i;
                if num.end_idx < end && matches!(word_at(tokens, num.end_idx), Some("am" | "pm")) {
                    total += 1;
                }
                break 'm Some(total);
            }

            None
        };

        if let Some(len) = matched_len
            && let Some(quote) = slice_tokens(orig, tokens, i, i + len)
        {
            out.push(MatchedTime {
                start_idx: i,
                end_idx: i + len,
                quote,
            });
            i += len;
        } else {
            i += 1;
        }
    }

    out
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
        if let Some(num) = parse_number(tokens, idx) {
            return num.end_idx < end
                && (tokens
                    .get(num.end_idx)
                    .is_some_and(|tk| matches!(tk.kind, TokenKind::Slash | TokenKind::Percent))
                    || matches!(
                        word_at(tokens, num.end_idx),
                        Some(
                            "bpm"
                                | "lpm"
                                | "h"
                                | "hr"
                                | "hrs"
                                | "hora"
                                | "horas"
                                | "hour"
                                | "hours"
                                | "c"
                                | "celsius"
                                | "grados"
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
            | "nada"
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
    clause_start: usize,
    clause_end: usize,
) -> bool {
    let Some(w) = word_at(tokens, idx) else {
        return false;
    };
    if !matches!(w, "y" | "e" | "and") {
        return false;
    }
    if idx == clause_start || idx + 1 >= clause_end {
        return false;
    }

    // Keep "cinco y media" / "five and a half"
    if w == "y" && matches!(word_at(tokens, idx + 1), Some("media" | "medio")) {
        return false;
    }
    if w == "and"
        && (starts_with_words(tokens, idx + 1, &["a", "half"])
            || word_at(tokens, idx + 1) == Some("half"))
    {
        return false;
    }

    // Keep Spanish tens + unit: "setenta y dos"
    if w == "y"
        && let Some(prev) = word_at(tokens, idx - 1)
        && (es_ten(prev).is_some() || prev == "diez")
        && let Some(next) = word_at(tokens, idx + 1)
        && (es_unit_in_compound(next).is_some())
    {
        return false;
    }

    // Keep English "one hundred and seventy two"
    if w == "and"
        && word_at(tokens, idx - 1) == Some("hundred")
        && en_sub_hundred(tokens, idx + 1).is_some()
    {
        return false;
    }

    // Keep "5 hours and 30 minutes" / "5 horas y 30 minutos"
    if matches!(
        word_at(tokens, idx - 1),
        Some("hora" | "horas" | "hour" | "hours" | "h" | "hr" | "hrs")
    ) && let Some(num) = parse_number(tokens, idx + 1)
        && num.end_idx < clause_end
        && matches!(
            word_at(tokens, num.end_idx),
            Some("minuto" | "minutos" | "minute" | "minutes" | "min" | "mins")
        )
    {
        return false;
    }

    // Split if the right side starts/contains an indicator, symptom, negation, or time+indicator
    let mut probe = idx + 1;
    while probe < clause_end {
        if is_indicator_or_symptom_start(tokens, probe, clause_end) {
            return true;
        }
        if parse_body_zone(tokens, probe, clause_end).is_some() {
            return true;
        }
        probe += 1;
    }
    false
}

fn split_clauses(tokens: &[Token]) -> Vec<(usize, usize)> {
    let mut rough: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    let n = tokens.len();

    for (idx, tk) in tokens.iter().enumerate() {
        let is_sep = tk.kind == TokenKind::ClauseSep
            || (tk.kind == TokenKind::Word && is_clause_break_word(&tk.norm));
        if is_sep {
            if start < idx {
                rough.push((start, idx));
            }
            start = idx + 1;
        }
    }
    if start < n {
        rough.push((start, n));
    }

    let mut refined = Vec::new();
    for (c_start, c_end) in rough {
        let mut sub_start = c_start;
        let mut i = c_start;
        while i < c_end {
            if should_split_conjunction(tokens, i, sub_start, c_end) {
                if sub_start < i {
                    refined.push((sub_start, i));
                }
                sub_start = i + 1;
            }
            i += 1;
        }
        if sub_start < c_end {
            refined.push((sub_start, c_end));
        }
    }
    refined
}

fn match_body_zone_phrase(tokens: &[Token], idx: usize, end: usize) -> Option<(BodyZone, usize)> {
    if idx >= end {
        return None;
    }

    // Side + limb in both orders ("left foot", "pie izquierdo")
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

    // Two-word non-sided zones
    if idx + 1 < end
        && (starts_with_words(tokens, idx, &["lower", "back"])
            || starts_with_words(tokens, idx, &["upper", "back"])
            || starts_with_words(tokens, idx, &["espalda", "baja"])
            || starts_with_words(tokens, idx, &["espalda", "alta"]))
    {
        return Some((BodyZone::Back, idx + 2));
    }

    // Single-word anatomical zones
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

fn parse_body_zone(tokens: &[Token], start: usize, end: usize) -> Option<(BodyZone, usize, usize)> {
    let mut i = start;
    while i < end {
        if let Some((zone, z_end)) = match_body_zone_phrase(tokens, i, end) {
            return Some((zone, i, z_end));
        }
        i += 1;
    }
    None
}

fn is_negated_good_sleep(tokens: &[Token], start: usize, end: usize, time_mask: &[bool]) -> bool {
    let words: Vec<&str> = (start..end)
        .filter(|&i| !time_mask.get(i).copied().unwrap_or(false))
        .filter_map(|i| word_at(tokens, i))
        .collect();
    let has_sleep = words.iter().any(|w| {
        matches!(
            *w,
            "dormi" | "dormir" | "dormido" | "sleep" | "slept" | "sleeping"
        )
    });
    if !has_sleep {
        return false;
    }
    let has_neg = words.iter().any(|w| {
        matches!(
            *w,
            "no" | "not" | "didnt" | "dont" | "havent" | "couldnt" | "never" | "nada"
        )
    });
    let has_good_or_can = words.iter().any(|w| {
        matches!(
            *w,
            "bien" | "well" | "good" | "pude" | "podido" | "could" | "nada"
        )
    });
    has_neg && has_good_or_can
}

fn is_symptom_or_indicator_negation(
    tokens: &[Token],
    start: usize,
    end: usize,
    time_mask: &[bool],
) -> bool {
    if is_negated_good_sleep(tokens, start, end, time_mask) {
        return false;
    }
    let words: Vec<&str> = (start..end)
        .filter(|&i| !time_mask.get(i).copied().unwrap_or(false))
        .filter_map(|i| word_at(tokens, i))
        .collect();

    let has_neg_cue = words.iter().any(|w| {
        matches!(
            *w,
            "no" | "sin"
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
                | "not"
                | "without"
                | "never"
                | "gone"
                | "anymore"
                | "desaparecio"
        )
    }) || words.windows(2).any(|w| {
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
    });

    if !has_neg_cue {
        return false;
    }

    words.iter().any(|w| {
        matches!(
            *w,
            "duele"
                | "duelen"
                | "dolia"
                | "dolor"
                | "molestia"
                | "molesta"
                | "sintoma"
                | "sintomas"
                | "fiebre"
                | "fever"
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
                | "symptom"
                | "symptoms"
                | "nausea"
                | "nauseas"
                | "mareo"
                | "mareos"
                | "dizzy"
                | "dizziness"
        )
    }) || parse_body_zone(tokens, start, end).is_some()
}

fn is_sleep_word(w: &str) -> bool {
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

fn is_hour_unit(w: &str) -> bool {
    matches!(w, "h" | "hr" | "hrs" | "hora" | "horas" | "hour" | "hours")
}

fn is_symptom_cue(w: &str) -> bool {
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
    )
}

fn is_different_unit_after(tokens: &[Token], idx: usize, target: Indicator) -> bool {
    let Some(tk) = tokens.get(idx) else {
        return false;
    };
    if tk.kind == TokenKind::Slash {
        let is_ten = tokens.get(idx + 1).is_some_and(|n| n.norm == "10");
        if is_ten {
            return !matches!(target, Indicator::Mood | Indicator::Symptom);
        }
        return target != Indicator::BloodPressure;
    }
    if tk.kind == TokenKind::Percent {
        return target != Indicator::OxygenInBlood;
    }
    if let Some(w) = word_at(tokens, idx) {
        return match w {
            "bpm" | "lpm" | "pulsaciones" | "latidos" => target != Indicator::HeartRate,
            "mmhg" => target != Indicator::BloodPressure,
            "celsius" | "centigrados" | "grados" | "degrees" => {
                target != Indicator::BodyTemperature
            }
            "c" if idx > 0
                && tokens.get(idx - 1).is_some_and(|p| {
                    p.norm == "°" || p.norm == "º" || p.kind == TokenKind::Number
                }) =>
            {
                target != Indicator::BodyTemperature
            }
            "mg" | "mgdl" => target != Indicator::Glucose,
            "h" | "hr" | "hrs" | "hora" | "horas" | "hour" | "hours" => target != Indicator::Sleep,
            "por" | "percent" => target != Indicator::OxygenInBlood,
            _ => false,
        };
    }
    if matches!(tk.norm.as_str(), "°" | "º") && word_at(tokens, idx + 1) == Some("c") {
        return target != Indicator::BodyTemperature;
    }
    false
}

fn find_number_in_range(
    tokens: &[Token],
    start: usize,
    end: usize,
    time_mask: &[bool],
    target: Indicator,
) -> Option<ParsedNumber> {
    let mut i = start;
    while i < end {
        if time_mask.get(i).copied().unwrap_or(false) {
            i += 1;
            continue;
        }
        if let Some(num) = parse_number(tokens, i)
            && num.end_idx <= end
        {
            let any_time = (i..num.end_idx).any(|k| time_mask.get(k).copied().unwrap_or(false));
            if !any_time && !is_different_unit_after(tokens, num.end_idx, target) {
                return Some(num);
            }
            i = num.end_idx;
            continue;
        }
        i += 1;
    }
    None
}

fn extract_sleep_record(
    orig: &str,
    tokens: &[Token],
    start: usize,
    end: usize,
    time_mask: &[bool],
    time_hint: TimeHint,
) -> Option<ProposedRecord> {
    let quote = slice_tokens(orig, tokens, start, end)?;
    let mut hours_field = field_none("hours");
    let mut quality_field = field_none("quality");

    // 1. Hours extraction
    let mut i = start;
    while i < end {
        if time_mask.get(i).copied().unwrap_or(false) {
            i += 1;
            continue;
        }
        if let Some(num) = parse_number(tokens, i)
            && num.end_idx <= end
        {
            let any_time = (i..num.end_idx).any(|k| time_mask.get(k).copied().unwrap_or(false));
            if any_time {
                i = num.end_idx;
                continue;
            }
            let mut val = num.value;
            let mut num_end = num.end_idx;
            let mut is_int = num.int_value;
            let mut has_hour_unit = false;

            if num_end < end && word_at(tokens, num_end).is_some_and(is_hour_unit) {
                has_hour_unit = true;
                let unit_idx = num_end;
                num_end += 1;

                // "<num> horas y media" / "<num> hours and a half"
                if (starts_with_words(tokens, unit_idx + 1, &["y", "media"])
                    || starts_with_words(tokens, unit_idx + 1, &["y", "medio"]))
                    && unit_idx + 3 <= end
                {
                    if let Some(iv) = is_int
                        && let Ok(parsed) = format!("{iv}.5").parse::<f64>()
                    {
                        val = parsed;
                        is_int = None;
                        num_end = unit_idx + 3;
                    }
                } else if starts_with_words(tokens, unit_idx + 1, &["and", "a", "half"])
                    && unit_idx + 4 <= end
                {
                    if let Some(iv) = is_int
                        && let Ok(parsed) = format!("{iv}.5").parse::<f64>()
                    {
                        val = parsed;
                        is_int = None;
                        num_end = unit_idx + 4;
                    }
                } else if (word_at(tokens, unit_idx + 1) == Some("y")
                    || word_at(tokens, unit_idx + 1) == Some("and"))
                    && let Some(mins) = parse_number(tokens, unit_idx + 2)
                    && mins.end_idx < end
                    && matches!(
                        word_at(tokens, mins.end_idx),
                        Some("minuto" | "minutos" | "minute" | "minutes" | "min" | "mins")
                    )
                    && mins.value > 0.0
                    && mins.value < 60.0
                {
                    val += (mins.value / 60.0 * 100.0).round() / 100.0;
                    is_int = None;
                    num_end = mins.end_idx + 1;
                }
            }

            if (has_hour_unit || !is_different_unit_after(tokens, num.end_idx, Indicator::Sleep))
                && Hours::try_from(val).is_ok()
                && let Some(val_quote) = slice_tokens(orig, tokens, i, num_end)
            {
                let json_val = if let Some(iv) = is_int {
                    Some(Value::from(iv))
                } else {
                    Number::from_f64(val).map(Value::Number)
                };
                if let Some(jv) = json_val {
                    hours_field = field_some("hours", jv, val_quote);
                    break;
                }
            }
            i = num.end_idx;
            continue;
        }
        // "media hora" / "half an hour" / "half hour"
        if (starts_with_words(tokens, i, &["media", "hora"])
            || starts_with_words(tokens, i, &["half", "hour"]))
            && i + 2 <= end
            && let Some(val_quote) = slice_tokens(orig, tokens, i, i + 2)
        {
            hours_field = field_some("hours", json!(0.5), val_quote);
            break;
        }
        if starts_with_words(tokens, i, &["half", "an", "hour"])
            && i + 3 <= end
            && let Some(val_quote) = slice_tokens(orig, tokens, i, i + 3)
        {
            hours_field = field_some("hours", json!(0.5), val_quote);
            break;
        }
        i += 1;
    }

    // 2. Quality extraction
    if is_negated_good_sleep(tokens, start, end, time_mask) {
        quality_field = field_some("quality", json!(SleepQuality::Poor.as_str()), quote.clone());
    } else {
        let mut q_idx = start;
        while q_idx < end {
            if time_mask.get(q_idx).copied().unwrap_or(false) {
                q_idx += 1;
                continue;
            }
            if starts_with_words(tokens, q_idx, &["mas", "o", "menos"])
                && q_idx + 3 <= end
                && let Some(q_quote) = slice_tokens(orig, tokens, q_idx, q_idx + 3)
            {
                quality_field = field_some("quality", json!(SleepQuality::Fair.as_str()), q_quote);
                break;
            }
            if starts_with_words(tokens, q_idx, &["so", "so"])
                && q_idx + 2 <= end
                && let Some(q_quote) = slice_tokens(orig, tokens, q_idx, q_idx + 2)
            {
                quality_field = field_some("quality", json!(SleepQuality::Fair.as_str()), q_quote);
                break;
            }
            if let Some(w) = word_at(tokens, q_idx) {
                let q = match w {
                    "mal" | "mala" | "malo" | "pesimo" | "pesima" | "fatal" | "horrible"
                    | "terrible" | "poor" | "poorly" | "bad" | "badly" | "terribly" | "awful"
                    | "awfully" | "restless" => Some(SleepQuality::Poor),
                    "regular" | "aceptable" | "pasable" | "mediocre" | "fair" | "fairly"
                    | "okay" | "ok" | "average" | "alright" | "decent" => Some(SleepQuality::Fair),
                    "bien" | "bueno" | "buena" | "genial" | "excelente" | "estupendo"
                    | "perfecto" | "profundo" | "profundamente" | "good" | "well" | "great"
                    | "excellent" | "fine" | "deeply" | "soundly" => Some(SleepQuality::Good),
                    _ => None,
                };
                if let Some(quality) = q
                    && let Some(q_quote) = slice_tokens(orig, tokens, q_idx, q_idx + 1)
                {
                    quality_field = field_some("quality", json!(quality.as_str()), q_quote);
                    break;
                }
            }
            q_idx += 1;
        }
    }

    Some(ProposedRecord {
        indicator: Indicator::Sleep,
        fields: vec![
            hours_field,
            quality_field,
            field_none("window_start"),
            field_none("window_end"),
        ],
        time: time_hint,
        quote,
    })
}

fn extract_symptom_record(
    orig: &str,
    tokens: &[Token],
    start: usize,
    end: usize,
    time_mask: &[bool],
    time_hint: TimeHint,
) -> Option<ProposedRecord> {
    let quote = slice_tokens(orig, tokens, start, end)?;

    let zone_field = if let Some((zone, z_start, z_end)) = parse_body_zone(tokens, start, end)
        && let Some(z_quote) = slice_tokens(orig, tokens, z_start, z_end)
    {
        field_some("body_zone", json!(zone.as_str()), z_quote)
    } else {
        // Explicit general symptom like dizziness / nausea / fatigue -> BodyZone::Other
        let mut other_idx = None;
        for idx in start..end {
            if let Some(w) = word_at(tokens, idx)
                && matches!(
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
            {
                other_idx = Some(idx);
                break;
            }
        }
        if let Some(idx) = other_idx
            && let Some(z_quote) = slice_tokens(orig, tokens, idx, idx + 1)
        {
            field_some("body_zone", json!(BodyZone::Other.as_str()), z_quote)
        } else {
            field_none("body_zone")
        }
    };

    let mut score_field = field_none("score");
    if let Some(num) = find_number_in_range(tokens, start, end, time_mask, Indicator::Symptom)
        && num.value.fract() == 0.0
        && (0.0..=10.0).contains(&num.value)
        && let Some(int_v) = num.int_value.or_else(|| {
            (0_u64..=10).find(|&x| f64::from(u32::try_from(x).unwrap_or(u32::MAX)) == num.value)
        })
        && let Ok(sc_u8) = u8::try_from(int_v)
        && Score::try_from(sc_u8).is_ok()
        && let Some(sc_quote) = slice_from_bytes(orig, num.start_byte, num.end_byte)
    {
        score_field = field_some("score", json!(sc_u8), sc_quote);
    }

    let desc_quote = truncate_chars(&quote, 2000);
    let desc_field = field_some("description", json!(desc_quote), desc_quote);

    Some(ProposedRecord {
        indicator: Indicator::Symptom,
        fields: vec![zone_field, score_field, desc_field],
        time: time_hint,
        quote,
    })
}

fn is_bp_keyword_at(tokens: &[Token], idx: usize, end: usize) -> Option<usize> {
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

fn find_bp_pair(
    tokens: &[Token],
    start: usize,
    end: usize,
    time_mask: &[bool],
    has_keyword: bool,
) -> Option<(ParsedNumber, ParsedNumber)> {
    let mut i = start;
    while i < end {
        if time_mask.get(i).copied().unwrap_or(false) {
            i += 1;
            continue;
        }
        if let Some(n1) = parse_number(tokens, i)
            && n1.end_idx < end
        {
            let sep_idx = n1.end_idx;
            let is_slash = tokens
                .get(sep_idx)
                .is_some_and(|tk| tk.kind == TokenKind::Slash);
            let is_over = matches!(word_at(tokens, sep_idx), Some("sobre" | "over"));
            if (is_slash || is_over)
                && let Some(n2) = parse_number(tokens, sep_idx + 1)
                && n2.end_idx <= end
            {
                let any_time = (i..n2.end_idx).any(|k| time_mask.get(k).copied().unwrap_or(false));
                if !any_time {
                    let is_score_slash =
                        is_slash && n2.value == 10.0 && n1.value <= 10.0 && !has_keyword;
                    let plausible_standalone = n1.value >= 30.0 && n2.value >= 20.0;
                    if !is_score_slash && (has_keyword || plausible_standalone) {
                        return Some((n1, n2));
                    }
                }
            }
            i = n1.end_idx;
            continue;
        }
        i += 1;
    }
    None
}

fn extract_bp_record(
    orig: &str,
    tokens: &[Token],
    start: usize,
    end: usize,
    time_mask: &[bool],
    has_keyword: bool,
    time_hint: TimeHint,
) -> Option<ProposedRecord> {
    let quote = slice_tokens(orig, tokens, start, end)?;
    if let Some((sys, dia)) = find_bp_pair(tokens, start, end, time_mask, has_keyword) {
        let sys_field = if MmHg::try_from(sys.value).is_ok()
            && let Some(v) = number_to_json(sys)
            && let Some(q) = slice_from_bytes(orig, sys.start_byte, sys.end_byte)
        {
            field_some("systolic", v, q)
        } else {
            field_none("systolic")
        };
        let dia_field = if MmHg::try_from(dia.value).is_ok()
            && let Some(v) = number_to_json(dia)
            && let Some(q) = slice_from_bytes(orig, dia.start_byte, dia.end_byte)
        {
            field_some("diastolic", v, q)
        } else {
            field_none("diastolic")
        };
        return Some(ProposedRecord {
            indicator: Indicator::BloodPressure,
            fields: vec![sys_field, dia_field],
            time: time_hint,
            quote,
        });
    }

    if has_keyword {
        let sys_field = if let Some(sys) =
            find_number_in_range(tokens, start, end, time_mask, Indicator::BloodPressure)
            && MmHg::try_from(sys.value).is_ok()
            && let Some(v) = number_to_json(sys)
            && let Some(q) = slice_from_bytes(orig, sys.start_byte, sys.end_byte)
        {
            field_some("systolic", v, q)
        } else {
            field_none("systolic")
        };
        return Some(ProposedRecord {
            indicator: Indicator::BloodPressure,
            fields: vec![sys_field, field_none("diastolic")],
            time: time_hint,
            quote,
        });
    }
    None
}

struct NumericSpec {
    indicator: Indicator,
    field_key: &'static str,
}

struct ClauseCtx<'a> {
    orig: &'a str,
    tokens: &'a [Token],
    time_mask: &'a [bool],
}

#[derive(Default)]
struct ProposalOut {
    records: Vec<ProposedRecord>,
    record_start_bytes: Vec<usize>,
    negated: Vec<String>,
    uninterpreted: Vec<String>,
}

fn extract_single_numeric_record(
    ctx: &ClauseCtx<'_>,
    start: usize,
    end: usize,
    spec: NumericSpec,
    time_hint: TimeHint,
) -> Option<ProposedRecord> {
    let quote = slice_tokens(ctx.orig, ctx.tokens, start, end)?;
    let mut field = field_none(spec.field_key);

    if let Some(num) = find_number_in_range(ctx.tokens, start, end, ctx.time_mask, spec.indicator) {
        let valid = match spec.indicator {
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
            && let Some(val) = match spec.indicator {
                Indicator::Mood => num
                    .int_value
                    .and_then(|u| u8::try_from(u).ok())
                    .map(|u| json!(u)),
                _ => number_to_json(num),
            }
            && let Some(q) = slice_from_bytes(ctx.orig, num.start_byte, num.end_byte)
        {
            field = field_some(spec.field_key, val, q);
        }
    }

    Some(ProposedRecord {
        indicator: spec.indicator,
        fields: vec![field],
        time: time_hint,
        quote,
    })
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

fn detect_anchor(tokens: &[Token], idx: usize, end: usize) -> Option<(AnchorKind, usize)> {
    if is_bp_keyword_at(tokens, idx, end).is_some() {
        return Some((AnchorKind::BloodPressure, idx));
    }
    if starts_with_words(tokens, idx, &["heart", "rate"])
        || starts_with_words(tokens, idx, &["frecuencia", "cardiaca"])
        || starts_with_words(tokens, idx, &["ritmo", "cardiaco"])
    {
        return Some((AnchorKind::HeartRate, idx));
    }
    if starts_with_words(tokens, idx, &["body", "temperature"])
        || starts_with_words(tokens, idx, &["body", "temp"])
    {
        return Some((AnchorKind::BodyTemperature, idx));
    }
    if starts_with_words(tokens, idx, &["blood", "glucose"])
        || starts_with_words(tokens, idx, &["blood", "sugar"])
        || starts_with_words(tokens, idx, &["azucar", "en", "sangre"])
    {
        return Some((AnchorKind::Glucose, idx));
    }
    if starts_with_words(tokens, idx, &["oxygen", "saturation"])
        || starts_with_words(tokens, idx, &["blood", "oxygen"])
        || starts_with_words(tokens, idx, &["saturacion", "de", "oxigeno"])
        || starts_with_words(tokens, idx, &["oxigeno", "en", "sangre"])
        || starts_with_words(tokens, idx, &["o2", "sat"])
    {
        return Some((AnchorKind::OxygenInBlood, idx));
    }
    if starts_with_words(tokens, idx, &["estado", "de", "animo"]) {
        return Some((AnchorKind::Mood, idx));
    }

    if let Some(w) = word_at(tokens, idx) {
        if matches!(
            w,
            "pulso" | "pulse" | "hr" | "bpm" | "lpm" | "pulsaciones" | "latidos"
        ) {
            return Some((AnchorKind::HeartRate, idx));
        }
        if matches!(w, "temperatura" | "temp" | "fiebre" | "fever" | "febricula") {
            return Some((AnchorKind::BodyTemperature, idx));
        }
        if matches!(w, "glucosa" | "glucemia" | "glucose" | "azucar") {
            return Some((AnchorKind::Glucose, idx));
        }
        if matches!(
            w,
            "saturacion" | "saturation" | "oxigeno" | "oxygen" | "spo2" | "o2"
        ) {
            return Some((AnchorKind::OxygenInBlood, idx));
        }
        if matches!(w, "animo" | "mood" | "humor") {
            return Some((AnchorKind::Mood, idx));
        }
        if is_sleep_word(w) {
            return Some((AnchorKind::Sleep, idx));
        }
        if is_symptom_cue(w) {
            return Some((AnchorKind::Symptom, idx));
        }
    }

    if match_body_zone_phrase(tokens, idx, end).is_some() {
        return Some((AnchorKind::Symptom, idx));
    }

    // Standalone number with explicit unit: "120/80", "36.8°C", "95 mg/dl", "72 bpm", "6h"
    if let Some(num) = parse_number(tokens, idx)
        && num.end_idx < end
    {
        let next = num.end_idx;
        if tokens
            .get(next)
            .is_some_and(|tk| tk.kind == TokenKind::Slash)
            && parse_number(tokens, next + 1)
                .is_some_and(|n2| n2.end_idx <= end && n1_n2_bp_plausible(num.value, n2.value))
        {
            return Some((AnchorKind::BloodPressure, idx));
        }
        if matches!(
            word_at(tokens, next),
            Some("bpm" | "lpm" | "pulsaciones" | "latidos")
        ) {
            return Some((AnchorKind::HeartRate, idx));
        }
        if matches!(word_at(tokens, next), Some("celsius" | "centigrados"))
            || (tokens
                .get(next)
                .is_some_and(|tk| matches!(tk.norm.as_str(), "°" | "º"))
                && word_at(tokens, next + 1) == Some("c"))
        {
            return Some((AnchorKind::BodyTemperature, idx));
        }
        if matches!(word_at(tokens, next), Some("mg" | "mgdl")) {
            return Some((AnchorKind::Glucose, idx));
        }
        if is_hour_unit(word_at(tokens, next).unwrap_or("")) {
            return Some((AnchorKind::Sleep, idx));
        }
    }

    None
}

fn n1_n2_bp_plausible(sys: f64, dia: f64) -> bool {
    sys >= 30.0 && dia >= 20.0 && dia != 10.0
}

fn anchor_end_advance(tokens: &[Token], idx: usize, end: usize) -> usize {
    if is_bp_keyword_at(tokens, idx, end) == Some(2)
        || starts_with_words(tokens, idx, &["heart", "rate"])
        || starts_with_words(tokens, idx, &["frecuencia", "cardiaca"])
        || starts_with_words(tokens, idx, &["ritmo", "cardiaco"])
        || starts_with_words(tokens, idx, &["body", "temperature"])
        || starts_with_words(tokens, idx, &["body", "temp"])
        || starts_with_words(tokens, idx, &["blood", "glucose"])
        || starts_with_words(tokens, idx, &["blood", "sugar"])
        || starts_with_words(tokens, idx, &["oxygen", "saturation"])
        || starts_with_words(tokens, idx, &["blood", "oxygen"])
        || starts_with_words(tokens, idx, &["o2", "sat"])
    {
        return idx + 2;
    }
    if starts_with_words(tokens, idx, &["azucar", "en", "sangre"])
        || starts_with_words(tokens, idx, &["saturacion", "de", "oxigeno"])
        || starts_with_words(tokens, idx, &["oxigeno", "en", "sangre"])
        || starts_with_words(tokens, idx, &["estado", "de", "animo"])
    {
        return idx + 3;
    }
    if let Some((_, z_end)) = match_body_zone_phrase(tokens, idx, end) {
        return z_end;
    }
    idx + 1
}

fn process_clause(
    orig: &str,
    tokens: &[Token],
    c_start: usize,
    c_end: usize,
    carried_time: Option<&MatchedTime>,
    out: &mut ProposalOut,
) -> Option<MatchedTime> {
    let time_matches = find_time_hints(orig, tokens, c_start, c_end);
    let mut time_mask = vec![false; tokens.len()];
    for tm in &time_matches {
        for flag in time_mask.iter_mut().take(tm.end_idx).skip(tm.start_idx) {
            *flag = true;
        }
    }

    // Check if the entire clause is solely a time hint (e.g. "ayer, dormí mal")
    let has_non_time_word_or_num = (c_start..c_end).any(|i| {
        !time_mask.get(i).copied().unwrap_or(false)
            && tokens
                .get(i)
                .is_some_and(|t| matches!(t.kind, TokenKind::Word | TokenKind::Number))
    });
    if !has_non_time_word_or_num {
        return time_matches.first().cloned();
    }

    if is_symptom_or_indicator_negation(tokens, c_start, c_end, &time_mask) {
        if let Some(q) = slice_tokens(orig, tokens, c_start, c_end) {
            out.negated.push(q);
        }
        return None;
    }

    // Find all distinct anchors in the clause to support multiple indicators in one clause
    let mut anchors: Vec<(AnchorKind, usize)> = Vec::new();
    let mut i = c_start;
    while i < c_end {
        if time_mask.get(i).copied().unwrap_or(false) {
            i += 1;
            continue;
        }
        if let Some((kind, a_idx)) = detect_anchor(tokens, i, c_end) {
            if let Some(&(prev_kind, _)) = anchors.last()
                && (prev_kind == kind
                    && matches!(
                        kind,
                        AnchorKind::Symptom
                            | AnchorKind::Sleep
                            | AnchorKind::HeartRate
                            | AnchorKind::BloodPressure
                            | AnchorKind::BodyTemperature
                            | AnchorKind::Glucose
                            | AnchorKind::OxygenInBlood
                            | AnchorKind::Mood
                    ))
            {
                i = anchor_end_advance(tokens, i, c_end);
                continue;
            }
            anchors.push((kind, a_idx));
            i = anchor_end_advance(tokens, i, c_end);
        } else {
            i += 1;
        }
    }

    if anchors.is_empty() {
        if let Some(q) = slice_tokens(orig, tokens, c_start, c_end) {
            out.uninterpreted.push(q);
        }
        return None;
    }

    let ctx = ClauseCtx {
        orig,
        tokens,
        time_mask: &time_mask,
    };

    // Determine sub-ranges for each anchor
    let anchor_count = anchors.len();
    for (pos, &(kind, a_idx)) in anchors.iter().enumerate() {
        let seg_start = if pos == 0 { c_start } else { a_idx };
        let seg_end = if pos + 1 < anchor_count {
            anchors
                .get(pos + 1)
                .map_or(c_end, |&(_, next_idx)| next_idx)
        } else {
            c_end
        };

        // Check if a time hint overlaps or belongs to this sub-segment (or carried from previous)
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

        // If this segment has only duration like "5 hours" right after an earlier Sleep without hours, merge!
        if kind == AnchorKind::Sleep
            && anchors.len() == 1
            && let Some(prev) = out.records.last_mut()
            && prev.indicator == Indicator::Sleep
            && prev
                .fields
                .iter()
                .any(|f| f.key == "hours" && f.value.is_none())
            && !((seg_start..seg_end).any(|k| word_at(tokens, k).is_some_and(is_sleep_word)))
            && let Some(extra) = extract_sleep_record(
                orig,
                tokens,
                seg_start,
                seg_end,
                &time_mask,
                time_hint.clone(),
            )
            && let Some(extra_hours) = extra.fields.iter().find(|f| f.key == "hours")
            && extra_hours.value.is_some()
        {
            for f in &mut prev.fields {
                if f.key == "hours" {
                    *f = extra_hours.clone();
                }
            }
            if let Some(&prev_start) = out.record_start_bytes.last()
                && let Some(end_tk) = tokens.get(seg_end.saturating_sub(1))
                && let Some(merged_quote) = slice_from_bytes(orig, prev_start, end_tk.end)
            {
                prev.quote = merged_quote;
            }
            continue;
        }

        let rec = match kind {
            AnchorKind::HeartRate => extract_single_numeric_record(
                &ctx,
                seg_start,
                seg_end,
                NumericSpec {
                    indicator: Indicator::HeartRate,
                    field_key: "bpm",
                },
                time_hint,
            ),
            AnchorKind::BloodPressure => {
                let has_kw =
                    (seg_start..seg_end).any(|k| is_bp_keyword_at(tokens, k, seg_end).is_some());
                extract_bp_record(
                    orig, tokens, seg_start, seg_end, &time_mask, has_kw, time_hint,
                )
            }
            AnchorKind::BodyTemperature => extract_single_numeric_record(
                &ctx,
                seg_start,
                seg_end,
                NumericSpec {
                    indicator: Indicator::BodyTemperature,
                    field_key: "celsius",
                },
                time_hint,
            ),
            AnchorKind::Glucose => extract_single_numeric_record(
                &ctx,
                seg_start,
                seg_end,
                NumericSpec {
                    indicator: Indicator::Glucose,
                    field_key: "mg_dl",
                },
                time_hint,
            ),
            AnchorKind::OxygenInBlood => extract_single_numeric_record(
                &ctx,
                seg_start,
                seg_end,
                NumericSpec {
                    indicator: Indicator::OxygenInBlood,
                    field_key: "percentage",
                },
                time_hint,
            ),
            AnchorKind::Mood => extract_single_numeric_record(
                &ctx,
                seg_start,
                seg_end,
                NumericSpec {
                    indicator: Indicator::Mood,
                    field_key: "score",
                },
                time_hint,
            ),
            AnchorKind::Sleep => {
                extract_sleep_record(orig, tokens, seg_start, seg_end, &time_mask, time_hint)
            }
            AnchorKind::Symptom => {
                extract_symptom_record(orig, tokens, seg_start, seg_end, &time_mask, time_hint)
            }
        };

        if let Some(r) = rec {
            let start_b = tokens.get(seg_start).map_or(0, |t| t.start);
            out.records.push(r);
            out.record_start_bytes.push(start_b);
        } else if let Some(q) = slice_tokens(orig, tokens, seg_start, seg_end) {
            out.uninterpreted.push(q);
        }
    }

    None
}

/// Interpreta texto libre (español, inglés o mezclado) mediante reglas deterministas.
pub fn interpret(text: &str) -> Proposal {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Proposal {
            schema_version: PROPOSAL_SCHEMA_VERSION,
            interpreter: InterpreterKind::Rules,
            interpreter_version: RULES_INTERPRETER_VERSION.to_owned(),
            records: Vec::new(),
            negated: Vec::new(),
            uninterpreted: Vec::new(),
        };
    }

    let tokens = tokenize(text);
    let clauses = split_clauses(&tokens);

    let mut out = ProposalOut::default();
    let mut carried_time: Option<MatchedTime> = None;

    for (c_start, c_end) in clauses {
        let next_carried = process_clause(
            text,
            &tokens,
            c_start,
            c_end,
            carried_time.as_ref(),
            &mut out,
        );
        carried_time = next_carried;
    }

    if let Some(leftover_time) = carried_time {
        out.uninterpreted.push(leftover_time.quote);
    }

    // Defense-in-depth: ensure any complete record passes Measurement::validate
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn field_val<'a>(rec: &'a ProposedRecord, key: &str) -> Option<&'a Value> {
        rec.fields
            .iter()
            .find(|f| f.key == key)
            .and_then(|f| f.value.as_ref())
    }

    fn assert_valid(text: &str) -> Proposal {
        let p = interpret(text);
        let issues = p.check(text);
        assert!(issues.is_empty(), "check failed for {text:?}: {issues:?}");
        assert_eq!(p.schema_version, PROPOSAL_SCHEMA_VERSION);
        assert_eq!(p.interpreter, InterpreterKind::Rules);
        assert_eq!(p.interpreter_version, RULES_INTERPRETER_VERSION);
        p
    }

    #[test]
    fn heart_rate_cases() {
        // 1
        let p = assert_valid("pulso 72");
        assert_eq!(p.records.len(), 1);
        assert_eq!(p.records[0].indicator, Indicator::HeartRate);
        assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));

        // 2
        let p = assert_valid("pulso setenta y dos");
        assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));
        assert_eq!(
            p.records[0].fields[0].quote.as_deref(),
            Some("setenta y dos")
        );

        // 3
        let p = assert_valid("Frecuencia cardíaca ciento setenta y dos");
        assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(172)));
        assert_eq!(
            p.records[0].fields[0].quote.as_deref(),
            Some("ciento setenta y dos")
        );

        // 4
        let p = assert_valid("My heart rate is seventy two bpm");
        assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));

        // 5
        let p = assert_valid("HR one hundred seventy two");
        assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(172)));

        // 6
        let p = assert_valid("84 lpm");
        assert_eq!(p.records[0].indicator, Indicator::HeartRate);
        assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(84)));
    }

    #[test]
    fn six_vs_sixteen_and_seventy_two_vs_one_hundred_seventy_two() {
        // 7
        let p = assert_valid("me duele la cabeza un seis");
        assert_eq!(field_val(&p.records[0], "score"), Some(&json!(6)));

        // 8: 16 is out of symptom score range 0..=10, must NOT match 6!
        let p = assert_valid("me duele la cabeza dieciséis");
        assert_eq!(p.records[0].indicator, Indicator::Symptom);
        assert_eq!(field_val(&p.records[0], "score"), None);

        // 9
        let p = assert_valid("pulso dieciséis");
        assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(16)));

        // 10
        let p = assert_valid("my left foot hurts six");
        assert_eq!(field_val(&p.records[0], "score"), Some(&json!(6)));

        // 11: sixteen must NOT match 6!
        let p = assert_valid("my left foot hurts sixteen");
        assert_eq!(field_val(&p.records[0], "score"), None);

        // 12
        let p = assert_valid("mood six");
        assert_eq!(field_val(&p.records[0], "score"), Some(&json!(6)));

        // 13
        let p = assert_valid("pulse sixteen");
        assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(16)));
    }

    #[test]
    fn blood_pressure_cases() {
        // 14
        let p = assert_valid("presión 120/80");
        assert_eq!(p.records[0].indicator, Indicator::BloodPressure);
        assert_eq!(field_val(&p.records[0], "systolic"), Some(&json!(120)));
        assert_eq!(field_val(&p.records[0], "diastolic"), Some(&json!(80)));

        // 15
        let p = assert_valid("tensión ciento veinte sobre ochenta");
        assert_eq!(field_val(&p.records[0], "systolic"), Some(&json!(120)));
        assert_eq!(field_val(&p.records[0], "diastolic"), Some(&json!(80)));

        // 16
        let p = assert_valid("blood pressure 118 over 76 mmHg");
        assert_eq!(field_val(&p.records[0], "systolic"), Some(&json!(118)));
        assert_eq!(field_val(&p.records[0], "diastolic"), Some(&json!(76)));

        // 17: standalone 120/80
        let p = assert_valid("120/80");
        assert_eq!(p.records[0].indicator, Indicator::BloodPressure);
        assert_eq!(field_val(&p.records[0], "systolic"), Some(&json!(120)));
        assert_eq!(field_val(&p.records[0], "diastolic"), Some(&json!(80)));

        // 18: partial BP
        let p = assert_valid("presión 130");
        assert_eq!(p.records[0].indicator, Indicator::BloodPressure);
        assert_eq!(field_val(&p.records[0], "systolic"), Some(&json!(130)));
        assert_eq!(field_val(&p.records[0], "diastolic"), None);
    }

    #[test]
    fn temperature_glucose_oxygen_mood_cases() {
        // 19
        let p = assert_valid("temperatura treinta y seis coma ocho");
        assert_eq!(p.records[0].indicator, Indicator::BodyTemperature);
        assert_eq!(field_val(&p.records[0], "celsius"), Some(&json!(36.8)));

        // 20
        let p = assert_valid("36,8 °C");
        assert_eq!(p.records[0].indicator, Indicator::BodyTemperature);
        assert_eq!(field_val(&p.records[0], "celsius"), Some(&json!(36.8)));

        // 21
        let p = assert_valid("Body temperature 37.4 celsius");
        assert_eq!(p.records[0].indicator, Indicator::BodyTemperature);
        assert_eq!(field_val(&p.records[0], "celsius"), Some(&json!(37.4)));

        // 22
        let p = assert_valid("glucosa 95 mg/dL");
        assert_eq!(p.records[0].indicator, Indicator::Glucose);
        assert_eq!(field_val(&p.records[0], "mg_dl"), Some(&json!(95)));

        // 23
        let p = assert_valid("blood sugar one hundred ten");
        assert_eq!(p.records[0].indicator, Indicator::Glucose);
        assert_eq!(field_val(&p.records[0], "mg_dl"), Some(&json!(110)));

        // 24
        let p = assert_valid("SpO₂ 98.2%");
        assert_eq!(p.records[0].indicator, Indicator::OxygenInBlood);
        assert_eq!(field_val(&p.records[0], "percentage"), Some(&json!(98.2)));

        // 25
        let p = assert_valid("saturación de oxígeno noventa y nueve");
        assert_eq!(p.records[0].indicator, Indicator::OxygenInBlood);
        assert_eq!(field_val(&p.records[0], "percentage"), Some(&json!(99)));

        // 26
        let p = assert_valid("ánimo 6");
        assert_eq!(p.records[0].indicator, Indicator::Mood);
        assert_eq!(field_val(&p.records[0], "score"), Some(&json!(6)));

        // 27
        let p = assert_valid("mood 8/10");
        assert_eq!(p.records[0].indicator, Indicator::Mood);
        assert_eq!(field_val(&p.records[0], "score"), Some(&json!(8)));
    }

    #[test]
    fn sleep_cases() {
        // 28
        let p = assert_valid("dormí cinco horas y media");
        let rec = &p.records[0];
        assert_eq!(rec.indicator, Indicator::Sleep);
        assert_eq!(field_val(rec, "hours"), Some(&json!(5.5)));
        assert_eq!(field_val(rec, "quality"), None);
        assert_eq!(field_val(rec, "window_start"), None);
        assert_eq!(field_val(rec, "window_end"), None);
        assert!(!rec.is_complete());

        // 29
        let p = assert_valid("slept 6h");
        assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(6)));
        assert_eq!(field_val(&p.records[0], "window_start"), None);
        assert_eq!(field_val(&p.records[0], "window_end"), None);

        // 30
        let p = assert_valid("dormí mal");
        assert_eq!(field_val(&p.records[0], "hours"), None);
        assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));
        assert_eq!(field_val(&p.records[0], "window_start"), None);

        // 31
        let p = assert_valid("slept badly");
        assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));

        // 32
        let p = assert_valid("no dormí bien");
        assert!(p.negated.is_empty());
        assert_eq!(p.records[0].indicator, Indicator::Sleep);
        assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));

        // 33
        let p = assert_valid("sueño regular 7 horas");
        assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("fair")));
        assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(7)));

        // 34
        let p = assert_valid("slept well for 8 hours");
        assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("good")));
        assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(8)));
    }

    #[test]
    fn symptom_and_time_hint_cases() {
        // 35
        let p = assert_valid("me duele la cabeza un 6");
        let rec = &p.records[0];
        assert_eq!(rec.indicator, Indicator::Symptom);
        assert_eq!(field_val(rec, "body_zone"), Some(&json!("head")));
        assert_eq!(field_val(rec, "score"), Some(&json!(6)));
        assert_eq!(
            field_val(rec, "description"),
            Some(&json!("me duele la cabeza un 6"))
        );

        // 36
        let p = assert_valid("dolor de espalda 4/10");
        assert_eq!(field_val(&p.records[0], "body_zone"), Some(&json!("back")));
        assert_eq!(field_val(&p.records[0], "score"), Some(&json!(4)));

        // 37
        let p = assert_valid("my left foot hurts 3");
        assert_eq!(
            field_val(&p.records[0], "body_zone"),
            Some(&json!("left_foot"))
        );
        assert_eq!(field_val(&p.records[0], "score"), Some(&json!(3)));

        // 38: symptom without score
        let p = assert_valid("me duele el pecho");
        assert_eq!(field_val(&p.records[0], "body_zone"), Some(&json!("chest")));
        assert_eq!(field_val(&p.records[0], "score"), None);

        // 39: time hint ES
        let p = assert_valid("me duele la cabeza un 6 desde esta mañana");
        assert_eq!(
            p.records[0].time,
            TimeHint::Ambiguous {
                quote: "desde esta mañana".to_owned()
            }
        );

        // 40: time hint EN
        let p = assert_valid("headache 5 since this morning");
        assert_eq!(
            p.records[0].time,
            TimeHint::Ambiguous {
                quote: "since this morning".to_owned()
            }
        );

        // 41: time hint with number does not leak into score
        let p = assert_valid("me duele la cabeza hace 2 horas");
        assert_eq!(field_val(&p.records[0], "score"), None);
        assert_eq!(
            p.records[0].time,
            TimeHint::Ambiguous {
                quote: "hace 2 horas".to_owned()
            }
        );
    }

    #[test]
    fn negations_mixed_multiple_and_uninterpreted_cases() {
        // 42
        let p = assert_valid("no me duele la cabeza");
        assert!(p.records.is_empty());
        assert_eq!(p.negated, vec!["no me duele la cabeza"]);

        // 43
        let p = assert_valid("ya no me duele");
        assert!(p.records.is_empty());
        assert_eq!(p.negated, vec!["ya no me duele"]);

        // 44
        let p = assert_valid("no headache");
        assert!(p.records.is_empty());
        assert_eq!(p.negated, vec!["no headache"]);

        // 45
        let p = assert_valid("ayer dormí mal pero hoy ya no me duele");
        assert_eq!(p.records.len(), 1);
        assert_eq!(p.records[0].indicator, Indicator::Sleep);
        assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));
        assert_eq!(
            p.records[0].time,
            TimeHint::Ambiguous {
                quote: "ayer".to_owned()
            }
        );
        assert_eq!(p.negated, vec!["hoy ya no me duele"]);

        // 46: multiple indicators in one phrase + time hint on first only
        let p = assert_valid("me duele la cabeza un 6 desde esta mañana y dormí mal");
        assert_eq!(p.records.len(), 2);
        assert_eq!(p.records[0].indicator, Indicator::Symptom);
        assert_eq!(
            p.records[0].time,
            TimeHint::Ambiguous {
                quote: "desde esta mañana".to_owned()
            }
        );
        assert_eq!(p.records[1].indicator, Indicator::Sleep);
        assert_eq!(p.records[1].time, TimeHint::Unspecified);

        // 47: multiple indicators without comma
        let p = assert_valid("pulso 72 temperatura 36.8 SpO₂ 98%");
        assert_eq!(p.records.len(), 3);
        assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));
        assert_eq!(field_val(&p.records[1], "celsius"), Some(&json!(36.8)));
        assert_eq!(field_val(&p.records[2], "percentage"), Some(&json!(98)));

        // 48: mixed ES/EN
        let p = assert_valid("tengo heart rate setenta y dos and dolor de right arm 5");
        assert_eq!(p.records.len(), 2);
        assert_eq!(field_val(&p.records[0], "bpm"), Some(&json!(72)));
        assert_eq!(
            field_val(&p.records[1], "body_zone"),
            Some(&json!("right_arm"))
        );
        assert_eq!(field_val(&p.records[1], "score"), Some(&json!(5)));

        // 49: case & accents preserved in literal quote
        let p = assert_valid("TENÍA ÁNIMO 6 y PRESIÓN 120/80");
        assert_eq!(p.records.len(), 2);
        assert_eq!(p.records[0].quote, "TENÍA ÁNIMO 6");
        assert_eq!(p.records[1].quote, "PRESIÓN 120/80");

        // 50: completely uninterpretable
        let p = assert_valid("  todo va tranquilo en la estación  ");
        assert!(p.records.is_empty());
        assert!(p.negated.is_empty());
        assert_eq!(p.uninterpreted, vec!["todo va tranquilo en la estación"]);

        // 51: partial interpretable + uninterpretable fragment
        let p = assert_valid("pulso 72, la escotilla hace ruido");
        assert_eq!(p.records.len(), 1);
        assert_eq!(p.uninterpreted, vec!["la escotilla hace ruido"]);

        // 52: empty / whitespace
        let p = assert_valid("   ");
        assert!(p.records.is_empty());
        assert!(p.negated.is_empty());
        assert!(p.uninterpreted.is_empty());
    }

    #[test]
    fn out_of_range_numbers_are_never_forced() {
        let p = assert_valid("pulso 0");
        assert_eq!(field_val(&p.records[0], "bpm"), None);

        let p = assert_valid("saturación 150%");
        assert_eq!(field_val(&p.records[0], "percentage"), None);

        let p = assert_valid("ánimo 12");
        assert_eq!(field_val(&p.records[0], "score"), None);

        let p = assert_valid("dormí 30 horas");
        assert_eq!(field_val(&p.records[0], "hours"), None);

        let p = assert_valid("me duele la cabeza un 4.5");
        assert_eq!(field_val(&p.records[0], "body_zone"), Some(&json!("head")));
        assert_eq!(field_val(&p.records[0], "score"), None);
    }

    #[test]
    fn extra_sleep_negation_and_time_cases() {
        let p = assert_valid("I didn't sleep well");
        assert!(p.negated.is_empty());
        assert_eq!(p.records[0].indicator, Indicator::Sleep);
        assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));

        let p = assert_valid("last night slept 6.5 hours");
        assert_eq!(p.records[0].indicator, Indicator::Sleep);
        assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(6.5)));
        assert_eq!(
            p.records[0].time,
            TimeHint::Ambiguous {
                quote: "last night".to_owned()
            }
        );

        let p = assert_valid("dormí mal, 5 horas");
        assert_eq!(p.records.len(), 1);
        assert_eq!(field_val(&p.records[0], "quality"), Some(&json!("poor")));
        assert_eq!(field_val(&p.records[0], "hours"), Some(&json!(5)));

        let p = assert_valid("sin dolor");
        assert!(p.records.is_empty());
        assert_eq!(p.negated, vec!["sin dolor"]);

        let p = assert_valid("no tengo fiebre");
        assert!(p.records.is_empty());
        assert_eq!(p.negated, vec!["no tengo fiebre"]);

        let p = assert_valid("headache is gone");
        assert!(p.records.is_empty());
        assert_eq!(p.negated, vec!["headache is gone"]);

        let p = assert_valid("my head doesn't hurt");
        assert!(p.records.is_empty());
        assert_eq!(p.negated, vec!["my head doesn't hurt"]);
    }

    fn quote_contains_number_in_digits_or_words(quote: &str, expected: f64) -> bool {
        let tokens = tokenize(quote);
        for i in 0..tokens.len() {
            if let Some(num) = parse_number(&tokens, i) {
                if (num.value - expected).abs() < 1e-6 {
                    return true;
                }
                if num.end_idx < tokens.len()
                    && word_at(&tokens, num.end_idx).is_some_and(is_hour_unit)
                {
                    if (starts_with_words(&tokens, num.end_idx + 1, &["y", "media"])
                        || starts_with_words(&tokens, num.end_idx + 1, &["y", "medio"])
                        || starts_with_words(&tokens, num.end_idx + 1, &["and", "a", "half"]))
                        && (num.value + 0.5 - expected).abs() < 1e-6
                    {
                        return true;
                    }
                    if (word_at(&tokens, num.end_idx + 1) == Some("y")
                        || word_at(&tokens, num.end_idx + 1) == Some("and"))
                        && let Some(mins) = parse_number(&tokens, num.end_idx + 2)
                    {
                        let approx = num.value + (mins.value / 60.0 * 100.0).round() / 100.0;
                        if (approx - expected).abs() < 1e-6 {
                            return true;
                        }
                    }
                }
            }
            if (starts_with_words(&tokens, i, &["media", "hora"])
                || starts_with_words(&tokens, i, &["half", "hour"])
                || starts_with_words(&tokens, i, &["half", "an", "hour"]))
                && (expected - 0.5).abs() < 1e-6
            {
                return true;
            }
        }
        false
    }

    fn check_proposal_invariants(s: &str) -> Result<(), proptest::test_runner::TestCaseError> {
        let p = interpret(s);
        let issues = p.check(s);
        prop_assert!(issues.is_empty(), "issues: {issues:?}");

        for rec in &p.records {
            for f in &rec.fields {
                if let (Some(Value::Number(n)), Some(q)) = (&f.value, &f.quote)
                    && let Some(num_f64) = n.as_f64()
                {
                    prop_assert!(s.contains(q.as_str()));
                    prop_assert!(
                        quote_contains_number_in_digits_or_words(q, num_f64),
                        "numeric value {num_f64} not backed by quote {q:?}"
                    );
                }
            }
        }
        Ok(())
    }

    proptest! {
        #[test]
        fn arbitrary_strings_never_panic_and_pass_check(s in ".*") {
            check_proposal_invariants(&s)?;
        }

        #[test]
        fn long_and_exotic_unicode_strings_never_panic_and_pass_check(
            parts in prop::collection::vec(
                prop_oneof![
                    Just("pulso 72 ".to_owned()),
                    Just("me duele la cabeza un 6 ".to_owned()),
                    Just("dormí cinco horas y media ".to_owned()),
                    Just("no me duele ".to_owned()),
                    Just("İ\u{0307}ßSpO₂ 98,2% ".to_owned()),
                    Just("🤒🔥💤 ".to_owned()),
                    ".{0,120}",
                ],
                1..80
            )
        ) {
            let s = parts.concat();
            check_proposal_invariants(&s)?;
        }
    }
}
