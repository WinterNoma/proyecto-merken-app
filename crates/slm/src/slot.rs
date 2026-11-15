//! Ranura única de modelo en memoria (RNF-09): máximo un modelo cargado a la vez.

use crate::{engine::Engine, error::SlmError, manifest::VerifiedModel};

struct LoadedSlot<E> {
    model: VerifiedModel,
    engine: E,
}

/// Contenedor que mantiene como máximo un modelo cargado en memoria.
///
/// Al cargar un nuevo modelo, descarga primero el anterior antes de invocar la fábrica
/// para evitar picos simultáneos de RAM (RNF-09).
pub struct ModelSlot<E: Engine> {
    loaded: Option<LoadedSlot<E>>,
}

impl<E: Engine> Default for ModelSlot<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Engine> ModelSlot<E> {
    /// Crea una ranura vacía sin ningún modelo cargado.
    pub fn new() -> Self {
        Self { loaded: None }
    }

    /// Descarga cualquier modelo previo y carga `model` mediante la fábrica inyectada.
    pub fn load<F>(&mut self, model: VerifiedModel, mut factory: F) -> Result<&mut E, SlmError>
    where
        F: FnMut(&VerifiedModel) -> Result<E, SlmError>,
    {
        // RNF-09: liberar primero los pesos actuales antes de reservar los nuevos.
        self.loaded = None;
        let engine = factory(&model)?;
        self.loaded = Some(LoadedSlot { model, engine });
        self.loaded
            .as_mut()
            .map(|slot| &mut slot.engine)
            .ok_or_else(|| SlmError::Engine("Failed to store loaded model".to_owned()))
    }

    /// Descarga el modelo actual si existe. Devuelve `true` si había uno cargado.
    pub fn unload(&mut self) -> bool {
        self.loaded.take().is_some()
    }

    /// Indica si hay un modelo actualmente cargado en memoria.
    pub fn is_loaded(&self) -> bool {
        self.loaded.is_some()
    }

    /// Devuelve los metadatos verificados del modelo actualmente cargado.
    pub fn current(&self) -> Option<&VerifiedModel> {
        self.loaded.as_ref().map(|slot| &slot.model)
    }

    /// Devuelve una referencia al motor cargado, si existe.
    pub fn engine(&self) -> Option<&E> {
        self.loaded.as_ref().map(|slot| &slot.engine)
    }

    /// Devuelve una referencia mutable al motor cargado, si existe.
    pub fn engine_mut(&mut self) -> Option<&mut E> {
        self.loaded.as_mut().map(|slot| &mut slot.engine)
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
    use crate::{
        control::Control,
        engine::GenerateRequest,
        manifest::{ModelRole, VerifiedModel},
    };
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    struct TrackedEngine {
        id: String,
        events: Arc<Mutex<Vec<String>>>,
    }

    impl Engine for TrackedEngine {
        fn generate(
            &mut self,
            _req: &GenerateRequest,
            _ctl: &Control<'_>,
        ) -> Result<String, SlmError> {
            Ok(self.id.clone())
        }

        fn model_id(&self) -> &str {
            &self.id
        }
    }

    impl Drop for TrackedEngine {
        fn drop(&mut self) {
            self.events
                .lock()
                .unwrap()
                .push(format!("drop:{}", self.id));
        }
    }

    fn verified(id: &str, role: ModelRole) -> VerifiedModel {
        VerifiedModel {
            model_id: id.into(),
            role,
            version: "1.0".into(),
            dir: PathBuf::from("."),
            files: vec![],
        }
    }

    #[test]
    fn loading_second_model_unloads_previous_before_factory_runs() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut slot: ModelSlot<TrackedEngine> = ModelSlot::new();
        assert!(!slot.is_loaded());

        let ev_a = Arc::clone(&events);
        slot.load(verified("granite", ModelRole::Interpreter), move |m| {
            ev_a.lock().unwrap().push(format!("load:{}", m.model_id));
            Ok(TrackedEngine {
                id: m.model_id.clone(),
                events: Arc::clone(&ev_a),
            })
        })
        .unwrap();
        assert_eq!(slot.current().unwrap().model_id, "granite");

        let ev_b = Arc::clone(&events);
        slot.load(verified("v5", ModelRole::Explainer), move |m| {
            ev_b.lock().unwrap().push(format!("load:{}", m.model_id));
            Ok(TrackedEngine {
                id: m.model_id.clone(),
                events: Arc::clone(&ev_b),
            })
        })
        .unwrap();

        assert_eq!(slot.current().unwrap().model_id, "v5");
        assert_eq!(
            *events.lock().unwrap(),
            vec!["load:granite", "drop:granite", "load:v5"]
        );

        assert!(slot.unload());
        assert!(!slot.is_loaded());
        assert_eq!(
            *events.lock().unwrap(),
            vec!["load:granite", "drop:granite", "load:v5", "drop:v5"]
        );
    }

    #[test]
    fn failed_load_leaves_slot_empty_after_unloading_previous() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut slot: ModelSlot<TrackedEngine> = ModelSlot::new();
        let ev = Arc::clone(&events);
        slot.load(verified("first", ModelRole::Interpreter), move |m| {
            Ok(TrackedEngine {
                id: m.model_id.clone(),
                events: Arc::clone(&ev),
            })
        })
        .unwrap();

        let res = slot.load(verified("broken", ModelRole::Explainer), |_| {
            Err(SlmError::Engine("boom".into()))
        });
        assert!(matches!(res, Err(SlmError::Engine(_))));
        assert!(!slot.is_loaded());
        assert_eq!(*events.lock().unwrap(), vec!["drop:first"]);
    }
}
