//! Control de ejecución del SLM: cancelación, plazo máximo y progreso (RNF-08).

use crate::error::SlmError;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Testigo clonable y seguro entre hilos para cancelar una inferencia en curso.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Crea un testigo sin cancelar.
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Marca la operación como cancelada.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Devuelve `true` si se solicitó la cancelación.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeadlineState {
    At(Instant),
    Expired,
    Never,
}

type ClockFn = Arc<dyn Fn() -> Instant + Send + Sync>;

/// Plazo máximo para una ejecución del SLM, inyectable con reloj para pruebas.
#[derive(Clone)]
pub struct Deadline {
    state: DeadlineState,
    now_fn: Option<ClockFn>,
}

impl Deadline {
    /// Crea un plazo que vence tras `duration` a partir de `Instant::now()`.
    pub fn after(duration: Duration) -> Self {
        Self::from_start(Instant::now(), duration)
    }

    /// Crea un plazo que vence en el instante absoluto indicado.
    pub fn at(instant: Instant) -> Self {
        Self {
            state: DeadlineState::At(instant),
            now_fn: None,
        }
    }

    /// Crea un plazo a partir de un instante inicial y una duración.
    pub fn from_start(start: Instant, duration: Duration) -> Self {
        let state = match start.checked_add(duration) {
            Some(at) => DeadlineState::At(at),
            None => DeadlineState::Never,
        };
        Self {
            state,
            now_fn: None,
        }
    }

    /// Crea un plazo con un reloj inyectable para pruebas deterministas sin esperas.
    pub fn with_clock(start: Instant, duration: Duration, now_fn: ClockFn) -> Self {
        let state = match start.checked_add(duration) {
            Some(at) => DeadlineState::At(at),
            None => DeadlineState::Never,
        };
        Self {
            state,
            now_fn: Some(now_fn),
        }
    }

    /// Crea un plazo ya vencido de forma determinista para pruebas.
    pub fn expired() -> Self {
        Self {
            state: DeadlineState::Expired,
            now_fn: None,
        }
    }

    fn current_now(&self) -> Instant {
        self.now_fn.as_ref().map_or_else(Instant::now, |f| f())
    }

    /// Indica si el plazo ha vencido tomando el reloj configurado.
    pub fn is_expired(&self) -> bool {
        self.is_expired_at(self.current_now())
    }

    /// Indica si el plazo ha vencido respecto al instante `now` proporcionado.
    pub fn is_expired_at(&self, now: Instant) -> bool {
        match self.state {
            DeadlineState::At(limit) => now >= limit,
            DeadlineState::Expired => true,
            DeadlineState::Never => false,
        }
    }

    /// Tiempo restante hasta el vencimiento según el reloj configurado.
    pub fn remaining(&self) -> Option<Duration> {
        self.remaining_at(self.current_now())
    }

    /// Tiempo restante hasta el vencimiento respecto al instante `now`.
    pub fn remaining_at(&self, now: Instant) -> Option<Duration> {
        match self.state {
            DeadlineState::At(limit) => Some(limit.saturating_duration_since(now)),
            DeadlineState::Expired => Some(Duration::ZERO),
            DeadlineState::Never => None,
        }
    }

    /// Comprueba el plazo usando el reloj configurado y devuelve [`SlmError::Timeout`] si venció.
    pub fn check(&self) -> Result<(), SlmError> {
        self.check_at(self.current_now())
    }

    /// Comprueba el plazo respecto a `now` y devuelve [`SlmError::Timeout`] si venció.
    pub fn check_at(&self, now: Instant) -> Result<(), SlmError> {
        if self.is_expired_at(now) {
            Err(SlmError::Timeout)
        } else {
            Ok(())
        }
    }
}

impl fmt::Debug for Deadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Deadline")
            .field("expired", &self.is_expired())
            .finish()
    }
}

/// Etapas de progreso notificables a la interfaz durante la ejecución del SLM.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum Progress {
    Loading,
    Generating { tokens: u32, max_tokens: u32 },
    Validating,
    Done,
}

/// Contexto de control compartido con el motor y los adaptadores de alto nivel.
pub struct Control<'a> {
    pub cancel: CancelToken,
    pub deadline: Deadline,
    on_progress: Option<RefCell<&'a mut dyn FnMut(Progress)>>,
}

impl<'a> Control<'a> {
    /// Crea un contexto de control con testigo de cancelación, plazo y callback de progreso.
    pub fn new(
        cancel: CancelToken,
        deadline: Deadline,
        on_progress: &'a mut dyn FnMut(Progress),
    ) -> Self {
        Self {
            cancel,
            deadline,
            on_progress: Some(RefCell::new(on_progress)),
        }
    }

    /// Crea un contexto de control sin callback de progreso.
    pub fn without_progress(cancel: CancelToken, deadline: Deadline) -> Self {
        Self {
            cancel,
            deadline,
            on_progress: None,
        }
    }

    /// Notifica un cambio de progreso si hay un observador registrado.
    pub fn report(&self, progress: Progress) {
        if let Some(cell) = &self.on_progress
            && let Ok(mut cb) = cell.try_borrow_mut()
        {
            cb(progress);
        }
    }

    /// Verifica si se debe abortar por cancelación o vencimiento del plazo.
    pub fn check(&self) -> Result<(), SlmError> {
        if self.cancel.is_cancelled() {
            return Err(SlmError::Cancelled);
        }
        self.deadline.check()
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
    use std::sync::atomic::AtomicU64;

    #[test]
    fn cancel_token_propagates_across_clones() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!token.is_cancelled());
        assert!(!clone.is_cancelled());
        clone.cancel();
        assert!(token.is_cancelled());
        assert!(clone.is_cancelled());
    }

    #[test]
    fn deadline_supports_injected_instant_and_clock() {
        let start = Instant::now();
        let d = Deadline::from_start(start, Duration::from_secs(5));
        assert!(!d.is_expired_at(start));
        assert!(!d.is_expired_at(start + Duration::from_secs(4)));
        assert!(d.is_expired_at(start + Duration::from_secs(5)));
        assert_eq!(
            d.check_at(start + Duration::from_secs(5)),
            Err(SlmError::Timeout)
        );

        let elapsed_secs = Arc::new(AtomicU64::new(0));
        let clock_secs = Arc::clone(&elapsed_secs);
        let d_clock = Deadline::with_clock(
            start,
            Duration::from_secs(2),
            Arc::new(move || start + Duration::from_secs(clock_secs.load(Ordering::SeqCst))),
        );
        assert!(d_clock.check().is_ok());
        elapsed_secs.store(3, Ordering::SeqCst);
        assert_eq!(d_clock.check(), Err(SlmError::Timeout));
    }

    #[test]
    fn progress_serializes_with_expected_stages() {
        let json = serde_json::to_string(&Progress::Generating {
            tokens: 12,
            max_tokens: 220,
        })
        .unwrap();
        assert!(json.contains("\"stage\":\"generating\""));
        assert!(json.contains("\"tokens\":12"));
        assert!(json.contains("\"max_tokens\":220"));
    }

    #[test]
    fn control_checks_cancel_before_deadline_and_emits_progress() {
        let cancel = CancelToken::new();
        let mut events = Vec::new();
        let mut on_progress = |p| events.push(p);
        let ctl = Control::new(cancel.clone(), Deadline::expired(), &mut on_progress);
        assert_eq!(ctl.check(), Err(SlmError::Timeout));
        cancel.cancel();
        assert_eq!(ctl.check(), Err(SlmError::Cancelled));

        ctl.report(Progress::Loading);
        ctl.report(Progress::Done);
        assert_eq!(events, vec![Progress::Loading, Progress::Done]);
    }
}
