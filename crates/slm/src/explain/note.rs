//! Validación pura con vocabulario cerrado para notas del explicador SLM (RF-15, RNF-14).

use crate::explain::facts::{
    FactLine, infer_status_from_facts, is_approved_protocol_from_facts, is_urgent_from_facts,
};
use merken_core::evaluate::Status;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Etiqueta obligatoria que identifica toda nota generada por el SLM (RF-15).
pub const AI_SUGGESTION_LABEL: &str = "AI-generated suggestion";

/// Longitud máxima en caracteres permitida para una nota del SLM.
pub const MAX_NOTE_CHARS: usize = 350;
/// Número máximo de frases permitido para una nota del SLM.
pub const MAX_NOTE_SENTENCES: usize = 3;
/// Número máximo de palabras permitido por frase en una nota del SLM.
pub const MAX_WORDS_PER_SENTENCE: usize = 30;

/// Código de descarte: nota vacía.
pub const ERR_EMPTY_NOTE: &str = "empty_note";
/// Código de descarte: la nota supera `MAX_NOTE_CHARS`.
pub const ERR_TOO_LONG: &str = "note_too_long";
/// Código de descarte: la nota no tiene entre 1 y 3 frases.
pub const ERR_SENTENCE_COUNT: &str = "invalid_sentence_count";
/// Código de descarte: una frase supera `MAX_WORDS_PER_SENTENCE`.
pub const ERR_SENTENCE_TOO_LONG: &str = "sentence_too_long";
/// Código de descarte: corchetes de cita vacíos, sin cerrar o mal formados.
pub const ERR_MALFORMED_CITATION: &str = "malformed_citation";
/// Código de descarte: cita a un identificador de documento no entregado.
pub const ERR_UNKNOWN_CITATION: &str = "unknown_citation";
/// Código de descarte: la nota contiene dígitos, números en palabras, ordinales o porcentajes.
pub const ERR_FORBIDDEN_NUMBER: &str = "forbidden_number";
/// Código de descarte: negación no respaldada por una frase literal de hechos o ficha citada.
pub const ERR_FORBIDDEN_NEGATION: &str = "forbidden_negation";
/// Código de descarte: vocabulario incompatible con el estado de la evaluación.
pub const ERR_INCOMPATIBLE_STATUS: &str = "incompatible_status_word";
/// Código de descarte: afirmación de aprobación sin `source_kind == approved_protocol` y respaldo en hechos.
pub const ERR_UNVERIFIED_APPROVAL: &str = "unverified_approval_claim";
/// Código de descarte: palabra fuera del vocabulario cerrado permitido.
pub const ERR_OUT_OF_VOCABULARY: &str = "out_of_vocabulary";
/// Código de descarte: texto no inglés o con caracteres/diacríticos ajenos.
pub const ERR_NON_ENGLISH: &str = "non_english_output";

/// Ficha de contramedida o protocolo curada proporcionada al explicador (RF-31).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub text: String,
}

/// Nota opcional en lenguaje llano generada por el SLM tras superar la validación cerrada.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiNote {
    pub text: String,
    pub cited_docs: Vec<String>,
    pub label: String,
}

/// Lista cerrada de palabras funcionales inglesas neutras permitidas en la nota.
///
/// No incluye verbos de consejo (`take`, `should`, `must`, etc.), palabras de estado
/// ni términos numéricos; esos solo se admiten si proceden de los hechos o fichas citadas.
const ENGLISH_FUNCTION_WORDS: &[&str] = &[
    // Artículos
    "a",
    "an",
    "the",
    // Pronombres y determinantes neutros
    "i",
    "me",
    "my",
    "mine",
    "you",
    "your",
    "yours",
    "it",
    "its",
    "we",
    "us",
    "our",
    "ours",
    "they",
    "them",
    "their",
    "theirs",
    "this",
    "that",
    "these",
    "those",
    "which",
    "who",
    "whom",
    "whose",
    "what",
    "such",
    "each",
    "every",
    "all",
    "some",
    "any",
    "other",
    "another",
    "same",
    // Preposiciones
    "in",
    "on",
    "at",
    "by",
    "for",
    "from",
    "of",
    "to",
    "with",
    "about",
    "against",
    "along",
    "among",
    "around",
    "as",
    "before",
    "after",
    "between",
    "during",
    "into",
    "near",
    "over",
    "through",
    "toward",
    "towards",
    "under",
    "until",
    "upon",
    "within",
    // Conjunciones y conectores
    "and",
    "or",
    "but",
    "if",
    "when",
    "while",
    "where",
    "because",
    "since",
    "so",
    "than",
    "then",
    "though",
    "although",
    "unless",
    "whether",
    "yet",
    // Auxiliares neutros (sin verbos de obligación o consejo)
    "is",
    "am",
    "are",
    "was",
    "were",
    "be",
    "been",
    "being",
    "has",
    "have",
    "had",
    "do",
    "does",
    "did",
    "can",
    "could",
    "may",
    "might",
    "will",
    "would",
    // Adverbios neutros
    "also",
    "currently",
    "here",
    "there",
    "still",
    "now",
    "today",
    "recently",
    "already",
    "only",
    "even",
    "just",
    "very",
    "more",
    "most",
    "less",
];

/// Números en palabras (inglés y español), ordinales, fracciones, multiplicadores y porcentajes.
const FORBIDDEN_NUMBER_WORDS: &[&str] = &[
    // Inglés: cardinales
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "thirty",
    "forty",
    "fourty",
    "fifty",
    "sixty",
    "seventy",
    "eighty",
    "ninety",
    "hundred",
    "hundreds",
    "thousand",
    "thousands",
    "million",
    "millions",
    "billion",
    "billions",
    "trillion",
    "trillions",
    // Inglés: multiplicadores, fracciones, cantidades y porcentaje
    "once",
    "twice",
    "thrice",
    "double",
    "triple",
    "quadruple",
    "half",
    "halves",
    "quarter",
    "quarters",
    "dozen",
    "dozens",
    "score",
    "pair",
    "couple",
    "both",
    "percent",
    "percentage",
    "percentages",
    "cent",
    // Inglés: ordinales
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
    "twentieth",
    "thirtieth",
    "fortieth",
    "fiftieth",
    "sixtieth",
    "seventieth",
    "eightieth",
    "ninetieth",
    "hundredth",
    "thousandth",
    "millionth",
    // Español: cardinales (sin acentos tras normalización)
    "cero",
    "uno",
    "una",
    "un",
    "dos",
    "tres",
    "cuatro",
    "cinco",
    "seis",
    "siete",
    "ocho",
    "nueve",
    "diez",
    "doce",
    "trece",
    "catorce",
    "quince",
    "dieciseis",
    "diecisiete",
    "dieciocho",
    "diecinueve",
    "veinte",
    "veintiuno",
    "veintiuna",
    "veintiun",
    "veintidos",
    "veintitres",
    "veinticuatro",
    "veinticinco",
    "veintiseis",
    "veintisiete",
    "veintiocho",
    "veintinueve",
    "treinta",
    "cuarenta",
    "cincuenta",
    "sesenta",
    "setenta",
    "ochenta",
    "noventa",
    "cien",
    "ciento",
    "cientos",
    "doscientos",
    "doscientas",
    "trescientos",
    "trescientas",
    "cuatrocientos",
    "cuatrocientas",
    "quinientos",
    "quinientas",
    "seiscientos",
    "seiscientas",
    "setecientos",
    "setecientas",
    "ochocientos",
    "ochocientas",
    "novecientos",
    "novecientas",
    "mil",
    "millon",
    "millones",
    // Español: multiplicadores, fracciones, porcentaje y ordinales
    "doble",
    "mitad",
    "medio",
    "media",
    "tercio",
    "cuarto",
    "cuarta",
    "docena",
    "docenas",
    "par",
    "porcentaje",
    "porciento",
    "primero",
    "primera",
    "primer",
    "segundo",
    "segunda",
    "tercero",
    "tercera",
    "tercer",
    "quinto",
    "quinta",
    "sexto",
    "sexta",
    "septimo",
    "septima",
    "octavo",
    "octava",
    "noveno",
    "novena",
    "decimo",
    "decima",
    "undecimo",
    "duodecimo",
    "centesimo",
    "milesimo",
];

const NEGATION_WORDS: &[&str] = &[
    "not",
    "no",
    "never",
    "without",
    "unnecessary",
    "none",
    "nothing",
    "neither",
    "nor",
    "cannot",
    "cant",
];

const FORBIDDEN_WHEN_ALARM: &[&str] = &[
    "stable",
    "fine",
    "safe",
    "normal",
    "healthy",
    "well",
    "excellent",
    "postpone",
    "wait",
    "later",
    "calm",
];

const FORBIDDEN_WHEN_STABLE: &[&str] = &[
    "alarm",
    "urgent",
    "urgency",
    "emergency",
    "immediately",
    "immediate",
    "critical",
];

const FORBIDDEN_WHEN_CHANGE: &[&str] = &[
    "stable",
    "alarm",
    "urgent",
    "urgency",
    "emergency",
    "immediately",
    "immediate",
    "critical",
    "fine",
    "safe",
    "normal",
    "healthy",
    "well",
    "excellent",
];

const FORBIDDEN_WHEN_NOT_EVALUATED: &[&str] = &[
    "stable",
    "unstable",
    "change",
    "alarm",
    "urgent",
    "urgency",
    "emergency",
    "immediately",
    "immediate",
    "critical",
    "fine",
    "safe",
    "normal",
    "healthy",
    "unhealthy",
    "well",
    "excellent",
    "good",
    "poor",
    "fair",
    "elevated",
    "high",
    "higher",
    "low",
    "lower",
    "above",
    "below",
    "within",
    "outside",
    "range",
    "shift",
    "trend",
    "improving",
    "worsening",
];

const APPROVAL_WORDS: &[&str] = &[
    "approved",
    "approval",
    "approve",
    "authorized",
    "authorize",
    "authority",
    "certified",
    "certify",
    "certification",
];

/// Valida de forma pura que la nota del SLM cumpla longitud, citas, ausencia de números,
/// negaciones autorizadas, coherencia de estado y vocabulario cerrado.
///
/// Devuelve [`AiNote`] si la nota es válida o un código `&'static str` constante si debe
/// descartarse. Nunca incorpora texto del modelo ni cifras clínicas en el error.
pub fn validate_note(
    text: &str,
    facts: &[FactLine],
    docs: &[Document],
) -> Result<AiNote, &'static str> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ERR_EMPTY_NOTE);
    }
    if trimmed.chars().count() > MAX_NOTE_CHARS {
        return Err(ERR_TOO_LONG);
    }

    let (prose, cited_docs) = parse_and_strip_citations(trimmed, docs)?;

    if has_non_english_chars(&prose) {
        return Err(ERR_NON_ENGLISH);
    }
    if has_numeric_symbol(&prose) {
        return Err(ERR_FORBIDDEN_NUMBER);
    }

    let sentences = split_sentences(&prose);
    if sentences.is_empty() || sentences.len() > MAX_NOTE_SENTENCES {
        return Err(ERR_SENTENCE_COUNT);
    }

    let literal_negated_sentences = collect_literal_sentences(facts, docs, &cited_docs);

    for sentence in &sentences {
        let raw_tokens = split_raw_tokens(sentence);
        let word_tokens = tokenize_prose_words(sentence);
        if word_tokens.is_empty() || word_tokens.len() > MAX_WORDS_PER_SENTENCE {
            return Err(ERR_SENTENCE_TOO_LONG);
        }

        if contains_forbidden_number_words(&word_tokens) {
            return Err(ERR_FORBIDDEN_NUMBER);
        }

        if sentence_has_negation(&raw_tokens, &word_tokens) {
            let norm = normalize_sentence(sentence);
            if !literal_negated_sentences.contains(&norm) {
                return Err(ERR_FORBIDDEN_NEGATION);
            }
        }
    }

    let all_prose_words = tokenize_prose_words(&prose);
    if all_prose_words.is_empty() {
        return Err(ERR_SENTENCE_COUNT);
    }

    check_status_compatibility(&all_prose_words, facts)?;
    check_approval_claims(&all_prose_words, facts)?;
    check_closed_vocabulary(&all_prose_words, facts, docs, &cited_docs)?;

    Ok(AiNote {
        text: trimmed.to_owned(),
        cited_docs,
        label: AI_SUGGESTION_LABEL.to_owned(),
    })
}

fn parse_and_strip_citations(
    text: &str,
    docs: &[Document],
) -> Result<(String, Vec<String>), &'static str> {
    let mut prose = String::with_capacity(text.len());
    let mut cited_docs = Vec::new();
    let mut rest = text;

    while let Some(open_idx) = rest.find('[') {
        let before = rest.get(..open_idx).unwrap_or("");
        if before.contains(']') {
            return Err(ERR_MALFORMED_CITATION);
        }
        prose.push_str(before);
        prose.push(' ');

        let after_open = rest.get(open_idx + 1..).unwrap_or("");
        let Some(close_idx) = after_open.find(']') else {
            return Err(ERR_MALFORMED_CITATION);
        };
        let raw_inner = after_open.get(..close_idx).unwrap_or("");
        if raw_inner.contains('[') || raw_inner.contains('\n') || raw_inner.contains('\r') {
            return Err(ERR_MALFORMED_CITATION);
        }
        let doc_id = raw_inner.trim();
        if doc_id.is_empty() {
            return Err(ERR_MALFORMED_CITATION);
        }
        if !docs.iter().any(|d| d.id == doc_id) {
            return Err(ERR_UNKNOWN_CITATION);
        }
        if !cited_docs.iter().any(|existing| existing == doc_id) {
            cited_docs.push(doc_id.to_owned());
        }

        rest = after_open.get(close_idx + 1..).unwrap_or("");
    }

    if rest.contains(']') {
        return Err(ERR_MALFORMED_CITATION);
    }
    prose.push_str(rest);

    Ok((prose, cited_docs))
}

fn has_non_english_chars(prose: &str) -> bool {
    prose.chars().any(|ch| {
        if matches!(
            ch,
            '¿' | '¡'
                | 'ñ'
                | 'Ñ'
                | 'á'
                | 'é'
                | 'í'
                | 'ó'
                | 'ú'
                | 'ü'
                | 'Á'
                | 'É'
                | 'Í'
                | 'Ó'
                | 'Ú'
                | 'Ü'
        ) {
            return true;
        }
        ch.is_alphabetic() && !ch.is_ascii_alphabetic()
    })
}

fn has_numeric_symbol(prose: &str) -> bool {
    prose
        .chars()
        .any(|c| c.is_numeric() || matches!(c, '%' | '‰' | '‱' | '％'))
}

fn split_sentences(prose: &str) -> Vec<&str> {
    prose
        .split(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| s.chars().any(char::is_alphanumeric))
        .collect()
}

fn split_raw_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' || ch == '’' {
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn fold_accent(ch: char) -> char {
    match ch {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        _ => ch,
    }
}

fn normalize_ascii_lower_no_accents(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        for lc in fold_accent(ch).to_lowercase() {
            out.push(lc);
        }
    }
    out
}

fn strip_possessive(token: &str) -> &str {
    let t = token.trim_matches(['\'', '’']);
    if let Some(stem) = t.strip_suffix("'s").or_else(|| t.strip_suffix("’s")) {
        stem
    } else if let Some(stem) = t.strip_suffix('\'').or_else(|| t.strip_suffix('’')) {
        stem
    } else {
        t
    }
}

fn tokenize_prose_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    for raw in split_raw_tokens(text) {
        let folded = normalize_ascii_lower_no_accents(&raw);
        let stripped = strip_possessive(&folded);
        for part in stripped.split(|c: char| !c.is_ascii_alphanumeric()) {
            if !part.is_empty() {
                words.push(part.to_owned());
            }
        }
    }
    words
}

fn tokenize_allowed_content_words(text: &str, out: &mut HashSet<String>) {
    for raw in split_raw_tokens(text) {
        let folded = normalize_ascii_lower_no_accents(&raw);
        let stripped = strip_possessive(&folded);
        for part in stripped.split(|c: char| !c.is_ascii_alphanumeric()) {
            if part.len() >= 2 && part.chars().all(|c| c.is_ascii_alphabetic()) {
                for stem in word_variants(part) {
                    out.insert(stem);
                }
            }
        }
    }
}

fn word_variants(word: &str) -> Vec<String> {
    let mut variants = Vec::with_capacity(6);
    variants.push(word.to_owned());

    let len = word.len();
    if len >= 5
        && word.ends_with('s')
        && !word.ends_with("ss")
        && !word.ends_with("us")
        && !word.ends_with("is")
        && let Some(base) = word.strip_suffix('s')
        && base.len() >= 4
    {
        variants.push(base.to_owned());
    }
    if len >= 6
        && word.ends_with("es")
        && let Some(base) = word.strip_suffix("es")
        && base.len() >= 4
    {
        variants.push(base.to_owned());
    }
    if len >= 6
        && word.ends_with("ed")
        && !word.ends_with("eed")
        && let Some(base) = word.strip_suffix("ed")
        && base.len() >= 4
    {
        variants.push(base.to_owned());
        variants.push(format!("{base}e"));
    }
    if len >= 7
        && word.ends_with("ing")
        && let Some(base) = word.strip_suffix("ing")
        && base.len() >= 4
    {
        variants.push(base.to_owned());
        variants.push(format!("{base}e"));
    }
    if len >= 6
        && word.ends_with("ly")
        && let Some(base) = word.strip_suffix("ly")
        && base.len() >= 4
    {
        variants.push(base.to_owned());
    }

    variants
}

fn contains_forbidden_number_words(words: &[String]) -> bool {
    if words
        .iter()
        .any(|w| FORBIDDEN_NUMBER_WORDS.contains(&w.as_str()))
    {
        return true;
    }
    for pair in words.windows(2) {
        if let (Some(a), Some(b)) = (pair.first(), pair.get(1))
            && ((a == "per" && b == "cent") || (a == "por" && b == "ciento"))
        {
            return true;
        }
    }
    false
}

fn sentence_has_negation(raw_tokens: &[String], word_tokens: &[String]) -> bool {
    if raw_tokens.iter().any(|tok| {
        let lower = tok.to_lowercase();
        lower.contains("n't") || lower.contains("n’t")
    }) {
        return true;
    }
    word_tokens
        .iter()
        .any(|w| NEGATION_WORDS.contains(&w.as_str()))
}

fn normalize_sentence(sentence: &str) -> String {
    let folded = normalize_ascii_lower_no_accents(sentence);
    let mut collapsed = String::with_capacity(folded.len());
    let mut prev_space = true;
    for ch in folded.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                collapsed.push(' ');
                prev_space = true;
            }
        } else {
            collapsed.push(ch);
            prev_space = false;
        }
    }
    collapsed
        .trim()
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .replace(" ,", ",")
        .replace(" ;", ";")
        .replace(" :", ":")
}

fn collect_literal_sentences(
    facts: &[FactLine],
    docs: &[Document],
    cited_docs: &[String],
) -> HashSet<String> {
    let mut out = HashSet::new();
    for fact in facts {
        for seg in fact.text.split(['.', '!', '?']) {
            let norm = normalize_sentence(seg);
            if !norm.is_empty() {
                out.insert(norm.clone());
            }
            if let Some((_, rhs)) = seg.split_once(": ") {
                let rhs_norm = normalize_sentence(rhs);
                if rhs_norm.split_whitespace().count() >= 2 {
                    out.insert(rhs_norm);
                }
            }
        }
    }
    for doc_id in cited_docs {
        if let Some(doc) = docs.iter().find(|d| d.id == *doc_id) {
            for seg in doc.text.split(['.', '!', '?']) {
                let norm = normalize_sentence(seg);
                if !norm.is_empty() {
                    out.insert(norm);
                }
            }
        }
    }
    out
}

fn token_matches_any(word: &str, list: &[&str]) -> bool {
    word_variants(word)
        .iter()
        .any(|variant| list.contains(&variant.as_str()))
}

fn check_status_compatibility(words: &[String], facts: &[FactLine]) -> Result<(), &'static str> {
    let status = infer_status_from_facts(facts);
    let urgent = is_urgent_from_facts(facts);

    let forbidden = if status == Status::Alarm || urgent {
        FORBIDDEN_WHEN_ALARM
    } else {
        match status {
            Status::Stable => FORBIDDEN_WHEN_STABLE,
            Status::Change => FORBIDDEN_WHEN_CHANGE,
            Status::NotEvaluated => FORBIDDEN_WHEN_NOT_EVALUATED,
            Status::Alarm => FORBIDDEN_WHEN_ALARM,
        }
    };

    if words.iter().any(|w| token_matches_any(w, forbidden)) {
        return Err(ERR_INCOMPATIBLE_STATUS);
    }
    Ok(())
}

fn check_approval_claims(words: &[String], facts: &[FactLine]) -> Result<(), &'static str> {
    let has_approval_word = words.iter().any(|w| token_matches_any(w, APPROVAL_WORDS));
    if !has_approval_word {
        return Ok(());
    }

    if !is_approved_protocol_from_facts(facts) {
        return Err(ERR_UNVERIFIED_APPROVAL);
    }

    let mut fact_words = HashSet::new();
    for fact in facts {
        tokenize_allowed_content_words(&fact.text, &mut fact_words);
    }

    for word in words {
        if token_matches_any(word, APPROVAL_WORDS)
            && !word_variants(word).iter().any(|v| fact_words.contains(v))
        {
            return Err(ERR_UNVERIFIED_APPROVAL);
        }
    }

    Ok(())
}

fn check_closed_vocabulary(
    words: &[String],
    facts: &[FactLine],
    docs: &[Document],
    cited_docs: &[String],
) -> Result<(), &'static str> {
    let mut allowed_content = HashSet::new();
    for fact in facts {
        tokenize_allowed_content_words(&fact.text, &mut allowed_content);
    }
    for doc_id in cited_docs {
        if let Some(doc) = docs.iter().find(|d| d.id == *doc_id) {
            tokenize_allowed_content_words(&doc.text, &mut allowed_content);
        }
    }

    for word in words {
        if ENGLISH_FUNCTION_WORDS.contains(&word.as_str()) {
            continue;
        }
        let in_content = word_variants(word)
            .iter()
            .any(|v| allowed_content.contains(v));
        if !in_content {
            return Err(ERR_OUT_OF_VOCABULARY);
        }
    }

    Ok(())
}
