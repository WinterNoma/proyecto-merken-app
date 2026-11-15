//! Análisis de números en cifras y palabras (ES/EN), decimales, signos y fracciones.

use super::lexicon::{Token, TokenKind, starts_with_words, word_at};
use serde_json::{Number, Value};

#[derive(Clone, Copy, Debug)]
pub(super) struct ParsedNumber {
    pub(super) value: f64,
    pub(super) int_value: Option<u64>,
    pub(super) end_idx: usize,
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum NumberScan {
    Valid(ParsedNumber),
    Rejected { end_idx: usize },
}

pub(super) fn es_unit(w: &str) -> Option<u64> {
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

pub(super) fn es_unit_in_compound(w: &str) -> Option<u64> {
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

pub(super) fn es_ten(w: &str) -> Option<u64> {
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

pub(super) fn en_sub_hundred(tokens: &[Token], idx: usize) -> Option<(u64, usize)> {
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

fn is_unsupported_number_head(w: &str) -> bool {
    matches!(
        w,
        "trescientos"
            | "trescientas"
            | "cuatrocientos"
            | "cuatrocientas"
            | "quinientos"
            | "quinientas"
            | "seiscientos"
            | "seiscientas"
            | "setecientos"
            | "setecientas"
            | "ochocientos"
            | "ochocientas"
            | "novecientos"
            | "novecientas"
            | "cientos"
            | "cientas"
            | "mil"
            | "millon"
            | "millones"
            | "billon"
            | "billones"
            | "thousand"
            | "thousands"
            | "million"
            | "millions"
            | "billion"
            | "billions"
            | "hundreds"
    )
}

fn is_multiplier_after_number(w: &str) -> bool {
    is_unsupported_number_head(w) || w == "hundred"
}

fn is_any_number_word(w: &str) -> bool {
    is_unsupported_number_head(w)
        || matches!(
            w,
            "cero" | "zero" | "cien" | "ciento" | "doscientos" | "doscientas" | "hundred" | "un"
        )
        || es_unit(w).is_some()
        || es_teen(w).is_some()
        || es_ten(w).is_some()
        || en_unit(w).is_some()
        || en_teen(w).is_some()
        || en_ten(w).is_some()
}

fn consume_trailing_number_phrase(tokens: &[Token], start: usize, end: usize) -> usize {
    let mut i = start;
    while i < end {
        let Some(tk) = tokens.get(i) else {
            break;
        };
        match tk.kind {
            TokenKind::Number | TokenKind::InvalidNumber => {
                i += 1;
            }
            TokenKind::Word => {
                let w = tk.norm.as_str();
                let next_after_conj = matches!(w, "y" | "and")
                    && i + 1 < end
                    && (tokens.get(i + 1).is_some_and(|n| {
                        matches!(n.kind, TokenKind::Number | TokenKind::InvalidNumber)
                    }) || word_at(tokens, i + 1).is_some_and(|nw| {
                        is_any_number_word(nw) || matches!(nw, "media" | "medio" | "a" | "half")
                    }));
                if is_any_number_word(w)
                    || matches!(
                        w,
                        "coma" | "punto" | "con" | "point" | "dot" | "media" | "medio" | "half"
                    )
                    || next_after_conj
                    || (w == "a" && word_at(tokens, i + 1) == Some("half"))
                {
                    i += 1;
                } else {
                    break;
                }
            }
            TokenKind::Hyphen
                if i + 1 < end
                    && (word_at(tokens, i + 1).is_some_and(is_any_number_word)
                        || tokens
                            .get(i + 1)
                            .is_some_and(|n| n.kind == TokenKind::Number)) =>
            {
                i += 1;
            }
            _ => break,
        }
    }
    i
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

/// Analiza un entero sin signo en cifras o palabras (0..=299), sin consumir
/// decimales ni sufijos como `"y media"`. Útil para horas del reloj.
pub(super) fn parse_int_word_or_digits(tokens: &[Token], idx: usize) -> Option<(u64, usize)> {
    let first = tokens.get(idx)?;
    if first.kind == TokenKind::Number && !first.norm.contains('.') {
        return first.norm.parse::<u64>().ok().map(|v| (v, idx + 1));
    }
    parse_es_int(tokens, idx).or_else(|| parse_en_int(tokens, idx))
}

/// Analiza un número sin consumir fracciones `/` ni detectar rechazos compuestos.
/// Útil para pares `sys/dia` de presión arterial y análisis de horas.
pub(super) fn parse_simple_number(tokens: &[Token], idx: usize) -> Option<ParsedNumber> {
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

/// Analiza una expresión numérica completa en `tokens[idx..end]`.
///
/// Rechaza de forma atómica (consumiendo toda la expresión para que no se reutilicen
/// subfragmentos ni denominadores):
/// - números malformados (`36.8.1`) o con signo (`-6`, `+6`, `menos 6`, `minus 6`),
/// - expresiones con magnitudes no soportadas (`trescientos setenta y dos`, `mil`, `three hundred`),
/// - fracciones distintas de `n/10` (o cualquier fracción si `!allow_fraction_10`).
pub(super) fn scan_number(
    tokens: &[Token],
    idx: usize,
    end: usize,
    allow_fraction_10: bool,
) -> Option<NumberScan> {
    if idx >= end {
        return None;
    }
    let tk = tokens.get(idx)?;

    if tk.kind == TokenKind::InvalidNumber {
        let after = consume_trailing_number_phrase(tokens, idx + 1, end);
        if after + 1 < end
            && tokens
                .get(after)
                .is_some_and(|s| s.kind == TokenKind::Slash)
            && let Some(denom) = parse_simple_number(tokens, after + 1)
            && denom.end_idx <= end
        {
            return Some(NumberScan::Rejected {
                end_idx: denom.end_idx,
            });
        }
        return Some(NumberScan::Rejected { end_idx: after });
    }

    // Signo separado antes de número: "- 6", "menos 6", "minus 6", "negative 6"
    let is_word_sign = word_at(tokens, idx).is_some_and(|w| {
        matches!(w, "minus" | "negative" | "negativo")
            || (w == "menos"
                && !idx
                    .checked_sub(1)
                    .and_then(|p| word_at(tokens, p))
                    .is_some_and(|pw| matches!(pw, "al" | "por" | "lo")))
    });
    let is_sym_sign = tk.kind == TokenKind::Hyphen || tk.norm == "+";
    if (is_word_sign || is_sym_sign) && idx + 1 < end {
        let next_is_num_start = tokens
            .get(idx + 1)
            .is_some_and(|n| matches!(n.kind, TokenKind::Number | TokenKind::InvalidNumber))
            || word_at(tokens, idx + 1).is_some_and(is_any_number_word);
        if next_is_num_start {
            let after = consume_trailing_number_phrase(tokens, idx + 1, end);
            return Some(NumberScan::Rejected { end_idx: after });
        }
    }

    if let Some(w) = word_at(tokens, idx)
        && is_unsupported_number_head(w)
    {
        let after = consume_trailing_number_phrase(tokens, idx + 1, end);
        return Some(NumberScan::Rejected { end_idx: after });
    }

    let num = parse_simple_number(tokens, idx)?;
    if num.end_idx > end {
        return None;
    }

    // Si sigue una magnitud no soportada ("three hundred", "dos mil", "ciento veinte mil"), rechaza todo.
    if num.end_idx < end && word_at(tokens, num.end_idx).is_some_and(is_multiplier_after_number) {
        let after = consume_trailing_number_phrase(tokens, num.end_idx + 1, end);
        return Some(NumberScan::Rejected { end_idx: after });
    }

    // Si sigue un separador decimal seguido de una magnitud no soportada, rechaza todo.
    if num.int_value.is_some()
        && num.end_idx + 1 < end
        && word_at(tokens, num.end_idx)
            .is_some_and(|sep| matches!(sep, "coma" | "punto" | "con" | "point" | "dot"))
        && word_at(tokens, num.end_idx + 1).is_some_and(is_unsupported_number_head)
    {
        let after = consume_trailing_number_phrase(tokens, num.end_idx + 2, end);
        return Some(NumberScan::Rejected { end_idx: after });
    }

    // Si sigue "/" + número, consume la fracción completa como unidad.
    if num.end_idx + 1 < end
        && tokens
            .get(num.end_idx)
            .is_some_and(|s| s.kind == TokenKind::Slash)
    {
        let denom_idx = num.end_idx + 1;
        if let Some(denom) = parse_simple_number(tokens, denom_idx)
            && denom.end_idx <= end
        {
            // Rechaza cadenas con múltiples "/" (p. ej. fechas "06/11/80" o "6/7/8").
            let mut chain_end = denom.end_idx;
            let mut chained = false;
            while chain_end + 1 < end
                && tokens
                    .get(chain_end)
                    .is_some_and(|s| s.kind == TokenKind::Slash)
            {
                if let Some(next_part) = parse_simple_number(tokens, chain_end + 1)
                    && next_part.end_idx <= end
                {
                    chained = true;
                    chain_end = next_part.end_idx;
                } else if tokens
                    .get(chain_end + 1)
                    .is_some_and(|d| d.kind == TokenKind::InvalidNumber)
                {
                    chained = true;
                    chain_end += 2;
                } else {
                    break;
                }
            }
            if chained {
                return Some(NumberScan::Rejected { end_idx: chain_end });
            }
            if allow_fraction_10
                && denom.int_value == Some(10)
                && num.int_value.is_some_and(|n| n <= 10)
            {
                return Some(NumberScan::Valid(ParsedNumber {
                    value: num.value,
                    int_value: num.int_value,
                    end_idx: denom.end_idx,
                    start_byte: num.start_byte,
                    end_byte: denom.end_byte,
                }));
            }
            return Some(NumberScan::Rejected {
                end_idx: denom.end_idx,
            });
        }
        if tokens
            .get(denom_idx)
            .is_some_and(|d| d.kind == TokenKind::InvalidNumber)
        {
            return Some(NumberScan::Rejected {
                end_idx: denom_idx + 1,
            });
        }
    }

    Some(NumberScan::Valid(num))
}

pub(super) fn number_to_json(num: ParsedNumber) -> Option<Value> {
    if let Some(u) = num.int_value {
        return Some(Value::from(u));
    }
    Number::from_f64(num.value).map(Value::Number)
}
