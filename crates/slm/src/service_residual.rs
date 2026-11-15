//! Conservative word coverage. Citations are evidence, not semantic coverage.
use merken_core::{
    catalog::Indicator,
    interpret::mentions_indicator,
    proposal::{Proposal, TimeHint},
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
        if ch.is_alphanumeric() || ('\u{0300}'..='\u{036f}').contains(&ch) || decimal {
            if start.is_none() {
                numeric = ch.is_ascii_digit();
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

// Only minimal mentions count. A longer phrase containing "pulse" cannot cover
// arbitrary neighbouring words. Preserve punctuation in phrases such as "mg/dL".
fn mention_coverage(text: &str, tokens: &[Word<'_>], indicator: Indicator) -> Vec<bool> {
    let mut covered = vec![false; tokens.len()];
    for width in 1..=3 {
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
            if !shorter {
                for value in covered.iter_mut().skip(start).take(width) {
                    *value = true;
                }
            }
        }
    }
    covered
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
            for quote in record
                .fields
                .iter()
                .filter_map(|f| f.quote.as_deref())
                .chain(match &record.time {
                    TimeHint::Ambiguous { quote } => Some(quote.as_str()),
                    _ => None,
                })
            {
                // Repeated evidence in a wide citation is ambiguous: retain it as residue.
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
