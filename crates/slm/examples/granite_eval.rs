//! Evaluación local de Granite sobre frases sintéticas; el volcado es optativo.
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use merken_core::interpret::quote_number;
use merken_core::proposal::{Proposal, ProposalIssue, TimeHint};
use merken_slm::{
    CancelToken, Control, Deadline, Engine, GenerateRequest, Interpreter, LlamaEngine,
    LlamaEngineConfig, ModelPackage, ModelSlot, ModelStore, Progress, SlmError, SlmInterpreter,
    service::granite_interpreter_manifest,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Expected {
    indicator: String,
    value: Value,
}
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum CaseClass {
    Literal,
    Words,
}

#[derive(Deserialize)]
struct Case {
    class: CaseClass,
    text: String,
    expect: Vec<Expected>,
}

struct MeasuredEngine<'a> {
    engine: &'a mut LlamaEngine,
    input: &'a str,
    json_valid: bool,
    raw_check: bool,
    literal: bool,
    raw: Option<String>,
    issues: Option<Vec<ProposalIssue>>,
}

fn literal_quotes(proposal: &Proposal, input: &str) -> bool {
    let quoted = |s: &str| !s.trim().is_empty() && input.contains(s);
    proposal
        .negated
        .iter()
        .chain(&proposal.uninterpreted)
        .all(|s| quoted(s))
        && proposal.records.iter().all(|record| {
            quoted(&record.quote)
                && (match &record.time {
                    TimeHint::Unspecified => true,
                    TimeHint::Ambiguous { quote } => quoted(quote),
                })
                && record.fields.iter().all(|field| {
                    field
                        .quote
                        .as_deref()
                        .is_none_or(|q| quoted(q) && record.quote.contains(q))
                        && (field.value.is_none() || field.quote.is_some())
                })
        })
}

impl Engine for MeasuredEngine<'_> {
    fn generate(
        &mut self,
        request: &GenerateRequest,
        ctl: &mut Control<'_>,
    ) -> Result<String, SlmError> {
        let raw = self.engine.generate(request, ctl)?;
        self.json_valid = serde_json::from_str::<Value>(&raw).is_ok();
        if let Ok(proposal) = serde_json::from_str::<Proposal>(&raw) {
            let issues = proposal.check(self.input);
            self.raw_check = issues.is_empty();
            self.issues = Some(issues);
            self.literal = literal_quotes(&proposal, self.input);
        }
        self.raw = Some(raw.clone());
        Ok(raw)
    }
}

fn matches_value(actual: &Value, expected: &Value) -> bool {
    match (actual.as_f64(), expected.as_f64()) {
        (Some(a), Some(b)) => (a - b).abs() < 1e-9,
        _ => actual == expected,
    }
}

fn recalled(proposal: &Proposal, expected: &Expected) -> bool {
    proposal.records.iter().any(|record| {
        serde_json::to_value(record.indicator)
            .ok()
            .as_ref()
            .and_then(Value::as_str)
            == Some(expected.indicator.as_str())
            && record.fields.iter().any(|field| {
                field
                    .value
                    .as_ref()
                    .is_some_and(|value| matches_value(value, &expected.value))
            })
    })
}

#[derive(Default)]
struct ClassMetrics {
    cases: usize,
    json: usize,
    raw_check: usize,
    final_check: usize,
    literal_raw: usize,
    hits: usize,
    expected: usize,
    invented: usize,
    no_digit_match: usize,
    errors: usize,
    unresolved: usize,
}

// Count complete numeric tokens, accepting comma decimals without matching substrings.
fn contains_digit_value(input: &str, value: f64) -> bool {
    input
        .split(|c: char| !c.is_ascii_digit() && c != '.' && c != ',')
        .filter(|part| !part.is_empty())
        .any(|part| {
            part.trim_matches(['.', ','])
                .replace(',', ".")
                .parse::<f64>()
                .is_ok_and(|n| (n - value).abs() < 1e-9)
        })
}

fn numeric_counts(proposal: &Proposal, input: &str) -> (usize, usize) {
    let mut invented = 0;
    let mut no_digit_match = 0;
    for field in proposal.records.iter().flat_map(|r| &r.fields) {
        if let Some(value) = field.value.as_ref().and_then(Value::as_f64) {
            let backed = field.quote.as_deref().is_some_and(|quote| {
                input.contains(quote)
                    && quote_number(quote).is_some_and(|n| (n - value).abs() < 1e-9)
            });
            invented += usize::from(!backed);
            no_digit_match += usize::from(!contains_digit_value(input, value));
        }
    }
    (invented, no_digit_match)
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let model_path = PathBuf::from(args.next().ok_or("usage: granite_eval <gguf> <jsonl>")?);
    let data_path = PathBuf::from(args.next().ok_or("usage: granite_eval <gguf> <jsonl>")?);
    let dump_path = match args.next() {
        None => None,
        Some(flag) if flag == "--dump" => Some(PathBuf::from(
            args.next().ok_or("--dump requires a new output file")?,
        )),
        Some(_) => return Err("usage: granite_eval <gguf> <jsonl> [--dump <file>]".into()),
    };
    if args.next().is_some() {
        return Err("usage: granite_eval <gguf> <jsonl> [--dump <file>]".into());
    }
    let dir = model_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let name = model_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("invalid model filename")?;
    let mut manifest = granite_interpreter_manifest();
    if let ModelPackage::Single { file } = &mut manifest.package {
        file.name = name.to_owned();
        // Comparación con otro GGUF de confianza (p. ej. la base Qwen3 del v5 sin LoRA):
        // la identidad esperada se da explícitamente; nunca se calcula a partir del archivo.
        if let (Ok(sha), Ok(bytes)) = (
            std::env::var("MERKEN_EVAL_SHA256"),
            std::env::var("MERKEN_EVAL_BYTES"),
        ) {
            file.sha256 = sha;
            file.bytes = bytes.parse()?;
        }
    }
    let cases: Vec<Case> = fs::read_to_string(data_path)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    if cases.is_empty() {
        return Err("empty dataset".into());
    }
    let mut dump = dump_path
        .map(|path| OpenOptions::new().write(true).create_new(true).open(path))
        .transpose()?
        .map(BufWriter::new);
    let store = ModelStore::new(dir)?;
    let mut slot = ModelSlot::new();
    let mut ctl = Control::without_progress(
        CancelToken::new(),
        Deadline::after(Duration::from_secs(600)),
    );
    let engine = LlamaEngine::load_into(
        &store,
        &manifest,
        &mut slot,
        LlamaEngineConfig::default(),
        &mut ctl,
    )?;
    let (mut json_count, mut raw_count, mut checked_count, mut literal_count) = (0, 0, 0, 0);
    let (mut hits, mut expected_count, mut empty_false_positives, mut errors) = (0, 0, 0, 0);
    let (mut literal_metrics, mut words_metrics) =
        (ClassMetrics::default(), ClassMetrics::default());
    let (mut total_tokens, mut total_seconds) = (0_u64, 0.0_f64);
    println!("case | json | raw check | final check | literal raw | recall | tokens | seconds");
    for (index, case) in cases.iter().enumerate() {
        let measured = MeasuredEngine {
            engine: &mut *engine,
            input: &case.text,
            json_valid: false,
            raw_check: false,
            literal: false,
            raw: None,
            issues: None,
        };
        let mut interpreter = SlmInterpreter::new(measured, &manifest.version);
        let mut tokens = 0_u32;
        let start = Instant::now();
        let result = {
            let mut ctl = Control::new(
                CancelToken::new(),
                Deadline::after(Duration::from_secs(120)),
                |p| {
                    if let Progress::Generating { tokens: n, .. } = p {
                        tokens = n;
                    }
                },
            );
            interpreter.interpret(&case.text, &mut ctl)
        };
        let seconds = start.elapsed().as_secs_f64();
        let metrics = interpreter.engine();
        let (valid, raw_check, literal) = (metrics.json_valid, metrics.raw_check, metrics.literal);
        if let Some(writer) = dump.as_mut() {
            let entry = serde_json::json!({
                "case": index + 1,
                "class": match case.class {
                    CaseClass::Literal => "literal",
                    CaseClass::Words => "words",
                },
                "text": &case.text,
                "raw": &metrics.raw,
                "raw_check_issues": &metrics.issues,
                "error": result.as_ref().err().map(ToString::to_string),
            });
            serde_json::to_writer(&mut *writer, &entry)?;
            writeln!(writer)?;
        }
        let final_check = result
            .as_ref()
            .is_ok_and(|p| p.check(&case.text).is_empty());
        let found = result
            .as_ref()
            .map_or(0, |p| case.expect.iter().filter(|e| recalled(p, e)).count());
        let (invented, no_digit_match) = result
            .as_ref()
            .map_or((0, 0), |proposal| numeric_counts(proposal, &case.text));
        let class_metrics = match case.class {
            CaseClass::Literal => &mut literal_metrics,
            CaseClass::Words => &mut words_metrics,
        };
        class_metrics.cases += 1;
        class_metrics.json += usize::from(valid);
        class_metrics.raw_check += usize::from(raw_check);
        class_metrics.final_check += usize::from(final_check);
        class_metrics.literal_raw += usize::from(literal);
        class_metrics.hits += found;
        class_metrics.expected += case.expect.len();
        class_metrics.invented += invented;
        class_metrics.no_digit_match += no_digit_match;
        class_metrics.errors += usize::from(result.is_err());
        class_metrics.unresolved +=
            usize::from(result.as_ref().is_ok_and(|p| !p.uninterpreted.is_empty()));
        if case.expect.is_empty()
            && result.as_ref().is_ok_and(|p| {
                p.records
                    .iter()
                    .any(|r| r.fields.iter().any(|f| f.value.is_some()))
            })
        {
            empty_false_positives += 1;
        }
        if result.is_err() {
            errors += 1;
        }
        json_count += usize::from(valid);
        raw_count += usize::from(raw_check);
        checked_count += usize::from(final_check);
        literal_count += usize::from(literal);
        hits += found;
        expected_count += case.expect.len();
        total_tokens += u64::from(tokens);
        total_seconds += seconds;
        println!(
            "{:>4} | {:>4} | {:>9} | {:>11} | {:>11} | {}/{} | {:>6} | {:.3}",
            index + 1,
            valid,
            raw_check,
            final_check,
            literal,
            found,
            case.expect.len(),
            tokens,
            seconds
        );
    }
    println!(
        "cases={} json={json_count} raw_check={raw_count} final_check={checked_count} literal_raw={literal_count} errors={errors}",
        cases.len()
    );
    println!(
        "recall={hits}/{expected_count} empty_expected_with_values={empty_false_positives} tokens={total_tokens} seconds={total_seconds:.3}"
    );
    for (name, m) in [("literal", literal_metrics), ("words", words_metrics)] {
        println!(
            "class={name} cases={} json={} raw_check={} final_check={} literal_raw={} recall={}/{} invented={} no_digit_match={} unresolved_cases={} errors={}",
            m.cases,
            m.json,
            m.raw_check,
            m.final_check,
            m.literal_raw,
            m.hits,
            m.expected,
            m.invented,
            m.no_digit_match,
            m.unresolved,
            m.errors
        );
    }
    if let Some(writer) = dump.as_mut() {
        writer.flush()?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    run()
}
