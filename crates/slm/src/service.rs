//! Single-worker SLM service. All generated content remains a proposal.
use crate::{
    AiNote, CancelToken, Control, Deadline, Engine, FactLine, Ficha, Interpreter, ModelFile,
    ModelManifest, ModelPackage, ModelRole, ModelSlot, ModelStore, Progress, SlmError,
    SlmExplainer, SlmInterpreter, offered_statements, render_facts,
};
use pulsys_core::{
    evaluate::Evaluation,
    interpret::{MAX_INPUT_CHARS, interpret},
    proposal::Proposal,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};

pub const V5_ID: &str = "space-health-v5";
const BASE_HASH: &str = "4fce4f8510d16a058a9e6eef90d6ffd992135259e0fcadd42e69b06d1243dea1";
/// Trusted manifest shipped with the application, never loaded from the model directory.
pub fn v5_manifest() -> ModelManifest {
    ModelManifest {
        model_id: V5_ID.into(),
        role: ModelRole::Explainer,
        version: "v5".into(),
        package: ModelPackage::BaseWithAdapter {
            base: ModelFile {
                name: "space-health-base-Q4_K_M.gguf".into(),
                bytes: 1_107_409_120,
                sha256: BASE_HASH.into(),
            },
            adapter: ModelFile {
                name: "space-health-lora-f16.gguf".into(),
                bytes: 34_892_576,
                sha256: "ca3d2b23e39547d0ebafb2e47793b4fba3a2943978e59205328dac47f5dd750f".into(),
            },
            lora_scale: 1.0,
            compatible_base_sha256: Some(BASE_HASH.into()),
        },
    }
}

/// Trusted interpreter manifest embedded in the application.
/// Intérprete con la base Qwen3 del v5 sin LoRA: es el mismo archivo que usan el
/// v5 y las recomendaciones V7, así que en el teléfono no añade espacio.
pub fn base_interpreter_manifest() -> ModelManifest {
    ModelManifest {
        model_id: "space-health-base".into(),
        role: ModelRole::Interpreter,
        version: "base-q4_k_m".into(),
        package: ModelPackage::Single {
            file: ModelFile {
                name: "space-health-base-Q4_K_M.gguf".into(),
                bytes: 1_107_409_120,
                sha256: BASE_HASH.into(),
            },
        },
    }
}

pub fn granite_interpreter_manifest() -> ModelManifest {
    ModelManifest {
        model_id: "granite-4.0-350m".into(),
        role: ModelRole::Interpreter,
        version: "4.0-350m-q4_k_m".into(),
        package: ModelPackage::Single {
            file: ModelFile {
                name: "granite-4.0-350m-Q4_K_M.gguf".into(),
                bytes: 236_985_760,
                sha256: "771c588a49607f274a2bba3185733607ebe6f74b996ab90e2d6bee0d98bcec52".into(),
            },
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceError {
    Busy,
    InvalidValue,
    InvalidRequestId,
    Unavailable,
}
#[derive(Clone, Debug, Serialize)]
pub struct InterpretResponse {
    pub proposal: Proposal,
    pub layer2_proposal: Option<Proposal>,
    pub unresolved: Vec<String>,
    pub layer2: &'static str,
}
#[derive(Clone, Debug, Serialize)]
pub struct ExplainResponse {
    pub facts: Vec<FactLine>,
    pub note: Option<AiNote>,
    pub note_status: &'static str,
    pub model_id: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ModelStatus {
    pub model_id: String,
    pub installed: bool,
    pub verified: bool,
    pub loaded: bool,
    pub available: bool,
}
#[derive(Debug, Serialize)]
pub struct SlmStatus {
    pub interpreter: ModelStatus,
    pub explainer: ModelStatus,
}

/// The factory loads into the single slot, maintaining the store's verify/load lock.
pub trait EngineFactory: Send + 'static {
    type Engine: Engine + 'static;
    fn available(&self) -> bool;
    fn load(
        &mut self,
        store: &ModelStore,
        manifest: &ModelManifest,
        slot: &mut ModelSlot<Self::Engine>,
        ctl: &mut Control<'_>,
    ) -> Result<(), SlmError>;
}
pub struct NoEngineFactory;
impl EngineFactory for NoEngineFactory {
    type Engine = crate::MockEngine;
    fn available(&self) -> bool {
        false
    }
    fn load(
        &mut self,
        _: &ModelStore,
        _: &ModelManifest,
        _: &mut ModelSlot<Self::Engine>,
        _: &mut Control<'_>,
    ) -> Result<(), SlmError> {
        Err(SlmError::ModelMissing)
    }
}
#[cfg(feature = "llama")]
pub struct LlamaFactory;
#[cfg(feature = "llama")]
impl EngineFactory for LlamaFactory {
    type Engine = crate::LlamaEngine;
    fn available(&self) -> bool {
        true
    }
    fn load(
        &mut self,
        store: &ModelStore,
        manifest: &ModelManifest,
        slot: &mut ModelSlot<Self::Engine>,
        ctl: &mut Control<'_>,
    ) -> Result<(), SlmError> {
        crate::LlamaEngine::load_into(
            store,
            manifest,
            slot,
            crate::LlamaEngineConfig::default(),
            ctl,
        )
        .map(|_| ())
    }
}
struct BorrowedEngine<'a, E>(&'a mut E);
impl<E: Engine> Engine for BorrowedEngine<'_, E> {
    fn generate(
        &mut self,
        request: &crate::GenerateRequest,
        ctl: &mut Control<'_>,
    ) -> Result<String, SlmError> {
        self.0.generate(request, ctl)
    }
    fn model_id(&self) -> &str {
        self.0.model_id()
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ServiceProgress {
    pub request_id: String,
    pub phase: &'static str,
    pub tokens: u32,
    pub max_tokens: u32,
}
pub type ProgressCallback = Arc<dyn Fn(ServiceProgress) + Send + Sync>;
type Requests = Arc<Mutex<HashMap<String, Arc<Request>>>>;
struct Request {
    id: Option<String>,
    token: CancelToken,
    worker_failed: Arc<AtomicBool>,
    timed_out: AtomicBool,
    stalled: Arc<AtomicUsize>,
}
impl Request {
    fn timeout(&self) {
        if !self.timed_out.swap(true, Ordering::SeqCst) {
            self.stalled.fetch_add(1, Ordering::SeqCst);
        }
        self.token.cancel();
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        if self.timed_out.load(Ordering::SeqCst) {
            self.stalled.fetch_sub(1, Ordering::SeqCst);
        }
    }
}
fn forget(requests: &Requests, request: &Arc<Request>) {
    if let Some(id) = &request.id
        && let Ok(mut entries) = requests.lock()
        && entries
            .get(id)
            .is_some_and(|entry| Arc::ptr_eq(entry, request))
    {
        entries.remove(id);
    }
}
struct Reply<T> {
    tx: mpsc::Sender<T>,
    request: Arc<Request>,
    requests: Requests,
}
impl<T> Reply<T> {
    fn send(self, value: T) {
        // Completion is visible before the caller receives the result.
        forget(&self.requests, &self.request);
        let _ = self.tx.send(value);
    }
}
impl<T> Drop for Reply<T> {
    fn drop(&mut self) {
        if thread::panicking() {
            self.request.worker_failed.store(true, Ordering::SeqCst);
        }
        forget(&self.requests, &self.request);
    }
}
/// Deterministic fallback lives on the command thread, outside the optional worker.
pub struct Pending<T> {
    rx: Receiver<T>,
    fallback: Box<dyn FnOnce(&'static str) -> T + Send>,
    request: Arc<Request>,
    requests: Requests,
    wait: Duration,
}
impl<T> Pending<T> {
    pub fn wait(self) -> T {
        let wait = self.wait;
        self.recv_timeout(wait)
    }
    pub fn recv_timeout(self, timeout: Duration) -> T {
        match self.rx.recv_timeout(timeout) {
            Ok(value) => value,
            Err(error) => {
                let status = match error {
                    mpsc::RecvTimeoutError::Timeout => {
                        self.request.timeout();
                        "timeout"
                    }
                    mpsc::RecvTimeoutError::Disconnected => "slm_error",
                };
                self.request.token.cancel();
                forget(&self.requests, &self.request);
                (self.fallback)(status)
            }
        }
    }
}
enum JobKind {
    Interpret {
        text: String,
        result: InterpretResponse,
        needed: bool,
        reply: Reply<InterpretResponse>,
    },
    Explain {
        eval: Box<Evaluation>,
        fichas: Vec<Ficha>,
        result: ExplainResponse,
        reply: Reply<ExplainResponse>,
    },
    Status {
        reply: Reply<SlmStatus>,
    },
    Release {
        reply: Reply<bool>,
    },
}
struct Job {
    request: Arc<Request>,
    deadline: Deadline,
    kind: JobKind,
}
struct Worker {
    tx: SyncSender<Job>,
    handle: thread::JoinHandle<()>,
    failed: Arc<AtomicBool>,
}
type SpawnWorker = Arc<dyn Fn(Receiver<Job>) -> thread::JoinHandle<()> + Send + Sync>;
#[derive(Clone)]
pub struct SlmService {
    worker: Arc<Mutex<Worker>>,
    spawn: SpawnWorker,
    requests: Requests,
    stalled: Arc<AtomicUsize>,
}
/// IDs are caller-generated and must not be reused for distinct UI requests.
pub fn valid_request_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
impl SlmService {
    pub fn start<F: EngineFactory>(
        root: PathBuf,
        factory: F,
        progress: Option<ProgressCallback>,
    ) -> Result<(Self, Vec<Ficha>), SlmError> {
        Self::start_with_interpreter(root, factory, progress, None)
    }
    pub fn start_with_interpreter<F: EngineFactory>(
        root: PathBuf,
        factory: F,
        progress: Option<ProgressCallback>,
        interpreter: Option<ModelManifest>,
    ) -> Result<(Self, Vec<Ficha>), SlmError> {
        let fichas = Ficha::vec_from_json(include_str!("../fichas/fichas.json"))?;
        let factory = Arc::new(Mutex::new(factory));
        let spawn: SpawnWorker = Arc::new(move |rx| {
            let (root, factory, progress, interpreter) = (
                root.clone(),
                factory.clone(),
                progress.clone(),
                interpreter.clone(),
            );
            thread::spawn(move || {
                let Ok(mut factory) = factory.lock() else {
                    return;
                };
                // The slot and all replies unwind BEFORE this thread finishes.
                // Keeping the factory guard outside catch_unwind avoids poisoning it.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    worker(root, &mut *factory, rx, progress, interpreter);
                }));
            })
        });
        let (tx, rx) = mpsc::sync_channel(2);
        let handle = spawn(rx);
        Ok((
            Self {
                worker: Arc::new(Mutex::new(Worker {
                    tx,
                    handle,
                    failed: Arc::new(AtomicBool::new(false)),
                })),
                spawn,
                requests: Arc::new(Mutex::new(HashMap::new())),
                stalled: Arc::new(AtomicUsize::new(0)),
            },
            fichas,
        ))
    }
    fn submit<T>(
        &self,
        id: Option<String>,
        deadline: Deadline,
        build: impl FnOnce(Reply<T>) -> JobKind,
    ) -> Result<(Receiver<T>, Arc<Request>), ServiceError> {
        if id.as_deref().is_some_and(|id| !valid_request_id(id)) {
            return Err(ServiceError::InvalidRequestId);
        }
        let mut worker = self.worker.lock().map_err(|_| ServiceError::Unavailable)?;
        if worker.handle.is_finished() {
            // is_finished proves that the old slot was dropped; never overlap engines.
            let (tx, rx) = mpsc::sync_channel(2);
            let previous = std::mem::replace(
                &mut *worker,
                Worker {
                    tx,
                    handle: (self.spawn)(rx),
                    failed: Arc::new(AtomicBool::new(false)),
                },
            );
            let _ = previous.handle.join();
        }
        if worker.failed.load(Ordering::SeqCst) || self.stalled.load(Ordering::SeqCst) != 0 {
            return Err(ServiceError::Busy);
        }
        let request = Arc::new(Request {
            id,
            token: CancelToken::new(),
            worker_failed: worker.failed.clone(),
            timed_out: AtomicBool::new(false),
            stalled: self.stalled.clone(),
        });
        if let Some(id) = &request.id {
            let mut entries = self
                .requests
                .lock()
                .map_err(|_| ServiceError::Unavailable)?;
            if entries.contains_key(id) {
                return Err(ServiceError::Busy);
            }
            entries.insert(id.clone(), request.clone());
        }
        let (tx, rx) = mpsc::channel();
        let kind = build(Reply {
            tx,
            request: request.clone(),
            requests: self.requests.clone(),
        });
        worker
            .tx
            .try_send(Job {
                request: request.clone(),
                deadline,
                kind,
            })
            .map_err(|e| match e {
                TrySendError::Full(_) => ServiceError::Busy,
                TrySendError::Disconnected(_) => ServiceError::Unavailable,
            })?;
        Ok((rx, request))
    }
    pub fn interpret(
        &self,
        request_id: String,
        text: String,
        deadline: Deadline,
    ) -> Result<Pending<InterpretResponse>, ServiceError> {
        if text.chars().count() > MAX_INPUT_CHARS {
            return Err(ServiceError::InvalidValue);
        }
        if !valid_request_id(&request_id) {
            return Err(ServiceError::InvalidRequestId);
        }
        let proposal = interpret(&text);
        let unresolved = unresolved_fragments(&text, &proposal);
        let needed =
            !unresolved.is_empty() || proposal.uninterpreted.iter().any(|u| has_content(u));
        let result = InterpretResponse {
            proposal,
            unresolved,
            layer2_proposal: None,
            layer2: if needed { "unavailable" } else { "not_needed" },
        };
        let fallback = result.clone();
        let wait = wait_limit(&deadline);
        let admission = self.submit(Some(request_id), deadline, |reply| JobKind::Interpret {
            text,
            result,
            needed,
            reply,
        });
        match admission {
            Ok((rx, request)) => Ok(Pending {
                rx,
                request,
                requests: self.requests.clone(),
                wait,
                fallback: Box::new(move |status| InterpretResponse {
                    layer2: status,
                    ..fallback
                }),
            }),
            Err(ServiceError::Unavailable) => Ok(self.ready(InterpretResponse {
                layer2: "slm_error",
                ..fallback
            })),
            Err(e) => Err(e),
        }
    }
    pub fn explain(
        &self,
        request_id: String,
        eval: Evaluation,
        fichas: Vec<Ficha>,
        deadline: Deadline,
    ) -> Result<Pending<ExplainResponse>, ServiceError> {
        if !valid_request_id(&request_id) {
            return Err(ServiceError::InvalidRequestId);
        }
        let result = ExplainResponse {
            facts: render_facts(&eval),
            note: None,
            note_status: "no_fichas",
            model_id: None,
        };
        let fallback = result.clone();
        let wait = wait_limit(&deadline);
        let admission = self.submit(Some(request_id), deadline, |reply| JobKind::Explain {
            eval: Box::new(eval),
            fichas,
            result,
            reply,
        });
        match admission {
            Ok((rx, request)) => Ok(Pending {
                rx,
                request,
                requests: self.requests.clone(),
                wait,
                fallback: Box::new(move |status| ExplainResponse {
                    note_status: status,
                    ..fallback
                }),
            }),
            Err(ServiceError::Unavailable) => Ok(self.ready(ExplainResponse {
                note_status: "slm_error",
                ..fallback
            })),
            Err(e) => Err(e),
        }
    }
    fn ready<T: Send + 'static>(&self, value: T) -> Pending<T> {
        let (tx, rx) = mpsc::channel();
        // A disconnected receiver immediately selects this deterministic fallback.
        let request = Arc::new(Request {
            id: None,
            token: CancelToken::new(),
            worker_failed: Arc::new(AtomicBool::new(false)),
            timed_out: AtomicBool::new(false),
            stalled: self.stalled.clone(),
        });
        drop(tx);
        Pending {
            rx,
            request,
            requests: self.requests.clone(),
            wait: Duration::ZERO,
            fallback: Box::new(move |_| value),
        }
    }
    pub fn status(&self, deadline: Deadline) -> Result<Receiver<SlmStatus>, ServiceError> {
        self.submit(None, deadline, |reply| JobKind::Status { reply })
            .map(|(rx, _)| rx)
    }
    pub fn release(&self, deadline: Deadline) -> Result<Receiver<bool>, ServiceError> {
        self.submit(None, deadline, |reply| JobKind::Release { reply })
            .map(|(rx, _)| rx)
    }
    pub fn cancel(&self, request_id: &str) -> bool {
        if let Ok(entries) = self.requests.lock()
            && let Some(request) = entries.get(request_id)
        {
            request.token.cancel();
            return true;
        }
        false
    }
}
/// Cooperative deadline plus a small command-side response margin.
pub const WAIT_MARGIN: Duration = Duration::from_secs(2);
pub fn wait_limit(deadline: &Deadline) -> Duration {
    deadline
        .remaining()
        .unwrap_or(Duration::from_secs(120))
        .saturating_add(WAIT_MARGIN)
}
fn reason(error: &SlmError) -> &'static str {
    match error {
        SlmError::Cancelled => "cancelled",
        SlmError::Timeout => "timeout",
        SlmError::InvalidOutput(_) => "discarded",
        SlmError::ModelMissing | SlmError::IncompletePair | SlmError::HashMismatch { .. } => {
            "model_unavailable"
        }
        _ => "slm_error",
    }
}
#[path = "service_residual.rs"]
mod residual;
use residual::{has_content, unresolved_fragments};

fn filter_layer2(
    mut extra: Proposal,
    rules: &Proposal,
    unresolved: &[String],
    text: &str,
) -> Option<Proposal> {
    extra.records.retain(|record| {
        !record.quote.is_empty()
            && !rules
                .records
                .iter()
                .any(|r| r.indicator == record.indicator)
            && unresolved
                .iter()
                .chain(&rules.uninterpreted)
                .any(|fragment| fragment.contains(&record.quote))
    });
    // El texto pendiente procede de la capa 1; no presentar listas adicionales del modelo.
    extra.negated.clear();
    extra.uninterpreted.clear();
    if extra.records.is_empty() || !extra.check(text).is_empty() {
        None
    } else {
        Some(extra)
    }
}

fn worker<F: EngineFactory>(
    root: PathBuf,
    factory: &mut F,
    rx: Receiver<Job>,
    progress: Option<ProgressCallback>,
    interpreter: Option<ModelManifest>,
) {
    let mut slot = ModelSlot::<F::Engine>::new();
    let manifest = v5_manifest();
    while let Ok(job) = rx.recv() {
        let callback = progress.clone();
        let request_id = job.request.id.clone();
        let mut ctl = Control::new(job.request.token.clone(), job.deadline, move |p| {
            if let (Some(cb), Some(id)) = (&callback, &request_id) {
                let (phase, tokens, max_tokens) = match p {
                    Progress::Loading => ("loading", 0, 0),
                    Progress::Generating { tokens, max_tokens } => {
                        ("generating", tokens, max_tokens)
                    }
                    Progress::Validating => ("validating", 0, 0),
                    Progress::Done => ("done", 0, 0),
                };
                cb(ServiceProgress {
                    request_id: id.clone(),
                    phase,
                    tokens,
                    max_tokens,
                });
            }
        });
        match job.kind {
            JobKind::Interpret {
                text,
                mut result,
                needed,
                reply,
            } => {
                let InterpretResponse {
                    proposal,
                    unresolved,
                    layer2,
                    layer2_proposal,
                } = &mut result;
                if let Err(e) = ctl.check() {
                    *layer2 = reason(&e);
                    reply.send(result);
                    continue;
                }
                if needed
                    && factory.available()
                    && let Some(model) = interpreter
                        .as_ref()
                        .filter(|m| m.role == ModelRole::Interpreter)
                {
                    let loaded = slot
                        .current()
                        .is_some_and(|m| m.model_id() == model.model_id);
                    let load = if loaded {
                        Ok(())
                    } else {
                        ModelStore::new(model_dir(&root, &model.model_id))
                            .and_then(|store| factory.load(&store, model, &mut slot, &mut ctl))
                    };
                    match load {
                        Err(e) => *layer2 = reason(&e),
                        Ok(()) => {
                            if let Some(engine) = slot.engine_mut() {
                                let mut l2 =
                                    SlmInterpreter::new(BorrowedEngine(engine), &model.version);
                                match l2.interpret(&text, &mut ctl) {
                                    Err(e) => *layer2 = reason(&e),
                                    Ok(extra) => {
                                        let filtered =
                                            filter_layer2(extra, proposal, unresolved, &text);
                                        if let Some(filtered) = filtered {
                                            *layer2_proposal = Some(filtered);
                                            *layer2 = "used";
                                        } else {
                                            *layer2 = "discarded";
                                        }
                                    }
                                }
                            } else {
                                *layer2 = "slm_error";
                            }
                        }
                    }
                }
                reply.send(result);
            }
            JobKind::Explain {
                eval,
                fichas,
                mut result,
                reply,
            } => {
                if let Err(e) = ctl.check() {
                    result.note_status = reason(&e);
                    reply.send(result);
                    continue;
                }
                match offered_statements(&fichas, eval.status) {
                    Ok(offered) if offered.is_empty() => {}
                    Err(_) => result.note_status = "discarded",
                    Ok(_) if !factory.available() => result.note_status = "model_unavailable",
                    Ok(_) => {
                        let loaded = slot
                            .current()
                            .is_some_and(|m| m.model_id() == manifest.model_id);
                        let load = if loaded {
                            Ok(())
                        } else {
                            ModelStore::new(model_dir(&root, V5_ID)).and_then(|store| {
                                factory.load(&store, &manifest, &mut slot, &mut ctl)
                            })
                        };
                        match load {
                            Err(e) => result.note_status = reason(&e),
                            Ok(()) => {
                                if let Some(engine) = slot.engine_mut() {
                                    let mut explainer =
                                        SlmExplainer::new(BorrowedEngine(engine), V5_ID);
                                    match explainer.explain_with_facts(
                                        &eval,
                                        &fichas,
                                        result.facts.clone(),
                                        &mut ctl,
                                    ) {
                                        Ok(explanation) => {
                                            result.note_status = if explanation.note.is_some() {
                                                "available"
                                            } else {
                                                "discarded"
                                            };
                                            result.note = explanation.note;
                                            result.model_id = Some(V5_ID.into());
                                        }
                                        Err(e) => result.note_status = reason(&e),
                                    }
                                } else {
                                    result.note_status = "slm_error";
                                }
                            }
                        }
                    }
                }
                reply.send(result);
            }
            JobKind::Status { reply } => {
                let dir = model_dir(&root, V5_ID);
                let installed = dir.join("space-health-base-Q4_K_M.gguf").is_file()
                    && dir.join("space-health-lora-f16.gguf").is_file();
                let verified = installed
                    && ModelStore::new(&dir)
                        .and_then(|s| s.verify(&manifest, &mut ctl))
                        .is_ok();
                let loaded = slot
                    .current()
                    .is_some_and(|m| m.model_id() == manifest.model_id);
                let interpreter_status = if let Some(m) = interpreter
                    .as_ref()
                    .filter(|m| m.role == ModelRole::Interpreter)
                {
                    let d = model_dir(&root, &m.model_id);
                    let installed = match &m.package {
                        ModelPackage::Single { file } => d.join(&file.name).is_file(),
                        ModelPackage::BaseWithAdapter { base, adapter, .. } => {
                            d.join(&base.name).is_file() && d.join(&adapter.name).is_file()
                        }
                    };
                    let verified = installed
                        && ModelStore::new(&d)
                            .and_then(|s| s.verify(m, &mut ctl))
                            .is_ok();
                    ModelStatus {
                        model_id: m.model_id.clone(),
                        installed,
                        verified,
                        loaded: slot.current().is_some_and(|v| v.model_id() == m.model_id),
                        available: factory.available(),
                    }
                } else {
                    ModelStatus {
                        model_id: "interpreter".into(),
                        installed: false,
                        verified: false,
                        loaded: false,
                        available: false,
                    }
                };
                reply.send(SlmStatus {
                    interpreter: interpreter_status,
                    explainer: ModelStatus {
                        model_id: V5_ID.into(),
                        installed,
                        verified,
                        loaded,
                        available: factory.available(),
                    },
                });
            }
            JobKind::Release { reply } => {
                reply.send(slot.unload());
            }
        }
    }
}
/// Standard job deadline, including time spent waiting in the queue.
/// Carpeta de un modelo: `<root>/<model_id>/` si existe; si no, la propia raíz.
/// La raíz plana es la de Android, donde el plugin `slm-assets` deja la base y los
/// adaptadores juntos (la base la comparten el intérprete, el v5 y V7). Es seguro
/// porque cada archivo se verifica igual por nombre, tamaño y SHA-256 del manifiesto.
pub fn model_dir(root: &std::path::Path, model_id: &str) -> PathBuf {
    let nested = root.join(model_id);
    if nested.is_dir() {
        nested
    } else {
        root.to_path_buf()
    }
}

pub fn default_deadline() -> Deadline {
    Deadline::after(Duration::from_secs(120))
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
    use pulsys_core::{evaluate::evaluate_all, time::Timestamp};

    #[test]
    fn granite_manifest_is_embedded_and_valid() {
        let manifest = granite_interpreter_manifest();
        assert_eq!(manifest.model_id, "granite-4.0-350m");
        assert_eq!(manifest.version, "4.0-350m-q4_k_m");
        assert_eq!(manifest.role, ModelRole::Interpreter);
        assert!(manifest.validate().is_ok());
        let ModelPackage::Single { file } = manifest.package else {
            panic!("expected single model")
        };
        assert_eq!(file.name, "granite-4.0-350m-Q4_K_M.gguf");
        assert_eq!(file.bytes, 236_985_760);
        assert_eq!(
            file.sha256,
            "771c588a49607f274a2bba3185733607ebe6f74b996ab90e2d6bee0d98bcec52"
        );
    }

    fn next_id() -> String {
        static ID: AtomicUsize = AtomicUsize::new(0);
        format!("test-{}", ID.fetch_add(1, Ordering::SeqCst))
    }

    #[test]
    fn no_model_returns_rules_and_facts() {
        let (service, fichas) = SlmService::start(
            std::env::temp_dir().join("pulsys-slm-no-model"),
            NoEngineFactory,
            None,
        )
        .unwrap();
        assert!(fichas.is_empty());
        let interpretation = service
            .interpret(
                next_id(),
                "pulso 72 y algo desconocido".into(),
                default_deadline(),
            )
            .unwrap()
            .wait();
        assert_eq!(interpretation.layer2, "unavailable");
        assert!(!interpretation.proposal.records.is_empty());
        let eval = evaluate_all(&[], &[], Timestamp::from_millis(0))
            .into_iter()
            .next()
            .unwrap();
        let explanation = service
            .explain(next_id(), eval, fichas, default_deadline())
            .unwrap()
            .wait();
        assert!(!explanation.facts.is_empty());
        assert_eq!(explanation.note_status, "no_fichas");
        assert!(explanation.note.is_none());
        let status = service
            .status(default_deadline())
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(!status.explainer.loaded);
        assert!(!status.explainer.available);
    }

    struct CountingFactory(std::sync::Arc<AtomicUsize>);
    impl EngineFactory for CountingFactory {
        type Engine = crate::MockEngine;
        fn available(&self) -> bool {
            true
        }
        fn load(
            &mut self,
            _: &ModelStore,
            _: &ModelManifest,
            _: &mut ModelSlot<Self::Engine>,
            _: &mut Control<'_>,
        ) -> Result<(), SlmError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(SlmError::ModelMissing)
        }
    }
    #[test]
    fn no_fichas_never_loads_engine() {
        let count = std::sync::Arc::new(AtomicUsize::new(0));
        let (service, fichas) = SlmService::start(
            std::env::temp_dir().join("pulsys-slm-no-fichas"),
            CountingFactory(count.clone()),
            None,
        )
        .unwrap();
        let eval = evaluate_all(&[], &[], Timestamp::from_millis(0))
            .into_iter()
            .next()
            .unwrap();
        let response = service
            .explain(next_id(), eval, fichas, default_deadline())
            .unwrap()
            .wait();
        assert_eq!(response.note_status, "no_fichas");
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn length_limit_is_reported_before_queueing() {
        let (service, _) = SlmService::start(
            std::env::temp_dir().join("pulsys-slm-limit"),
            NoEngineFactory,
            None,
        )
        .unwrap();
        assert!(matches!(
            service.interpret(
                next_id(),
                "a".repeat(MAX_INPUT_CHARS + 1),
                default_deadline()
            ),
            Err(ServiceError::InvalidValue)
        ));
    }
    #[test]
    fn residual_examples() {
        let (service, _) = SlmService::start(
            std::env::temp_dir().join("pulsys-residual"),
            NoEngineFactory,
            None,
        )
        .unwrap();
        for (text, status, expected) in [
            ("pulso 72", "not_needed", vec![]),
            (
                "pulso 72 y no pegué ojo",
                "unavailable",
                vec!["no pegué ojo"],
            ),
            ("pulso 72, y", "not_needed", vec![]),
            // Negación de un indicador medible: cubierta, sin capa 2.
            ("pulso 72 y no tengo fiebre", "not_needed", vec![]),
        ] {
            let response = service
                .interpret(next_id(), text.into(), default_deadline())
                .unwrap()
                .wait();
            assert_eq!(response.layer2, status, "{text}");
            assert_eq!(response.unresolved, expected, "{text}");
            assert_eq!(response.proposal, interpret(text));
            assert!(response.layer2_proposal.is_none());
        }
    }

    fn fixture_manifest(id: &str, role: ModelRole) -> ModelManifest {
        let file = |name: &str| ModelFile {
            name: name.into(),
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
        };
        let package = if role == ModelRole::Explainer {
            ModelPackage::BaseWithAdapter {
                base: file("model.gguf"),
                adapter: file("adapter.gguf"),
                lora_scale: 1.0,
                compatible_base_sha256: Some(file("model.gguf").sha256),
            }
        } else {
            ModelPackage::Single {
                file: file("model.gguf"),
            }
        };
        ModelManifest {
            model_id: id.into(),
            role,
            version: "test".into(),
            package,
        }
    }

    type Gate = (mpsc::Sender<()>, mpsc::Receiver<()>);
    type LoadCount = Arc<AtomicUsize>;
    struct TestFactory {
        responses: Vec<String>,
        count: LoadCount,
        live: LoadCount,
        gate: Option<Gate>,
    }
    struct TestEngine {
        mock: crate::MockEngine,
        live: LoadCount,
        panic_on_generate: bool,
        gate: Option<Gate>,
    }
    impl Drop for TestEngine {
        fn drop(&mut self) {
            self.live.fetch_sub(1, Ordering::SeqCst);
        }
    }
    impl Engine for TestEngine {
        fn generate(
            &mut self,
            req: &crate::GenerateRequest,
            ctl: &mut Control<'_>,
        ) -> Result<String, SlmError> {
            if let Some((entered, release)) = self.gate.take() {
                let _ = entered.send(());
                let _ = release.recv();
            }
            if self.panic_on_generate {
                panic!("test engine unwind");
            }
            self.mock.generate(req, ctl)
        }
        fn model_id(&self) -> &str {
            self.mock.model_id()
        }
    }
    impl EngineFactory for TestFactory {
        type Engine = TestEngine;
        fn available(&self) -> bool {
            true
        }
        fn load(
            &mut self,
            store: &ModelStore,
            manifest: &ModelManifest,
            slot: &mut ModelSlot<Self::Engine>,
            ctl: &mut Control<'_>,
        ) -> Result<(), SlmError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            let fixture = fixture_manifest(&manifest.model_id, manifest.role);
            let panic_on_generate = self.responses.first().is_some_and(|r| r == "__panic__");
            if panic_on_generate {
                self.responses.remove(0);
            }
            let responses = self.responses.clone();
            let live = self.live.clone();
            let mut gate = self.gate.take();
            store
                .verify_and_load(&fixture, slot, ctl, |model, _| {
                    assert_eq!(live.fetch_add(1, Ordering::SeqCst), 0);
                    Ok(TestEngine {
                        live: live.clone(),
                        panic_on_generate,
                        mock: crate::MockEngine::with_responses(responses.clone())
                            .with_model_id(model.model_id()),
                        gate: gate.take(),
                    })
                })
                .map(|_| ())
        }
    }
    #[test]
    fn model_dir_prefers_subfolder_and_falls_back_to_flat_root() {
        let root = std::env::temp_dir().join(format!("pulsys-model-dir-{}", std::process::id()));
        std::fs::create_dir_all(root.join(V5_ID)).unwrap();
        assert_eq!(model_dir(&root, V5_ID), root.join(V5_ID));
        assert_eq!(model_dir(&root, "space-health-base"), root);
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn test_service(responses: Vec<String>, gate: Option<Gate>) -> (SlmService, LoadCount) {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let id = SEQ.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("pulsys-service-test-{}-{id}", std::process::id()));
        for model in ["test-interpreter", V5_ID] {
            let dir = root.join(model);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("model.gguf"), b"abc").unwrap();
            std::fs::write(dir.join("adapter.gguf"), b"abc").unwrap();
        }
        let count = Arc::new(AtomicUsize::new(0));
        let (service, _) = SlmService::start_with_interpreter(
            root,
            TestFactory {
                responses,
                count: count.clone(),
                live: Arc::new(AtomicUsize::new(0)),
                gate,
            },
            None,
            Some(fixture_manifest("test-interpreter", ModelRole::Interpreter)),
        )
        .unwrap();
        (service, count)
    }

    #[test]
    fn mock_filter_and_invalid_output() {
        let input = "pulso 72 y radiación 0.42 uSv/h";
        let json = r#"{"schema_version":1,"records":[{"indicator":"space_radiation","fields":[{"key":"usv_h","value":0.42,"quote":"0.42 uSv/h"}],"time":{"kind":"unspecified"},"quote":"radiación 0.42 uSv/h"},{"indicator":"heart_rate","fields":[{"key":"bpm","value":72,"quote":"72"}],"time":{"kind":"unspecified"},"quote":"pulso 72"}],"negated":[],"uninterpreted":[]}"#;
        let (service, _) = test_service(vec![json.into(), "not json".into()], None);
        let first = service
            .interpret(next_id(), input.into(), default_deadline())
            .unwrap()
            .wait();
        assert_eq!(first.layer2, "used");
        let extra = first.layer2_proposal.unwrap();
        assert_eq!(extra.records.len(), 1);
        assert_eq!(
            extra.records[0].indicator,
            pulsys_core::catalog::Indicator::SpaceRadiation
        );
        assert!(extra.check(input).is_empty());
        assert_eq!(first.proposal, interpret(input));
        let second = service
            .interpret(
                next_id(),
                "pulso 72 y desconocido".into(),
                default_deadline(),
            )
            .unwrap()
            .wait();
        assert_eq!(second.layer2, "discarded");
        assert!(second.layer2_proposal.is_none());
        // Una cita válida para la entrada, pero fuera de los fragmentos pendientes.
        let mut rules = interpret(input);
        rules.uninterpreted.clear();
        assert!(filter_layer2(extra, &rules, &["otro".into()], input).is_none());
    }

    #[test]
    fn blocked_generation_allows_cancel_and_reports_busy() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (service, _) = test_service(vec!["{}".into()], Some((entered_tx, release_rx)));
        let active_id = next_id();
        let first = service
            .interpret(active_id.clone(), "desconocido".into(), default_deadline())
            .unwrap();
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let _second = service
            .interpret(next_id(), "otro".into(), default_deadline())
            .unwrap();
        let _third = service
            .interpret(next_id(), "más".into(), default_deadline())
            .unwrap();
        assert!(matches!(
            service.interpret(next_id(), "llena".into(), default_deadline()),
            Err(ServiceError::Busy)
        ));
        let cancel_service = service.clone();
        assert!(
            std::thread::spawn(move || cancel_service.cancel(&active_id))
                .join()
                .unwrap()
        );
        release_tx.send(()).unwrap();
        let result = first.recv_timeout(Duration::from_secs(5));
        assert_eq!(result.layer2, "cancelled");
        assert_eq!(result.proposal, interpret("desconocido"));
        assert!(result.layer2_proposal.is_none());
    }

    #[test]
    fn timeout_and_engine_failure_fall_back() {
        let (service, _) = test_service(vec![], None);
        let timeout = service
            .interpret(next_id(), "desconocido".into(), Deadline::expired())
            .unwrap()
            .wait();
        assert_eq!(timeout.layer2, "timeout");
        let error = service
            .interpret(next_id(), "desconocido".into(), default_deadline())
            .unwrap()
            .wait();
        assert_eq!(error.layer2, "slm_error");
        assert!(error.layer2_proposal.is_none());
    }

    #[test]
    fn explainer_engine_error_keeps_facts() {
        use crate::{FichaId, Statement, StatementId};
        let (service, _) = test_service(vec![], None);
        let eval = evaluate_all(&[], &[], Timestamp::from_millis(0))
            .into_iter()
            .next()
            .unwrap();
        let ficha = Ficha::new(
            FichaId::new("test").unwrap(),
            vec![
                Statement::new(
                    StatementId::new("line").unwrap(),
                    "Review your recorded data.",
                    vec![eval.status],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let response = service
            .explain(next_id(), eval, vec![ficha], default_deadline())
            .unwrap()
            .wait();
        assert!(!response.facts.is_empty());
        assert!(response.note.is_none());
        assert_eq!(response.note_status, "slm_error");
    }

    #[test]
    fn switching_models_uses_one_slot() {
        use crate::{FichaId, Statement, StatementId};
        let (service, loads) = test_service(vec!["{}".into()], None);
        let eval = evaluate_all(&[], &[], Timestamp::from_millis(0))
            .into_iter()
            .next()
            .unwrap();
        let ficha = Ficha::new(
            FichaId::new("test").unwrap(),
            vec![
                Statement::new(
                    StatementId::new("line").unwrap(),
                    "Review your recorded data.",
                    vec![eval.status],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let _ = service
            .interpret(next_id(), "desconocido".into(), default_deadline())
            .unwrap()
            .wait();
        let status = service
            .status(default_deadline())
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(status.interpreter.loaded && !status.explainer.loaded);
        let explanation = service
            .explain(next_id(), eval, vec![ficha], default_deadline())
            .unwrap()
            .wait();
        assert!(!explanation.facts.is_empty());
        let status = service
            .status(default_deadline())
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(!status.interpreter.loaded && status.explainer.loaded);
        let _ = service
            .interpret(next_id(), "desconocido".into(), default_deadline())
            .unwrap()
            .wait();
        let status = service
            .status(default_deadline())
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(status.interpreter.loaded && !status.explainer.loaded);
        assert_eq!(loads.load(Ordering::SeqCst), 3);
    }

    const RESIDUAL_CASES: &[(&str, &[&str])] = &[
        ("pulso 72", &[]),
        ("pulso 72 y no pegué ojo", &["no pegué ojo"]),
        ("pulso 72 y tos 172 veces", &["tos 172 veces"]),
        ("pulso 72 estoy agotado", &["estoy agotado"]),
        ("no tengo fiebre", &[]),
        ("no headache", &[]),
        ("pulso 72, y", &[]),
        ("dolor de cabeza 6 desde esta mañana", &[]),
        ("glucosa 95 mg/dL y temperatura 37 grados", &[]),
        ("me duele la cabeza un 6 y dormí mal", &[]),
        ("heart rate 72 and glucose 95 mg/dL", &[]),
        ("pulse 72 I feel exhausted", &["I feel exhausted"]),
        ("no dormí bien estoy agotado", &["estoy agotado"]),
        (
            "I did not sleep well I feel exhausted",
            &["I feel exhausted"],
        ),
        ("NO DORMÍ BIEN ESTOY AGOTADO", &["ESTOY AGOTADO"]),
        ("no dormi\u{0301} bien estoy agotado", &["estoy agotado"]),
        ("dormí mal", &[]),
        ("I slept poorly", &[]),
        ("temperatura -0,5", &["-0,5"]),
        ("temperatura - 0,5", &["- 0,5"]),
        ("pulse − 72", &["− 72"]),
        ("pulso + 72", &["+ 72"]),
        ("temperatura -\t0,5", &["-\t0,5"]),
        ("pulse −\t72", &["−\t72"]),
        ("pulso +\t72", &["+\t72"]),
        ("temperatura - \t 0,5", &["- \t 0,5"]),
        ("pulse -72", &["-72"]),
        ("pulse −72", &["−72"]),
        ("pulse +72", &["+72"]),
        ("pulse – 72", &["– 72"]),
        ("pulse —72", &["—72"]),
        ("pulso + setenta", &["+ setenta"]),
        ("pulso ± 72", &["± 72"]),
        ("pulse \u{2212}\u{00a0}72", &["\u{2212}\u{00a0}72"]),
        ("pulso 72%", &["72%"]),
        ("temperatura 36℉", &["36℉"]),
        ("temperatura 36 ℉.", &["36 ℉"]),
        ("temperatura −0.5", &["−0.5"]),
        ("temperatura +36,6", &["+36,6"]),
        ("temperatura 36,6", &[]),
        ("temperatura 36.6", &[]),
        ("dolor - cabeza", &[]),
        ("120/80", &[]),
        ("blood glucose 95", &[]),
        ("estado de ánimo 6", &[]),
        ("BLOOD   GLUCOSE 95", &[]),
        ("estado de a\u{0301}nimo 6", &[]),
        ("frecuencia cardíaca 72", &[]),
    ];

    // Process creation is denied (Access denied / EPERM) in this environment.
    // Expectations are derived from core's extract/lexicon/numbers, not observed runs.
    // Run with --nocapture to inspect ALL layer-1 citations before the assertions.
    #[test]
    fn layer1_citations_and_residual_contract() {
        let proposals: Vec<_> = RESIDUAL_CASES
            .iter()
            .map(|(text, _)| interpret(text))
            .collect();
        for ((text, _), proposal) in RESIDUAL_CASES.iter().zip(&proposals) {
            println!(
                "LAYER1 {text}: {}",
                serde_json::to_string(proposal).unwrap()
            );
        }
        for ((text, expected), proposal) in RESIDUAL_CASES.iter().zip(&proposals) {
            let unresolved = unresolved_fragments(text, proposal);
            assert_eq!(unresolved, *expected, "{text}: core={proposal:?}");
            let needed =
                !unresolved.is_empty() || proposal.uninterpreted.iter().any(|u| has_content(u));
            assert_eq!(needed, !expected.is_empty(), "{text}: core={proposal:?}");
        }
    }

    #[test]
    fn layer1_sleep_field_citations_follow_the_core_extractor() {
        for (text, value, quote) in [
            (
                "no dormí bien estoy agotado",
                "poor",
                "no dormí bien estoy agotado",
            ),
            (
                "I did not sleep well I feel exhausted",
                "poor",
                "I did not sleep well I feel exhausted",
            ),
            ("dormí mal", "poor", "mal"),
            ("I slept poorly", "poor", "poorly"),
        ] {
            let proposal = interpret(text);
            assert_eq!(proposal.records.len(), 1, "{text}: {proposal:?}");
            assert_eq!(proposal.records[0].quote, text);
            let quality = proposal.records[0]
                .fields
                .iter()
                .find(|field| field.key == "quality")
                .unwrap();
            assert_eq!(quality.value, Some(serde_json::json!(value)), "{text}");
            assert_eq!(quality.quote.as_deref(), Some(quote), "{text}");
            assert!(proposal.negated.is_empty(), "{text}: {proposal:?}");
            assert!(proposal.uninterpreted.is_empty(), "{text}: {proposal:?}");
        }
    }

    #[test]
    fn layer1_numeric_and_compound_citations_follow_the_core_extractor() {
        for (text, key, value, quote) in [
            ("temperatura -0,5", "celsius", None, None),
            ("temperatura - 0,5", "celsius", None, None),
            ("pulse − 72", "bpm", None, None),
            ("pulso + 72", "bpm", None, None),
            ("temperatura -\t0,5", "celsius", None, None),
            ("pulse −\t72", "bpm", None, None),
            ("pulso +\t72", "bpm", None, None),
            ("temperatura - \t 0,5", "celsius", None, None),
            ("pulse -72", "bpm", None, None),
            ("pulse −72", "bpm", None, None),
            ("pulse +72", "bpm", None, None),
            ("pulse – 72", "bpm", None, None),
            ("pulse —72", "bpm", None, None),
            ("pulso + setenta", "bpm", None, None),
            ("temperatura −0.5", "celsius", None, None),
            ("temperatura +36,6", "celsius", None, None),
            ("temperatura 36,6", "celsius", Some(36.6), Some("36,6")),
            ("temperatura 36.6", "celsius", Some(36.6), Some("36.6")),
            ("pulso 72", "bpm", Some(72.0), Some("72")),
            ("blood glucose 95", "mg_dl", Some(95.0), Some("95")),
            ("estado de ánimo 6", "score", Some(6.0), Some("6")),
        ] {
            let proposal = interpret(text);
            assert_eq!(proposal.records.len(), 1, "{text}: {proposal:?}");
            assert_eq!(proposal.records[0].quote, text);
            let field = proposal.records[0]
                .fields
                .iter()
                .find(|field| field.key == key)
                .unwrap();
            assert_eq!(
                field.value.as_ref().and_then(|v| v.as_f64()),
                value,
                "{text}"
            );
            assert_eq!(field.quote.as_deref(), quote, "{text}");
            assert!(proposal.negated.is_empty(), "{text}: {proposal:?}");
            assert!(proposal.uninterpreted.is_empty(), "{text}: {proposal:?}");
        }
    }

    #[test]
    fn pending_fragments_keep_general_symbol_prefixes() {
        // Clear value evidence to test the fragment rule independently of which
        // symbols core currently recognizes, with digits, number words and prose.
        for prefix in ["-", "−", "–", "—", "+", "±", "~", "@", "→", "- ±~"] {
            for gap in ["", " ", "\t", " \t "] {
                for word in ["72", "setenta", "agotado"] {
                    let pending = format!("{prefix}{gap}{word}");
                    let text = format!("pulso {pending}");
                    let mut proposal = interpret("pulso 72");
                    proposal.records[0].quote = text.clone();
                    proposal.records[0].fields.clear();
                    assert_eq!(unresolved_fragments(&text, &proposal), [pending], "{text}");
                }
            }
        }
    }

    #[test]
    fn symbol_prefixes_stop_at_sentence_boundaries_and_attached_words() {
        for boundary in [
            ".", ",", ";", ":", "!", "?", "¡", "¿", "\"", "'", "(", ")", "‘", "’", "“", "”", "«",
            "»", "‹", "›", "\n", "\r",
        ] {
            let text = format!("pulso ~{boundary}\t± 72");
            let mut proposal = interpret("pulso 72");
            proposal.records[0].quote = text.clone();
            proposal.records[0].fields.clear();
            assert_eq!(unresolved_fragments(&text, &proposal), ["± 72"], "{text}");
        }
        for text in ["dolor-cabeza", "dolor- cabeza", "dolor±~\tcabeza"] {
            let mut proposal = interpret("dolor de cabeza");
            proposal.records[0].quote = text.into();
            proposal.records[0].fields.clear();
            // "cabeza" is a symptom mention (zone), so it may be covered; what matters
            // is that a symbol attached to a covered word never starts a pending fragment.
            let pending = unresolved_fragments(text, &proposal);
            assert!(
                pending
                    .iter()
                    .all(|f| f.starts_with(|c: char| c.is_alphanumeric())),
                "{text}: {pending:?}"
            );
        }
    }

    #[test]
    fn unsigned_numeric_evidence_cannot_consume_a_symbol_prefixed_value() {
        for literal in [
            "-72",
            "− 72",
            "– 72",
            "—72",
            "+\t72",
            "± 72",
            "~ 72",
            "+ seventy two",
        ] {
            let text = format!("pulse {literal}");
            let mut proposal = interpret("pulse 72");
            proposal.records[0].quote = text.clone();
            // Exercise both the original narrow citation and a broad citation.
            assert_eq!(unresolved_fragments(&text, &proposal), [literal], "{text}");
            proposal.records[0].fields[0].quote = Some(text.clone());
            assert_eq!(unresolved_fragments(&text, &proposal), [literal], "{text}");
        }
    }

    #[test]
    fn hyphen_between_words_is_not_a_numeric_residue() {
        let text = "dolor - cabeza";
        let mut proposal = interpret(text);
        assert_eq!(proposal.records.len(), 1, "{proposal:?}");
        assert_eq!(proposal.records[0].quote, text);
        let score = proposal.records[0]
            .fields
            .iter()
            .find(|field| field.key == "score")
            .unwrap();
        assert!(score.value.is_none());
        assert!(score.quote.is_none());
        // Exercise mention/zone coverage without the free-text description.
        proposal.records[0]
            .fields
            .retain(|field| field.key != "description");
        assert!(proposal.uninterpreted.is_empty(), "{proposal:?}");
        assert!(unresolved_fragments(text, &proposal).is_empty());
    }

    #[test]
    fn broad_scalar_quotes_only_cover_value_expressions() {
        use pulsys_core::proposal::ProposedField;
        // Deliberately widen real layer-1 field quotes, keeping the original value.
        for (text, key, expected) in [
            ("pulso 72 estoy agotado", "bpm", vec!["estoy agotado"]),
            ("dormí bien estoy agotado", "quality", vec!["estoy agotado"]),
            (
                "dormí más o menos estoy agotado",
                "quality",
                vec!["estoy agotado"],
            ),
            (
                "I slept so so I feel exhausted",
                "quality",
                vec!["I feel exhausted"],
            ),
            (
                "dolor en brazo izquierdo estoy agotado",
                "body_zone",
                vec!["estoy agotado"],
            ),
        ] {
            let mut proposal = interpret(text);
            let record = &mut proposal.records[0];
            let field = record.fields.iter().find(|field| field.key == key).unwrap();
            assert!(field.value.is_some(), "{text}: {field:?}");
            record.fields = vec![ProposedField {
                quote: Some(record.quote.clone()),
                ..field.clone()
            }];
            proposal.uninterpreted.clear();
            assert_eq!(unresolved_fragments(text, &proposal), expected, "{text}");
        }
        // A null scalar and a description differing from its quote consume no
        // arbitrary text. A literal free-text value DOES transport that text.
        let text = "headache I feel exhausted";
        let original = interpret(text);
        let description = original.records[0]
            .fields
            .iter()
            .find(|field| field.key == "description")
            .unwrap();
        assert_eq!(description.quote.as_deref(), Some(text));
        assert_eq!(description.value, Some(serde_json::json!(text)));
        assert!(unresolved_fragments(text, &original).is_empty());
        for value in [None, Some(serde_json::json!("headache"))] {
            let mut proposal = original.clone();
            proposal.records[0].fields = vec![ProposedField {
                key: "description".into(),
                value,
                quote: Some(text.into()),
            }];
            assert_eq!(unresolved_fragments(text, &proposal), ["I feel exhausted"]);
        }
    }

    #[test]
    fn compound_mentions_cover_aliases_without_consuming_neighbours() {
        use pulsys_core::{
            catalog::Indicator,
            proposal::{ProposedRecord, TimeHint},
        };
        for (indicator, alias) in [
            (Indicator::HeartRate, "heart rate"),
            (Indicator::HeartRate, "frecuencia cardiaca"),
            (Indicator::Glucose, "blood glucose"),
            (Indicator::Glucose, "blood sugar"),
            (Indicator::Glucose, "azúcar en sangre"),
            (Indicator::Mood, "estado de ánimo"),
            (Indicator::OxygenInBlood, "blood oxygen"),
            (Indicator::SpaceRadiation, "radiación espacial"),
            (Indicator::DistanceFromEarth, "distancia desde la tierra"),
        ] {
            assert!(pulsys_core::interpret::mentions_indicator(indicator, alias));
            let text = format!("misterio {alias} exhausted");
            let mut proposal = interpret("");
            proposal.records.push(ProposedRecord {
                indicator,
                fields: Vec::new(),
                time: TimeHint::Unspecified,
                quote: text.clone(),
            });
            assert_eq!(
                unresolved_fragments(&text, &proposal),
                ["misterio", "exhausted"]
            );
        }
        for (indicator, text, expected) in [
            (
                Indicator::Glucose,
                "misterio glucose exhausted",
                vec!["misterio", "exhausted"],
            ),
            (
                Indicator::Mood,
                "misterio de ánimo exhausted",
                vec!["misterio", "exhausted"],
            ),
        ] {
            let mut proposal = interpret("");
            proposal.records.push(ProposedRecord {
                indicator,
                fields: Vec::new(),
                time: TimeHint::Unspecified,
                quote: text.into(),
            });
            assert_eq!(unresolved_fragments(text, &proposal), expected, "{text}");
        }
    }

    #[test]
    fn queued_cancellation_is_identity_scoped() {
        let (entered, seen) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (service, loads) = test_service(vec!["{}".into()], Some((entered, gate)));
        let b = service
            .interpret("B".into(), "desconocido".into(), default_deadline())
            .unwrap();
        seen.recv_timeout(Duration::from_secs(5)).unwrap();
        let a = service
            .interpret("A".into(), "otro".into(), default_deadline())
            .unwrap();
        assert!(service.cancel("A"));
        assert!(!service.requests.lock().unwrap()["B"].token.is_cancelled());
        release.send(()).unwrap();
        assert_eq!(b.wait().layer2, "discarded");
        assert_eq!(a.wait().layer2, "cancelled");
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert!(!service.cancel("A"));
    }

    #[test]
    fn late_cancellation_does_not_touch_running_successor() {
        let (entered, seen) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (service, _) = test_service(vec!["{}".into()], Some((entered, gate)));
        assert_eq!(
            service
                .interpret("A".into(), "pulso 72".into(), default_deadline())
                .unwrap()
                .wait()
                .layer2,
            "not_needed"
        );
        let b = service
            .interpret("B".into(), "desconocido".into(), default_deadline())
            .unwrap();
        seen.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(!service.cancel("A"));
        assert!(!service.cancel("unknown"));
        assert!(!service.requests.lock().unwrap()["B"].token.is_cancelled());
        release.send(()).unwrap();
        assert_eq!(b.wait().layer2, "discarded");
    }

    #[test]
    fn command_wait_times_out_while_native_work_remains_busy() {
        let (entered, seen) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (service, _) = test_service(vec!["{}".into()], Some((entered, gate)));
        let text = "pulso 72 estoy agotado";
        let pending = service
            .interpret("A".into(), text.into(), default_deadline())
            .unwrap();
        seen.recv_timeout(Duration::from_secs(5)).unwrap();
        let response = pending.recv_timeout(Duration::from_millis(5));
        assert_eq!(response.layer2, "timeout");
        assert_eq!(response.proposal, interpret(text));
        assert_eq!(response.unresolved, ["estoy agotado"]);
        assert!(!service.cancel("A"));
        assert!(matches!(
            service.interpret("B".into(), "otro".into(), default_deadline()),
            Err(ServiceError::Busy)
        ));
        release.send(()).unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while service.stalled.load(Ordering::SeqCst) != 0 {
            assert!(std::time::Instant::now() < until);
            thread::yield_now();
        }
        assert_eq!(
            service
                .interpret("B".into(), "pulso 72".into(), default_deadline())
                .unwrap()
                .wait()
                .layer2,
            "not_needed"
        );
    }

    struct PanicFactory;
    impl EngineFactory for PanicFactory {
        type Engine = crate::MockEngine;
        fn available(&self) -> bool {
            true
        }
        fn load(
            &mut self,
            _: &ModelStore,
            _: &ModelManifest,
            _: &mut ModelSlot<Self::Engine>,
            _: &mut Control<'_>,
        ) -> Result<(), SlmError> {
            panic!("test worker unwind");
        }
    }

    #[test]
    fn worker_unwind_preserves_rules_and_restarts_without_overlap() {
        let (service, _) = SlmService::start_with_interpreter(
            std::env::temp_dir().join("pulsys-worker-unwind"),
            PanicFactory,
            None,
            Some(fixture_manifest("test-interpreter", ModelRole::Interpreter)),
        )
        .unwrap();
        let text = "pulso 72 estoy agotado";
        let response = service
            .interpret("A".into(), text.into(), default_deadline())
            .unwrap()
            .wait();
        assert_eq!(response.layer2, "slm_error");
        assert_eq!(response.proposal, interpret(text));
        assert_eq!(response.unresolved, ["estoy agotado"]);
        assert!(!service.cancel("A"));
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while !service.worker.lock().unwrap().handle.is_finished() {
            assert!(std::time::Instant::now() < until);
            thread::yield_now();
        }
        let response = service
            .interpret("B".into(), "pulso 72".into(), default_deadline())
            .unwrap()
            .wait();
        assert_eq!(response.layer2, "not_needed");
        assert!(!service.cancel("B"));
    }

    #[test]
    fn release_unloads_and_next_model_request_reloads() {
        let (service, loads) = test_service(vec!["{}".into()], None);
        service
            .interpret(next_id(), "otro".into(), default_deadline())
            .unwrap()
            .wait();
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert!(
            service
                .release(default_deadline())
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
        );
        assert!(
            !service
                .status(default_deadline())
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .interpreter
                .loaded
        );
        assert!(
            !service
                .release(default_deadline())
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
        );
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        service
            .interpret(next_id(), "otro".into(), default_deadline())
            .unwrap()
            .wait();
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn request_id_validation_and_duplicate_admission() {
        for invalid in ["", "two words", "ñ", "a/b", &"a".repeat(65)] {
            assert!(!valid_request_id(invalid));
        }
        assert!(valid_request_id(&"a".repeat(64)));
        assert!(valid_request_id("Az09_--"));
        let (entered, seen) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (service, _) = test_service(vec!["{}".into()], Some((entered, gate)));
        assert!(matches!(
            service.interpret("bad id".into(), "otro".into(), default_deadline()),
            Err(ServiceError::InvalidRequestId)
        ));
        let pending = service
            .interpret("same".into(), "otro".into(), default_deadline())
            .unwrap();
        seen.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(
            service.interpret("same".into(), "otro".into(), default_deadline()),
            Err(ServiceError::Busy)
        ));
        release.send(()).unwrap();
        pending.wait();
    }

    #[test]
    fn explainer_unwind_keeps_precomputed_facts() {
        use crate::{FichaId, Statement, StatementId};
        let (service, _) = SlmService::start(
            std::env::temp_dir().join("pulsys-explainer-unwind"),
            PanicFactory,
            None,
        )
        .unwrap();
        let eval = evaluate_all(&[], &[], Timestamp::from_millis(0))
            .into_iter()
            .next()
            .unwrap();
        let expected = render_facts(&eval);
        let ficha = Ficha::new(
            FichaId::new("test").unwrap(),
            vec![
                Statement::new(
                    StatementId::new("line").unwrap(),
                    "Review your recorded data.",
                    vec![eval.status],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let result = service
            .explain("explain-A".into(), eval, vec![ficha], default_deadline())
            .unwrap()
            .wait();
        assert_eq!(result.facts, expected);
        assert_eq!(result.note_status, "slm_error");
        assert!(result.note.is_none());
        assert!(!service.cancel("explain-A"));
    }

    #[test]
    fn progress_is_identified_and_contains_no_text() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let seen = events.clone();
        let (service, _) = SlmService::start_with_interpreter(
            std::env::temp_dir().join("pulsys-progress-test"),
            ProgressFactory,
            Some(Arc::new(move |event| {
                seen.lock().unwrap().push(event);
            })),
            Some(fixture_manifest("test-interpreter", ModelRole::Interpreter)),
        )
        .unwrap();
        service
            .interpret("progress-A".into(), "otro".into(), default_deadline())
            .unwrap()
            .wait();
        let observed = events.lock().unwrap();
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].request_id, "progress-A");
        assert_eq!(observed[0].phase, "loading");
        let event = ServiceProgress {
            request_id: "progress-A".into(),
            phase: "generating",
            tokens: 4,
            max_tokens: 32,
        };
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({
                "request_id": "progress-A", "phase": "generating", "tokens": 4, "max_tokens": 32,
            })
        );
    }

    struct ProgressFactory;
    impl EngineFactory for ProgressFactory {
        type Engine = crate::MockEngine;
        fn available(&self) -> bool {
            true
        }
        fn load(
            &mut self,
            _: &ModelStore,
            _: &ModelManifest,
            _: &mut ModelSlot<Self::Engine>,
            ctl: &mut Control<'_>,
        ) -> Result<(), SlmError> {
            ctl.report(Progress::Loading);
            Err(SlmError::ModelMissing)
        }
    }

    #[test]
    fn loaded_engine_unwind_drops_slot_before_restarting() {
        let (entered, seen) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (service, loads) = test_service(vec!["__panic__".into()], Some((entered, gate)));
        let pending = service
            .interpret("engine-A".into(), "otro".into(), default_deadline())
            .unwrap();
        seen.recv_timeout(Duration::from_secs(5)).unwrap();
        let eval = evaluate_all(&[], &[], Timestamp::from_millis(0))
            .into_iter()
            .next()
            .unwrap();
        let expected_facts = render_facts(&eval);
        let queued = service
            .explain("queued".into(), eval, vec![], default_deadline())
            .unwrap();
        release.send(()).unwrap();
        let response = pending.wait();
        assert_eq!(response.layer2, "slm_error");
        assert!(!service.cancel("engine-A"));
        let queued = queued.wait();
        assert_eq!(queued.note_status, "slm_error");
        assert_eq!(queued.facts, expected_facts);
        assert!(!service.cancel("queued"));
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while !service.worker.lock().unwrap().handle.is_finished() {
            assert!(std::time::Instant::now() < until);
            thread::yield_now();
        }
        // The factory's live-engine counter asserts zero before every load.
        let second = service
            .interpret("engine-B".into(), "otro".into(), default_deadline())
            .unwrap()
            .wait();
        assert_eq!(second.layer2, "slm_error"); // ordinary mock engine error, no unwind
        let status = service
            .status(default_deadline())
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(status.interpreter.loaded);
        assert_eq!(loads.load(Ordering::SeqCst), 2);
    }
}
