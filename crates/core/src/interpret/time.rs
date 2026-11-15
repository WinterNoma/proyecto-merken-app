//! Detección de expresiones temporales y horas del reloj (siempre `TimeHint::Ambiguous`).

use super::{
    lexicon::{Token, TokenKind, slice_tokens, starts_with_words, word_at},
    numbers::{parse_int_word_or_digits, parse_simple_number},
};

#[derive(Clone, Debug)]
pub(super) struct MatchedTime {
    pub(super) start_idx: usize,
    pub(super) end_idx: usize,
    pub(super) quote: String,
}

/// Máscara booleana local a una cláusula (`[c_start..c_end)`), evitando coste cuadrático.
pub(super) struct TimeMask {
    offset: usize,
    flags: Vec<bool>,
}

impl TimeMask {
    pub(super) fn from_matches(c_start: usize, c_end: usize, matches: &[MatchedTime]) -> Self {
        let len = c_end.saturating_sub(c_start);
        let mut flags = vec![false; len];
        for tm in matches {
            let rel_start = tm.start_idx.saturating_sub(c_start);
            let rel_end = tm.end_idx.saturating_sub(c_start).min(len);
            for flag in flags.iter_mut().take(rel_end).skip(rel_start) {
                *flag = true;
            }
        }
        Self {
            offset: c_start,
            flags,
        }
    }

    pub(super) fn is_masked(&self, idx: usize) -> bool {
        idx.checked_sub(self.offset)
            .and_then(|rel| self.flags.get(rel).copied())
            .unwrap_or(false)
    }

    pub(super) fn any_masked(&self, start: usize, end: usize) -> bool {
        (start..end).any(|i| self.is_masked(i))
    }
}

fn is_colon_at(tokens: &[Token], idx: usize) -> bool {
    tokens
        .get(idx)
        .is_some_and(|tk| tk.kind == TokenKind::Other && tk.norm == ":")
}

fn is_two_digit_minute(tokens: &[Token], idx: usize) -> bool {
    tokens.get(idx).is_some_and(|tk| {
        tk.kind == TokenKind::Number
            && !tk.norm.contains('.')
            && tk.norm.len() == 2
            && tk.norm.parse::<u64>().is_ok_and(|m| m < 60)
    })
}

fn is_non_clock_unit(tokens: &[Token], idx: usize, end: usize) -> bool {
    if idx >= end {
        return false;
    }
    if tokens
        .get(idx)
        .is_some_and(|tk| matches!(tk.kind, TokenKind::Slash | TokenKind::Percent))
    {
        return true;
    }
    word_at(tokens, idx).is_some_and(|w| {
        matches!(
            w,
            "bpm"
                | "lpm"
                | "pulsaciones"
                | "latidos"
                | "mmhg"
                | "celsius"
                | "centigrados"
                | "grados"
                | "degrees"
                | "f"
                | "fahrenheit"
                | "mg"
                | "mgdl"
                | "mmol"
                | "h"
                | "hr"
                | "hrs"
                | "hora"
                | "horas"
                | "hour"
                | "hours"
                | "min"
                | "mins"
                | "minuto"
                | "minutos"
                | "minute"
                | "minutes"
                | "kcal"
                | "km"
                | "usv"
                | "ejercicios"
                | "ejercicio"
                | "exercises"
                | "exercise"
                | "veces"
                | "times"
                | "dias"
                | "days"
        )
    })
}

fn consume_clock_suffix(tokens: &[Token], mut idx: usize, end: usize) -> usize {
    if idx < end && matches!(word_at(tokens, idx), Some("am" | "pm" | "oclock")) {
        idx += 1;
    }
    if idx + 1 < end
        && (starts_with_words(tokens, idx, &["en", "punto"])
            || starts_with_words(tokens, idx, &["y", "media"])
            || starts_with_words(tokens, idx, &["y", "cuarto"])
            || starts_with_words(tokens, idx, &["menos", "cuarto"]))
    {
        idx += 2;
    }
    if idx + 2 < end
        && (starts_with_words(tokens, idx, &["de", "la", "manana"])
            || starts_with_words(tokens, idx, &["de", "la", "tarde"])
            || starts_with_words(tokens, idx, &["de", "la", "noche"])
            || starts_with_words(tokens, idx, &["de", "la", "madrugada"])
            || starts_with_words(tokens, idx, &["in", "the", "morning"])
            || starts_with_words(tokens, idx, &["in", "the", "afternoon"])
            || starts_with_words(tokens, idx, &["in", "the", "evening"]))
    {
        idx += 3;
    }
    idx
}

/// Analiza una hora del reloj empezando en `idx` dentro de `[idx..end)`.
/// Si `require_explicit_marker` es `true`, exige `:mm`, `am/pm` u `o'clock`.
fn parse_clock_time(
    tokens: &[Token],
    idx: usize,
    end: usize,
    require_explicit_marker: bool,
) -> Option<usize> {
    let (hour, mut cur) = parse_int_word_or_digits(tokens, idx)?;
    if hour > 23 || cur > end {
        return None;
    }

    // Formato "HH:MM[:SS]" (incluye minutos malformados tras "a las"/"at" para bloquear fragmentos).
    if cur < end && is_colon_at(tokens, cur) {
        if cur + 1 < end
            && tokens
                .get(cur + 1)
                .is_some_and(|tk| matches!(tk.kind, TokenKind::Number | TokenKind::InvalidNumber))
            && (!require_explicit_marker || is_two_digit_minute(tokens, cur + 1))
        {
            cur += 2;
            if cur + 1 < end
                && is_colon_at(tokens, cur)
                && tokens
                    .get(cur + 1)
                    .is_some_and(|tk| tk.kind == TokenKind::Number)
            {
                cur += 2;
            }
            return Some(consume_clock_suffix(tokens, cur, end));
        }
        return None;
    }

    // Formato "<1..=12> am/pm/o'clock"
    if cur < end
        && (1..=12).contains(&hour)
        && matches!(word_at(tokens, cur), Some("am" | "pm" | "oclock"))
    {
        return Some(consume_clock_suffix(tokens, cur, end));
    }

    if require_explicit_marker {
        return None;
    }

    // Sin ":" ni am/pm (p. ej. "a las 8", "desde las 8", "at 8", "a las ocho y media"):
    // no absorber si inmediatamente sigue una unidad clínica ("bpm", "horas", "%", etc.).
    if cur < end && is_non_clock_unit(tokens, cur, end) {
        // Excepción: "a las 20 horas" o "a las 8 hrs" cuando ya viene introducido por "las"
        if matches!(word_at(tokens, cur), Some("horas" | "hrs"))
            && idx > 0
            && word_at(tokens, idx - 1) == Some("las")
        {
            return Some(cur + 1);
        }
        return None;
    }

    Some(consume_clock_suffix(tokens, cur, end))
}

pub(super) fn find_time_hints(
    orig: &str,
    tokens: &[Token],
    start: usize,
    end: usize,
) -> Vec<MatchedTime> {
    let mut out = Vec::new();
    let mut i = start;

    while i < end {
        let matched_len: Option<usize> = 'm: {
            // "desde hace <num> horas/minutos/dias" or "desde hace un rato/tiempo"
            if starts_with_words(tokens, i, &["desde", "hace"]) && i + 2 < end {
                if let Some(num) = parse_simple_number(tokens, i + 2)
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
                if let Some(num) = parse_simple_number(tokens, i + 1)
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

            // "<num> hours/minutes/days ago"
            if let Some(num) = parse_simple_number(tokens, i)
                && num.end_idx + 1 < end
                && matches!(
                    word_at(tokens, num.end_idx),
                    Some("hour" | "hours" | "minute" | "minutes" | "day" | "days" | "hr" | "hrs")
                )
                && word_at(tokens, num.end_idx + 1) == Some("ago")
            {
                break 'm Some(num.end_idx + 2 - i);
            }

            // Preposiciones españolas con hora del reloj:
            // "como a las 08:05", "despues de las 8", "antes de las 8", "a las 08:05", "desde las 8", etc.
            for prefix_len in [3_usize, 2] {
                if i + prefix_len < end {
                    let is_es_clock_prefix = match prefix_len {
                        3 => {
                            starts_with_words(tokens, i, &["como", "a", "las"])
                                || starts_with_words(tokens, i, &["como", "a", "la"])
                                || starts_with_words(tokens, i, &["despues", "de", "las"])
                                || starts_with_words(tokens, i, &["despues", "de", "la"])
                                || starts_with_words(tokens, i, &["antes", "de", "las"])
                                || starts_with_words(tokens, i, &["antes", "de", "la"])
                        }
                        2 => {
                            starts_with_words(tokens, i, &["a", "las"])
                                || starts_with_words(tokens, i, &["a", "la"])
                                || starts_with_words(tokens, i, &["desde", "las"])
                                || starts_with_words(tokens, i, &["desde", "la"])
                                || starts_with_words(tokens, i, &["hasta", "las"])
                                || starts_with_words(tokens, i, &["hasta", "la"])
                                || starts_with_words(tokens, i, &["sobre", "las"])
                                || starts_with_words(tokens, i, &["hacia", "las"])
                                || starts_with_words(tokens, i, &["entre", "las"])
                        }
                        _ => false,
                    };
                    if is_es_clock_prefix
                        && let Some(c_end) = parse_clock_time(tokens, i + prefix_len, end, false)
                    {
                        // Si es "entre las <t1> y las <t2>", consume hasta <t2> también.
                        if starts_with_words(tokens, i, &["entre", "las"])
                            && c_end + 2 < end
                            && starts_with_words(tokens, c_end, &["y", "las"])
                            && let Some(c_end2) = parse_clock_time(tokens, c_end + 2, end, false)
                        {
                            break 'm Some(c_end2 - i);
                        }
                        break 'm Some(c_end - i);
                    }
                }
            }

            // Inglés con hora del reloj: "at 8pm", "at 08:05", "at 8", "since 8", "until 8pm", etc.
            if i + 1 < end
                && let Some(prep) = word_at(tokens, i)
            {
                let require_explicit = matches!(
                    prep,
                    "around" | "about" | "from" | "by" | "before" | "after"
                );
                if matches!(
                    prep,
                    "at" | "since"
                        | "until"
                        | "till"
                        | "around"
                        | "about"
                        | "from"
                        | "by"
                        | "before"
                        | "after"
                ) && let Some(c_end) = parse_clock_time(tokens, i + 1, end, require_explicit)
                {
                    break 'm Some(c_end - i);
                }
            }

            // Frases temporales fijas (de más larga a más corta)
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

            // Hora explícita independiente: "08:05", "8:30 pm", "8pm"
            if let Some(c_end) = parse_clock_time(tokens, i, end, true) {
                break 'm Some(c_end - i);
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
