//! Contrato del motor de inferencia SLM y motor simulado para pruebas sin modelo.

use crate::{
    control::{Control, Progress},
    error::SlmError,
};
use std::collections::VecDeque;

/// Temperatura por defecto para ejecución determinista en demo (RNF-08).
pub const DEFAULT_TEMPERATURE: f32 = 0.0;
/// Semilla fija por defecto para reproducibilidad (RNF-08 / evaluación v5).
pub const DEFAULT_SEED: u32 = 3407;
/// Límite de tokens de salida por defecto.
pub const DEFAULT_MAX_TOKENS: u32 = 220;

/// Parámetros de generación entregados al motor de inferencia.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerateRequest {
    pub system: String,
    pub prompt: String,
    pub grammar: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub seed: u32,
}

impl Default for GenerateRequest {
    fn default() -> Self {
        Self {
            system: String::new(),
            prompt: String::new(),
            grammar: None,
            max_tokens: DEFAULT_MAX_TOKENS,
            temperature: DEFAULT_TEMPERATURE,
            seed: DEFAULT_SEED,
        }
    }
}

/// Abstracción del motor de inferencia local (p. ej. llama.cpp, candle, ONNX o mock).
pub trait Engine: Send {
    /// Genera una respuesta respetando el control de cancelación, plazo y progreso.
    fn generate(&mut self, req: &GenerateRequest, ctl: &Control<'_>) -> Result<String, SlmError>;

    /// Identificador del modelo activo en el motor.
    fn model_id(&self) -> &str {
        "unknown"
    }
}

/// Motor simulado con respuestas en cola para pruebas unitarias y de interfaz sin GGUF.
#[derive(Clone, Debug)]
pub struct MockEngine {
    model_id: String,
    queue: VecDeque<Result<String, SlmError>>,
    requests: Vec<GenerateRequest>,
    steps: u32,
}

impl Default for MockEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEngine {
    /// Crea un motor simulado vacío con identificador `mock-slm`.
    pub fn new() -> Self {
        Self {
            model_id: "mock-slm".to_owned(),
            queue: VecDeque::new(),
            requests: Vec::new(),
            steps: 3,
        }
    }

    /// Crea un motor simulado precargado con respuestas exitosas en orden FIFO.
    pub fn with_responses<I, S>(responses: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut engine = Self::new();
        for resp in responses {
            engine.push_response(resp);
        }
        engine
    }

    /// Fija el identificador de modelo reportado por el motor simulado.
    pub fn with_model_id(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = model_id.into();
        self
    }

    /// Configura cuántos pasos de progreso `Generating` se emiten por llamada.
    pub fn with_steps(mut self, steps: u32) -> Self {
        self.steps = steps.max(1);
        self
    }

    /// Encola una respuesta textual para la siguiente llamada a [`Engine::generate`].
    pub fn push_response(&mut self, response: impl Into<String>) {
        self.queue.push_back(Ok(response.into()));
    }

    /// Encola un error simulado para la siguiente llamada a [`Engine::generate`].
    pub fn push_error(&mut self, err: SlmError) {
        self.queue.push_back(Err(err));
    }

    /// Devuelve el historial de peticiones recibidas para inspección en pruebas.
    pub fn requests(&self) -> &[GenerateRequest] {
        &self.requests
    }

    /// Cantidad de respuestas pendientes en cola.
    pub fn remaining(&self) -> usize {
        self.queue.len()
    }
}

impl Engine for MockEngine {
    fn generate(&mut self, req: &GenerateRequest, ctl: &Control<'_>) -> Result<String, SlmError> {
        self.requests.push(req.clone());
        ctl.check()?;

        if req.max_tokens == 0 {
            ctl.report(Progress::Generating {
                tokens: 0,
                max_tokens: 0,
            });
            ctl.check()?;
        } else {
            let total_steps = self.steps.min(req.max_tokens).max(1);
            for step in 1..=total_steps {
                ctl.check()?;
                let tokens = if step == total_steps {
                    req.max_tokens.min(step)
                } else {
                    step
                };
                ctl.report(Progress::Generating {
                    tokens,
                    max_tokens: req.max_tokens,
                });
                ctl.check()?;
            }
        }

        match self.queue.pop_front() {
            Some(result) => result,
            None => Err(SlmError::Engine(
                "MockEngine response queue is empty".to_owned(),
            )),
        }
    }

    fn model_id(&self) -> &str {
        &self.model_id
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
    use crate::control::{CancelToken, Deadline};
    use std::time::Duration;

    #[test]
    fn mock_engine_emits_progress_and_returns_queued_responses() {
        let mut engine = MockEngine::with_responses(["first", "second"]).with_steps(2);
        let mut progress = Vec::new();
        let mut on_progress = |p| progress.push(p);
        let ctl = Control::new(
            CancelToken::new(),
            Deadline::after(Duration::from_secs(5)),
            &mut on_progress,
        );
        let req = GenerateRequest {
            max_tokens: 10,
            ..Default::default()
        };

        assert_eq!(engine.generate(&req, &ctl).unwrap(), "first");
        assert_eq!(engine.generate(&req, &ctl).unwrap(), "second");
        assert_eq!(engine.remaining(), 0);
        assert_eq!(engine.requests().len(), 2);
        assert_eq!(
            progress,
            vec![
                Progress::Generating {
                    tokens: 1,
                    max_tokens: 10
                },
                Progress::Generating {
                    tokens: 2,
                    max_tokens: 10
                },
                Progress::Generating {
                    tokens: 1,
                    max_tokens: 10
                },
                Progress::Generating {
                    tokens: 2,
                    max_tokens: 10
                },
            ]
        );
    }

    #[test]
    fn mock_engine_stops_when_cancelled_mid_generation() {
        let mut engine = MockEngine::with_responses(["never"]).with_steps(4);
        let cancel = CancelToken::new();
        let cancel_on_step = cancel.clone();
        let mut seen = 0u32;
        let mut on_progress = |p| {
            if let Progress::Generating { tokens, .. } = p {
                seen = tokens;
                if tokens == 1 {
                    cancel_on_step.cancel();
                }
            }
        };
        let ctl = Control::new(
            cancel,
            Deadline::after(Duration::from_secs(5)),
            &mut on_progress,
        );
        let req = GenerateRequest {
            max_tokens: 10,
            ..Default::default()
        };
        assert_eq!(engine.generate(&req, &ctl), Err(SlmError::Cancelled));
        assert_eq!(seen, 1);
    }

    #[test]
    fn mock_engine_respects_expired_deadline() {
        let mut engine = MockEngine::with_responses(["never"]);
        let ctl = Control::without_progress(CancelToken::new(), Deadline::expired());
        assert_eq!(
            engine.generate(&GenerateRequest::default(), &ctl),
            Err(SlmError::Timeout)
        );
    }
}
