//! Conservative word coverage. Citations are evidence, not semantic coverage.
use merken_core::{
    catalog::Indicator,
    interpret::{mentions_indicator, quote_number, quote_sleep_quality, quote_zone},
    proposal::{Proposal, ProposedField, TimeHint},
};

#[derive(Clone, Copy)]
struct Word<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

fn words(text: &str) -> Vec<Word<'_>> {
    let mut out = Vec::new();
    let mut start = None;
    let mut numeric = false;
    for (offset, ch) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        // Combining marks stay with their word. Offsets always refer to the original UTF-8.
        let decimal = numeric
            && matches!(ch, '.' | ',')
            && text
                .get(offset + ch.len_utf8()..)
                .and_then(|tail| tail.chars().next())
                .is_some_and(|next| next.is_ascii_digit());
        // Keep attached numeric signs in the original token, even when core rejects it.
        let sign = matches!(ch, '-' | '−' | '+')
            && text
                .get(offset + ch.len_utf8()..)
                .and_then(|tail| tail.chars().next())
                .is_some_and(|next| next.is_ascii_digit());
        if sign
            && let Some(begin) = start.take()
            && let Some(value) = text.get(begin..offset)
        {
            out.push(Word {
                text: value,
                start: begin,
                end: offset,
            });
        }
        if ch.is_alphanumeric() || ('\u{0300}'..='\u{036f}').contains(&ch) || decimal || sign {
            if start.is_none() {
                numeric = ch.is_ascii_digit() || sign;
            } else if ch.is_alphabetic() {
                numeric = false;
            }
            start.get_or_insert(offset);
        } else if let Some(begin) = start.take()
            && let Some(value) = text.get(begin..offset)
        {
            out.push(Word {
                text: value,
                start: begin,
                end: offset,
            });
            numeric = false;
        }
    }
    out
}

pub(super) fn connector(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "y" | "e" | "pero" | "también" | "tambien" | "and" | "but" | "also"
    )
}

// Closed ES/EN function-word list. These words cannot establish clinical coverage.
fn functional(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "de" | "la"
            | "el"
            | "los"
            | "las"
            | "un"
            | "una"
            | "me"
            | "mi"
            | "my"
            | "the"
            | "a"
            | "of"
            | "en"
            | "in"
            | "con"
            | "with"
            | "al"
            | "del"
            | "at"
            | "i"
    )
}

// Closed negation scaffold list; unknown verbs/adjectives invalidate the WHOLE negation.
fn negation_particle(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "no" | "not"
            | "sin"
            | "without"
            | "nunca"
            | "never"
            | "ni"
            | "nor"
            | "tengo"
            | "have"
            | "has"
            | "siento"
            | "feel"
            | "hay"
            | "any"
    )
}

pub(super) fn has_content(text: &str) -> bool {
    words(text).iter().any(|w| !connector(w.text))
}

// Match complete word sequences AND their exact original punctuation/spacing.
// No substring search: e.g. the field quote "72" cannot match the word "172".
fn occurrences(text: &str, tokens: &[Word<'_>], quote: &str) -> Vec<(usize, usize)> {
    let quoted = words(quote);
    let (Some(first), Some(last)) = (quoted.first(), quoted.last()) else {
        return Vec::new();
    };
    let Some(needle) = quote.get(first.start..last.end) else {
        return Vec::new();
    };
    tokens
        .windows(quoted.len())
        .filter_map(|window| {
            let (a, b) = (window.first()?, window.last()?);
            if text.get(a.start..b.end) != Some(needle) {
                return None;
            }
            let begin = a.start.checked_sub(first.start)?;
            let end = b.end.checked_add(quote.len().checked_sub(last.end)?)?;
            (text.get(begin..end) == Some(quote)).then_some((begin, end))
        })
        .collect()
}

fn inside(word: &Word<'_>, span: (usize, usize)) -> bool {
    word.start >= span.0 && word.end <= span.1
}

// Compound aliases from core's indicator_mention_end_raw (including zone phrases).
// The public predicate finds mentions anywhere, so it cannot by itself distinguish
// "blood glucose" from "unknown glucose". Only these exact aliases may extend a
// minimal mention. Keep this list in sync with core; no neighbouring text is covered.
const COMPOUND_MENTIONS: &[&str] = &[
    "heart rate",
    "frecuencia cardiaca",
    "ritmo cardiaco",
    "beats per minute",
    "blood pressure",
    "presion arterial",
    "tension arterial",
    "mm hg",
    "body temperature",
    "body temp",
    "blood glucose",
    "blood sugar",
    "azucar en sangre",
    "estado de animo",
    "oxygen saturation",
    "blood oxygen",
    "saturacion de oxigeno",
    "oxigeno en sangre",
    "o2 sat",
    "space radiation",
    "radiacion espacial",
    "distance from earth",
    "distancia a la tierra",
    "distancia desde la tierra",
    "lower back",
    "upper back",
    "espalda baja",
    "espalda alta",
];

// Match core's Latin normalization without changing the original UTF-8 slices.
fn normalized(word: &str) -> String {
    let normalized: String = word
        .chars()
        .filter(|ch| !('\u{0300}'..='\u{036f}').contains(ch))
        .flat_map(char::to_lowercase)
        .map(|ch| match ch {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            _ => ch,
        })
        .collect();
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn mention_coverage(text: &str, tokens: &[Word<'_>], indicator: Indicator) -> Vec<bool> {
    let mut covered = vec![false; tokens.len()];
    for width in 1..=4 {
        for (start, window) in tokens.windows(width).enumerate() {
            let (Some(a), Some(b)) = (window.first(), window.last()) else {
                continue;
            };
            let Some(phrase) = text.get(a.start..b.end) else {
                continue;
            };
            if !mentions_indicator(indicator, phrase) {
                continue;
            }
            let shorter = width > 1
                && (window.get(1).is_some_and(|next| {
                    text.get(next.start..b.end)
                        .is_some_and(|q| mentions_indicator(indicator, q))
                }) || window.get(width - 2).is_some_and(|prev| {
                    text.get(a.start..prev.end)
                        .is_some_and(|q| mentions_indicator(indicator, q))
                }));
            if !shorter
                || (!phrase.chars().any(|ch| matches!(ch, '\n' | '\r'))
                    && COMPOUND_MENTIONS.contains(&normalized(phrase).as_str()))
            {
                for value in covered.iter_mut().skip(start).take(width) {
                    *value = true;
                }
            }
        }
    }
    covered
}

// A scalar's citation is only evidence: retain words outside the value expression.
// Numeric parsing is strict for the whole candidate (including optional units).
// Enum predicates search within text, so only their minimal supporting spans count.
fn field_evidence(
    text: &str,
    tokens: &[Word<'_>],
    indicator: Indicator,
    field: &ProposedField,
) -> Vec<(usize, usize)> {
    let (Some(quote), Some(value)) = (field.quote.as_deref(), field.value.as_ref()) else {
        return Vec::new();
    };
    let found = occurrences(text, tokens, quote);
    // Repeated evidence in a wide citation is ambiguous: retain it as residue.
    let [span] = found.as_slice() else {
        return Vec::new();
    };
    if indicator == Indicator::Symptom
        && field.key == "description"
        && value.as_str() == Some(quote)
    {
        return found;
    }
    let local: Vec<_> = tokens
        .iter()
        .copied()
        .filter(|w| inside(w, *span))
        .collect();
    let number = value.as_f64();
    let matches = |candidate: &str| match field.key.as_str() {
        "quality" if indicator == Indicator::Sleep => quote_sleep_quality(candidate)
            .is_some_and(|quality| Some(quality.as_str()) == value.as_str()),
        "body_zone" if indicator == Indicator::Symptom => {
            quote_zone(candidate).is_some_and(|zone| Some(zone.as_str()) == value.as_str())
        }
        _ => number.is_some_and(|expected| {
            quote_number(candidate).is_some_and(|actual| (actual - expected).abs() <= 1e-9)
        }),
    };
    let mut evidence: Vec<(usize, usize)> = Vec::new();
    for width in 1..=local.len() {
        for (start, window) in local.windows(width).enumerate() {
            let (Some(a), Some(b)) = (window.first(), window.last()) else {
                continue;
            };
            if number.is_none() && evidence.iter().any(|s| a.start <= s.0 && s.1 <= b.end) {
                continue;
            }
            if !text.get(a.start..b.end).is_some_and(&matches) {
                continue;
            }
            evidence.push((a.start, b.end));
            // Core's negated-sleep recognizer starts at "not"; also consume the
            // immediately preceding auxiliary of the explicit "did not" form.
            if field.key == "quality"
                && a.text.eq_ignore_ascii_case("not")
                && let Some(previous) = start.checked_sub(1).and_then(|i| local.get(i))
                && previous.text.eq_ignore_ascii_case("did")
                && text
                    .get(previous.end..a.start)
                    .is_some_and(|gap| gap.chars().all(char::is_whitespace))
            {
                evidence.push((previous.start, previous.end));
            }
        }
    }
    evidence
}

pub(super) fn unresolved_fragments(text: &str, proposal: &Proposal) -> Vec<String> {
    let tokens = words(text);
    let mut covered = vec![false; tokens.len()];
    let mut visible = vec![false; tokens.len()];
    for quote in &proposal.uninterpreted {
        for span in occurrences(text, &tokens, quote) {
            for (i, _) in tokens.iter().enumerate().filter(|(_, w)| inside(w, span)) {
                if let Some(value) = visible.get_mut(i) {
                    *value = true;
                }
            }
        }
    }
    for record in &proposal.records {
        for span in occurrences(text, &tokens, &record.quote) {
            let local: Vec<_> = tokens.iter().copied().filter(|w| inside(w, span)).collect();
            let mentions = mention_coverage(text, &local, record.indicator);
            let mentioned: Vec<_> = local
                .iter()
                .zip(mentions)
                .filter_map(|(w, found)| found.then_some(w.start))
                .collect();
            let mut evidence = Vec::new();
            for field in &record.fields {
                evidence.extend(field_evidence(text, &local, record.indicator, field));
            }
            if let TimeHint::Ambiguous { quote } = &record.time {
                let found = occurrences(text, &local, quote);
                if found.len() == 1 {
                    evidence.extend(found);
                }
            }
            for (i, word) in tokens.iter().enumerate().filter(|(_, w)| inside(w, span)) {
                if let Some(value) = covered.get_mut(i) {
                    *value |= functional(word.text)
                        || mentioned.contains(&word.start)
                        || evidence.iter().any(|s| inside(word, *s));
                }
            }
        }
    }
    let mut unknown_negations = Vec::new();
    for quote in &proposal.negated {
        for span in occurrences(text, &tokens, quote) {
            let local: Vec<_> = tokens.iter().copied().filter(|w| inside(w, span)).collect();
            let mut mentions = vec![false; local.len()];
            for indicator in Indicator::ALL {
                for (value, found) in mentions
                    .iter_mut()
                    .zip(mention_coverage(text, &local, indicator))
                {
                    *value |= found;
                }
            }
            let understood = mentions.contains(&true)
                && local.iter().zip(&mentions).all(|(w, m)| {
                    *m || functional(w.text) || connector(w.text) || negation_particle(w.text)
                });
            if understood {
                for (i, _) in tokens.iter().enumerate().filter(|(_, w)| inside(w, span)) {
                    if let Some(value) = covered.get_mut(i) {
                        *value = true;
                    }
                }
            } else {
                unknown_negations.push(span);
                for (i, _) in tokens.iter().enumerate().filter(|(_, w)| inside(w, span)) {
                    if let Some(value) = covered.get_mut(i) {
                        *value = false;
                    }
                }
            }
        }
    }
    let mut spans = Vec::new();
    let mut start = None;
    let mut end = 0;
    let mut leading = None;
    for (i, word) in tokens.iter().enumerate() {
        let already_visible = visible.get(i).copied().unwrap_or(false);
        let known = covered.get(i).copied().unwrap_or(false);
        if already_visible || connector(word.text) || (known && !functional(word.text)) {
            if let Some(begin) = start.take() {
                spans.push((begin, end));
            }
            leading = None;
        } else if known {
            leading.get_or_insert(word.start);
        } else {
            start.get_or_insert(leading.unwrap_or(word.start));
            end = word.end;
        }
    }
    if let Some(begin) = start {
        spans.push((begin, end));
    }
    for span in &mut spans {
        for negated in &unknown_negations {
            if span.0 < negated.1 && span.1 > negated.0 {
                span.0 = span.0.min(negated.0);
                span.1 = span.1.max(negated.1);
            }
        }
    }
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for span in spans {
        if let Some(last) = merged.last_mut()
            && span.0 <= last.1
        {
            last.1 = last.1.max(span.1);
        } else {
            merged.push(span);
        }
    }
    merged
        .into_iter()
        .filter_map(|(a, b)| text.get(a..b).map(str::to_owned))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use merken_core::{interpret::interpret, proposal::ProposedRecord};

    fn wide_record(text: &str) -> Proposal {
        let mut proposal = interpret("pulso 72");
        proposal.records[0].quote = text.into();
        proposal.uninterpreted.clear();
        proposal
    }

    #[test]
    fn wide_evidence_does_not_cover_substrings_or_unknown_content() {
        for (text, expected) in [
            ("pulso 72 y tos 172 veces", vec!["tos 172 veces"]),
            ("pulso 72 estoy agotado", vec!["estoy agotado"]),
            ("pulse 72 I feel exhausted", vec!["I feel exhausted"]),
            ("pulso 72 y x72palabra", vec!["x72palabra"]),
            ("pulso 72 y tos 172.5 veces", vec!["tos 172.5 veces"]),
            ("pulso 72 y tos 0.72 veces", vec!["tos 0.72 veces"]),
            // An ambiguous repeated field citation is retained, never silently erased.
            ("pulso 72 y tos 72 veces", vec!["72", "tos 72 veces"]),
        ] {
            assert_eq!(
                unresolved_fragments(text, &wide_record(text)),
                expected,
                "{text}"
            );
        }
    }

    #[test]
    fn negation_requires_every_content_word_to_be_accounted_for() {
        for (text, expected) in [
            ("no tengo fiebre", vec![]),
            ("no headache", vec![]),
            ("no pegué ojo", vec!["no pegué ojo"]),
            (
                "no headache but exhausted",
                vec!["no headache but exhausted"],
            ),
            (
                "no tengo fiebre estoy agotado",
                vec!["no tengo fiebre estoy agotado"],
            ),
        ] {
            let mut proposal = interpret("");
            proposal.negated.push(text.into());
            assert_eq!(unresolved_fragments(text, &proposal), expected, "{text}");
        }
    }

    #[test]
    fn already_visible_uninterpreted_is_not_duplicated() {
        let mut proposal = wide_record("pulso 72 y tos 172 veces");
        proposal.uninterpreted.push("tos 172 veces".into());
        assert!(unresolved_fragments("pulso 72 y tos 172 veces", &proposal).is_empty());
        assert!(!has_content(", y and !"));
        assert!(has_content("no pegué ojo"));
    }

    #[test]
    fn residual_tokens_preserve_signs_decimals_and_utf8_boundaries() {
        // Token tests also prevent unsigned evidence from consuming part of a
        // signed value, even if a broad record quote contains both spellings.
        for literal in ["-0,5", "−0.5", "+36,6", "-72", "−72", "+72"] {
            let text = format!("pulse {literal}");
            let tokens = words(&text);
            assert_eq!(tokens.len(), 2, "{text}");
            assert_eq!(tokens[1].text, literal);
            assert_eq!(&text[tokens[1].start..tokens[1].end], literal);
            let mut proposal = interpret("pulse 72");
            proposal.records[0].quote = text.clone();
            assert_eq!(unresolved_fragments(&text, &proposal), [literal]);
        }
        // A sign attached to a number starts that numeric token even next to a word.
        let tokens = words("pulse−72");
        assert_eq!(
            tokens.iter().map(|word| word.text).collect::<Vec<_>>(),
            ["pulse", "−72"]
        );
    }

    #[test]
    fn indicator_phrase_cannot_cover_adjacent_unknown_words() {
        let proposal = Proposal {
            records: vec![ProposedRecord {
                indicator: Indicator::HeartRate,
                fields: Vec::new(),
                time: TimeHint::Unspecified,
                quote: "heart rate exhausted".into(),
            }],
            ..interpret("")
        };
        assert_eq!(
            unresolved_fragments("heart rate exhausted", &proposal),
            ["exhausted"]
        );
    }
}
