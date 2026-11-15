//! Selección estructurada de frases curadas con gramática GBNF (RF-15, RF-31, RNF-14).
//!
//! Sustituye la validación por vocabulario cerrado: el SLM únicamente selecciona y ordena
//! entre 0 y 3 identificadores de frases autorizadas previamente filtradas por Rust según
//! el estado clínico de la evaluación. El texto visible procede íntegramente de las fichas
//! curadas sin modificaciones ni prosa libre generada por el modelo.

use crate::error::SlmError;
use merken_core::evaluate::Status;
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::Value;
use std::{
    collections::HashSet,
    fmt::{self, Write as _},
};

/// Etiqueta obligatoria que identifica toda nota asistida por el SLM (RF-15).
pub const AI_SUGGESTION_LABEL: &str = "AI-generated suggestion";

/// Longitud máxima admitida para identificadores de ficha y frase.
pub const MAX_ID_LEN: usize = 40;
/// Máximo de IDs ofrecidos en una gramática de selección.
pub const MAX_SELECTION_IDS: usize = 32;
const MAX_SELECTION_ID_BYTES: usize = 64;

fn valid_selection_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_SELECTION_ID_BYTES
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'/' | b'-'))
}

/// Longitud máxima en caracteres Unicode de una frase curada.
pub const MAX_STATEMENT_CHARS: usize = 500;
/// Número máximo de frases seleccionables en una nota del explicador.
pub const MAX_SELECTED_STATEMENTS: usize = 3;
/// Tamaño máximo en bytes de la salida cruda del explicador antes de analizar JSON.
pub const MAX_SELECTION_OUTPUT_BYTES: usize = 4096;

/// Código de descarte: la salida del modelo supera `MAX_SELECTION_OUTPUT_BYTES`.
pub const ERR_OUTPUT_TOO_LARGE: &str = "output_too_large";
/// Código de descarte: la salida del modelo no es JSON sintácticamente válido.
pub const ERR_INVALID_JSON: &str = "invalid_json";
/// Código de descarte: el objeto JSON contiene campos fuera del esquema `{"select": [...]}`.
pub const ERR_EXTRA_FIELDS: &str = "extra_fields";
/// Código de descarte: la estructura o los tipos JSON no coinciden con `{"select": ["..."]}`.
pub const ERR_INVALID_SCHEMA: &str = "invalid_schema";
/// Código de descarte: se seleccionaron más de `MAX_SELECTED_STATEMENTS` elementos.
pub const ERR_TOO_MANY_SELECTIONS: &str = "too_many_selections";
/// Código de descarte: algún identificador devuelto no existe entre las frases ofrecidas.
pub const ERR_UNKNOWN_SELECTION: &str = "unknown_selection";
/// Código de descarte: algún identificador aparece repetido en la selección.
pub const ERR_DUPLICATE_SELECTION: &str = "duplicate_selection";

fn validate_closed_id(raw: &str, kind: &str) -> Result<(), SlmError> {
    if raw.is_empty() || raw.len() > MAX_ID_LEN {
        return Err(SlmError::Unsupported(format!(
            "{kind} ID length must be between 1 and {MAX_ID_LEN} ASCII characters"
        )));
    }
    if !raw
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
    {
        return Err(SlmError::Unsupported(format!(
            "{kind} ID must match [a-z0-9-]{{1,{MAX_ID_LEN}}}"
        )));
    }
    Ok(())
}

fn is_bidi_or_format_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200E}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{200B}'..='\u{200D}'
            | '\u{FEFF}'
            | '\u{2060}'
            | '\u{00AD}'
    )
}

fn validate_curated_text(text: &str) -> Result<(), SlmError> {
    if text.trim().is_empty() {
        return Err(SlmError::Unsupported(
            "Curated statement text cannot be empty".to_owned(),
        ));
    }
    if text.chars().count() > MAX_STATEMENT_CHARS {
        return Err(SlmError::Unsupported(format!(
            "Curated statement text exceeds {MAX_STATEMENT_CHARS} characters"
        )));
    }
    if !text.chars().all(|ch| {
        !is_bidi_or_format_char(ch)
            && !ch.is_control()
            && ch.is_ascii()
            && (' '..='~').contains(&ch)
    }) {
        return Err(SlmError::Unsupported(
            "Curated statement text must be printable ASCII English without bidi, format, or control characters".to_owned(),
        ));
    }
    if !text.chars().any(|ch| ch.is_ascii_alphabetic()) {
        return Err(SlmError::Unsupported(
            "Curated statement text must contain English alphabetic words".to_owned(),
        ));
    }
    Ok(())
}

/// Identificador opaco y cerrado de una ficha curada (`[a-z0-9-]{1,40}`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct FichaId(String);

impl FichaId {
    /// Construye y valida un identificador de ficha con formato `[a-z0-9-]{1,40}`.
    pub fn new(id: impl AsRef<str>) -> Result<Self, SlmError> {
        let raw = id.as_ref();
        validate_closed_id(raw, "Ficha")?;
        Ok(Self(raw.to_owned()))
    }

    /// Devuelve la representación textual opaca del identificador.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for FichaId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FichaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for FichaId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(|_| de::Error::custom("invalid ficha id"))
    }
}

/// Identificador opaco y cerrado de una frase dentro de una ficha (`[a-z0-9-]{1,40}`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct StatementId(String);

impl StatementId {
    /// Construye y valida un identificador de frase con formato `[a-z0-9-]{1,40}`.
    pub fn new(id: impl AsRef<str>) -> Result<Self, SlmError> {
        let raw = id.as_ref();
        validate_closed_id(raw, "Statement")?;
        Ok(Self(raw.to_owned()))
    }

    /// Devuelve la representación textual opaca del identificador.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for StatementId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StatementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for StatementId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(|_| de::Error::custom("invalid statement id"))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StatusRepr {
    NotEvaluated,
    Stable,
    Change,
    Alarm,
}

impl From<StatusRepr> for Status {
    fn from(repr: StatusRepr) -> Self {
        match repr {
            StatusRepr::NotEvaluated => Self::NotEvaluated,
            StatusRepr::Stable => Self::Stable,
            StatusRepr::Change => Self::Change,
            StatusRepr::Alarm => Self::Alarm,
        }
    }
}

/// Frase curada individual perteneciente a una [`Ficha`] (RF-31).
///
/// Su texto se valida al construir (ASCII imprimible en inglés, sin controles bidi,
/// caracteres de formato ni de control) y nunca se modifica ni se reescribe por el SLM.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Statement {
    id: StatementId,
    text: String,
    applies_to: Vec<Status>,
}

impl Statement {
    /// Construye y valida una frase curada para los estados clínicos indicados.
    pub fn new(
        id: StatementId,
        text: impl Into<String>,
        applies_to: Vec<Status>,
    ) -> Result<Self, SlmError> {
        let text = text.into();
        validate_curated_text(&text)?;
        Ok(Self {
            id,
            text,
            applies_to,
        })
    }

    /// Identificador opaco de la frase dentro de su ficha.
    pub fn id(&self) -> &StatementId {
        &self.id
    }

    /// Texto curado exacto tal como fue cargado.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Estados de evaluación en los que esta frase puede ofrecerse al SLM.
    pub fn applies_to(&self) -> &[Status] {
        &self.applies_to
    }

    /// Indica si la frase es elegible para el `status` clínico indicado.
    pub fn applies_to_status(&self, status: Status) -> bool {
        self.applies_to.contains(&status)
    }
}

impl<'de> Deserialize<'de> for Statement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawStatement {
            id: StatementId,
            text: String,
            applies_to: Vec<StatusRepr>,
        }

        let raw = RawStatement::deserialize(deserializer)?;
        let applies_to = raw.applies_to.into_iter().map(Status::from).collect();
        Self::new(raw.id, raw.text, applies_to)
            .map_err(|_| de::Error::custom("invalid curated statement"))
    }
}

/// Ficha curada de protocolo o contramedida compuesta por frases con ID (RF-31).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Ficha {
    id: FichaId,
    statements: Vec<Statement>,
}

impl Ficha {
    /// Construye una ficha curada asegurando unicidad de `StatementId` internos.
    pub fn new(id: FichaId, statements: Vec<Statement>) -> Result<Self, SlmError> {
        let mut seen = HashSet::with_capacity(statements.len());
        for stmt in &statements {
            if !seen.insert(stmt.id().as_str()) {
                return Err(SlmError::Unsupported(
                    "Duplicate statement ID within ficha".to_owned(),
                ));
            }
        }
        Ok(Self { id, statements })
    }

    /// Deserializa y valida una ficha curada desde JSON sin filtrar texto en los errores.
    pub fn from_json(json: &str) -> Result<Self, SlmError> {
        serde_json::from_str(json)
            .map_err(|_| SlmError::Unsupported("Invalid curated ficha JSON".to_owned()))
    }

    /// Deserializa y valida una lista de fichas curadas verificando unicidad global de `FichaId`.
    pub fn vec_from_json(json: &str) -> Result<Vec<Self>, SlmError> {
        let fichas: Vec<Self> = serde_json::from_str(json)
            .map_err(|_| SlmError::Unsupported("Invalid curated fichas JSON".to_owned()))?;
        validate_unique_fichas(&fichas)?;
        Ok(fichas)
    }

    /// Identificador opaco de la ficha.
    pub fn id(&self) -> &FichaId {
        &self.id
    }

    /// Frases curadas contenidas en la ficha.
    pub fn statements(&self) -> &[Statement] {
        &self.statements
    }
}

impl<'de> Deserialize<'de> for Ficha {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawFicha {
            id: FichaId,
            statements: Vec<Statement>,
        }

        let raw = RawFicha::deserialize(deserializer)?;
        Self::new(raw.id, raw.statements).map_err(|_| de::Error::custom("invalid curated ficha"))
    }
}

fn validate_unique_fichas(fichas: &[Ficha]) -> Result<(), SlmError> {
    let mut seen_fichas = HashSet::with_capacity(fichas.len());
    for ficha in fichas {
        if !seen_fichas.insert(ficha.id().as_str()) {
            return Err(SlmError::Unsupported(
                "Duplicate ficha ID in curated fichas".to_owned(),
            ));
        }
    }
    Ok(())
}

/// Frase autorizada ofrecida al SLM tras aplicar el filtro determinista por estado.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OfferedStatement {
    pub ficha_id: FichaId,
    pub statement_id: StatementId,
    pub selection_id: String,
    pub text: String,
}

/// Frase curada seleccionada por el SLM y renderizada directamente desde la ficha por Rust.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedStatement {
    pub ficha_id: FichaId,
    pub statement_id: StatementId,
    pub text: String,
}

/// Nota opcional del explicador construida íntegramente con frases curadas seleccionadas.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiNote {
    pub statements: Vec<RenderedStatement>,
    pub label: String,
}

/// Filtra determinísticamente las frases de `fichas` aplicables al `status` actual.
pub fn offered_statements(
    fichas: &[Ficha],
    status: Status,
) -> Result<Vec<OfferedStatement>, SlmError> {
    validate_unique_fichas(fichas)?;
    let mut offered = Vec::new();
    for ficha in fichas {
        for stmt in ficha.statements() {
            if stmt.applies_to_status(status) {
                if offered.len() >= MAX_SELECTION_IDS {
                    return Err(SlmError::Unsupported(
                        "Too many offered statements".to_owned(),
                    ));
                }
                if !valid_selection_id(&format!("{}/{}", ficha.id(), stmt.id())) {
                    return Err(SlmError::Unsupported("Invalid selection ID".to_owned()));
                }
                offered.push(OfferedStatement {
                    ficha_id: ficha.id().clone(),
                    statement_id: stmt.id().clone(),
                    selection_id: format!("{}/{}", ficha.id(), stmt.id()),
                    text: stmt.text().to_owned(),
                });
            }
        }
    }
    Ok(offered)
}

fn json_string_token(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for ch in raw.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if c.is_control() => {
                let code = c as u32;
                let _ = write!(out, "\\u{code:04x}");
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn escape_gbnf_terminal(literal: &str) -> String {
    let mut out = String::with_capacity(literal.len() + 2);
    out.push('"');
    for ch in literal.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let code = c as u32;
                let _ = write!(out, "\\x{code:02x}");
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Genera una gramática GBNF para `{"select": [...]}` restringida de 0 a 3 IDs ofrecidos.
///
/// Devuelve una cadena vacía si hay más de 32 IDs o alguno no cumple el formato
/// ASCII permitido de hasta 64 bytes. Sin IDs, solo admite `{"select": []}`.
pub fn build_selection_grammar(offered_ids: &[&str]) -> String {
    if offered_ids.len() > MAX_SELECTION_IDS || offered_ids.iter().any(|id| !valid_selection_id(id))
    {
        return String::new();
    }
    let mut unique_literals = Vec::with_capacity(offered_ids.len());
    let mut seen = HashSet::with_capacity(offered_ids.len());
    for id in offered_ids {
        if seen.insert(*id) {
            let json_tok = json_string_token(id);
            unique_literals.push(escape_gbnf_terminal(&json_tok));
        }
    }

    if unique_literals.is_empty() {
        return String::from(
            "root ::= ws \"{\" ws \"\\\"select\\\"\" ws \":\" ws \"[\" ws \"]\" ws \"}\" ws\nws ::= [ \\t\\n\\r]*\n",
        );
    }

    let id_alternatives = unique_literals.join(" | ");
    format!(
        "root ::= ws \"{{\" ws \"\\\"select\\\"\" ws \":\" ws select-array ws \"}}\" ws\n\
         select-array ::= \"[\" ws \"]\" | \"[\" ws id ws \"]\" | \"[\" ws id ws \",\" ws id ws \"]\" | \"[\" ws id ws \",\" ws id ws \",\" ws id ws \"]\"\n\
         id ::= {id_alternatives}\n\
         ws ::= [ \\t\\n\\r]*\n"
    )
}

/// Valida la selección JSON del SLM y construye [`AiNote`] desde las frases ofrecidas.
///
/// - Si la selección es `{"select": []}`, devuelve `Ok(None)` (nota omitida sin error).
/// - Si algún ID no fue ofrecido, está duplicado, supera 3 elementos o el JSON contiene
///   campos extra o formato inválido, devuelve un código constante `&'static str`.
pub fn validate_selection(
    raw_output: &str,
    offered: &[OfferedStatement],
) -> Result<Option<AiNote>, &'static str> {
    if raw_output.len() > MAX_SELECTION_OUTPUT_BYTES {
        return Err(ERR_OUTPUT_TOO_LARGE);
    }

    let parsed: Value = serde_json::from_str(raw_output.trim()).map_err(|_| ERR_INVALID_JSON)?;
    let Value::Object(map) = parsed else {
        return Err(ERR_INVALID_SCHEMA);
    };

    if map.keys().any(|k| k != "select") {
        return Err(ERR_EXTRA_FIELDS);
    }

    let Some(select_val) = map.get("select") else {
        return Err(ERR_INVALID_SCHEMA);
    };
    let Value::Array(items) = select_val else {
        return Err(ERR_INVALID_SCHEMA);
    };

    if items.len() > MAX_SELECTED_STATEMENTS {
        return Err(ERR_TOO_MANY_SELECTIONS);
    }
    if items.is_empty() {
        return Ok(None);
    }

    let mut seen: HashSet<&str> = HashSet::with_capacity(items.len());
    let mut rendered = Vec::with_capacity(items.len());

    for item in items {
        let Value::String(sel_id) = item else {
            return Err(ERR_INVALID_SCHEMA);
        };
        let Some(found) = offered.iter().find(|o| o.selection_id == *sel_id) else {
            return Err(ERR_UNKNOWN_SELECTION);
        };
        if !seen.insert(sel_id.as_str()) {
            return Err(ERR_DUPLICATE_SELECTION);
        }
        rendered.push(RenderedStatement {
            ficha_id: found.ficha_id.clone(),
            statement_id: found.statement_id.clone(),
            text: found.text.clone(),
        });
    }

    Ok(Some(AiNote {
        statements: rendered,
        label: AI_SUGGESTION_LABEL.to_owned(),
    }))
}

/// Comprueba la forma canónica antes de entregar GBNF al parser nativo.
#[cfg(any(feature = "llama", test))]
pub(crate) fn is_known_grammar(grammar: &str) -> bool {
    if grammar == crate::interpreter::PROPOSAL_GBNF {
        return true;
    }
    if grammar == build_selection_grammar(&[]) {
        return true;
    }
    let Some(line) = grammar.lines().find(|line| line.starts_with("id ::= ")) else {
        return false;
    };
    let Some(alternatives) = line.strip_prefix("id ::= ") else {
        return false;
    };
    let mut ids = Vec::new();
    for alternative in alternatives.split(" | ") {
        // Cada alternativa canónica contiene un string JSON.
        let Some(id) = alternative
            .strip_prefix("\"\\\"")
            .and_then(|s| s.strip_suffix("\\\"\""))
        else {
            return false;
        };
        if !valid_selection_id(id) || ids.len() >= MAX_SELECTION_IDS {
            return false;
        }
        ids.push(id);
    }
    grammar == build_selection_grammar(&ids)
}

#[cfg(test)]
mod known_grammar_tests {
    use super::*;

    #[test]
    fn accepts_only_canonical_bounded_grammars() {
        assert!(is_known_grammar(crate::interpreter::PROPOSAL_GBNF));
        assert!(is_known_grammar(&build_selection_grammar(&[])));
        let selection = build_selection_grammar(&["a/b", "X_2.-"]);
        assert!(is_known_grammar(&selection));
        let extra = selection.replace("ws ::=", "id-extra ::= \"extra\"\nws ::=");
        assert!(!is_known_grammar(&extra));
        assert!(!is_known_grammar(&selection.replacen(
            "root ::= ",
            "root ::=  ",
            1
        )));
        let invalid = selection.replace("a/b", "a!b");
        assert!(!is_known_grammar(&invalid));
        assert!(build_selection_grammar(&["bad!"]).is_empty());
        assert!(build_selection_grammar(&["x".repeat(65).as_str()]).is_empty());
        let ids: Vec<String> = (0..=MAX_SELECTION_IDS).map(|i| format!("id{i}")).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        assert!(build_selection_grammar(&refs).is_empty());
        let first: Vec<&str> = refs.iter().take(MAX_SELECTION_IDS).copied().collect();
        let mut oversized = build_selection_grammar(&first);
        oversized = oversized.replace("\nws ::=", " | \"\\\"extra\\\"\"\nws ::=");
        assert!(!is_known_grammar(&oversized));
        let mut exponential = String::from("root ::= n0\n");
        for i in 0..30 {
            exponential.push_str(&format!("n{i} ::= n{} \"a\" | n{} \"b\"\n", i + 1, i + 1));
        }
        exponential.push_str("n30 ::= \"x\"\n");
        assert!(!is_known_grammar(&exponential));
    }
}
