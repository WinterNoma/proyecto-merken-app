//! Ranura única de modelo en memoria (RNF-09): máximo un modelo cargado a la vez.

use crate::{
    control::{Control, Progress},
    engine::Engine,
    error::{EngineErrorCode, SlmError},
    manifest::VerifiedModel,
};

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

    /// Descarga cualquier modelo previo, revalida el paquete y carga `model` mediante `factory`.
    ///
    /// Adquiere el bloqueo de lectura compartido del directorio de `model` y lo mantiene hasta
    /// que `factory` devuelve. Para ejecutar verificación y carga bajo un único bloqueo de
    /// lectura ininterrumpido, usa [`crate::ModelStore::verify_and_load`].
    ///
    /// # Mitigación frente a TOCTOU
    ///
    /// Los modelos residen en el almacenamiento privado de la aplicación Android, donde únicamente
    /// la aplicación tiene permisos de escritura (frontera frente a otros procesos). Dentro del
    /// proceso, [`crate::ModelStore`] serializa las escrituras con un `RwLock` y mantiene el
    /// bloqueo de lectura hasta que `factory` devuelve. Justo antes de invocar `factory`, se
    /// vuelve a comprobar tamaño, fecha de modificación, ausencia de enlaces simbólicos y
    /// confinamiento bajo la raíz canónica.
    pub fn load<F>(
        &mut self,
        model: VerifiedModel,
        ctl: &mut Control<'_>,
        factory: F,
    ) -> Result<&mut E, SlmError>
    where
        F: FnMut(&VerifiedModel, &mut Control<'_>) -> Result<E, SlmError>,
    {
        let dir = model.dir().to_path_buf();
        crate::store::with_dir_read_lock(&dir, ctl, |ctl| {
            self.load_unlocked(model, ctl, factory).map(|_| ())
        })?;
        self.loaded
            .as_mut()
            .map(|slot| &mut slot.engine)
            .ok_or(SlmError::Engine(EngineErrorCode::Internal))
    }

    pub(crate) fn load_unlocked<F>(
        &mut self,
        model: VerifiedModel,
        ctl: &mut Control<'_>,
        mut factory: F,
    ) -> Result<&mut E, SlmError>
    where
        F: FnMut(&VerifiedModel, &mut Control<'_>) -> Result<E, SlmError>,
    {
        ctl.check()?;
        // RNF-09: liberar primero los pesos actuales antes de reservar los nuevos.
        self.loaded = None;
        ctl.report_and_check(Progress::Loading)?;
        model.revalidate_before_load()?;
        let engine = factory(&model, ctl)?;
        ctl.check()?;
        self.loaded = Some(LoadedSlot { model, engine });
        self.loaded
            .as_mut()
            .map(|slot| &mut slot.engine)
            .ok_or(SlmError::Engine(EngineErrorCode::Internal))
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
        control::{CancelToken, Deadline},
        engine::GenerateRequest,
        manifest::{ModelFile, ModelManifest, ModelPackage, ModelRole},
    };
    use sha2::{Digest, Sha256};
    use std::{
        fmt::Write as _,
        fs,
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, Ordering},
        },
        time::Duration,
    };

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let n = TEMP_SEQ.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "pulsys_slm_slot_{label}_{}_{}",
                std::process::id(),
                n
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(bytes);
        let mut out = String::with_capacity(64);
        for b in h.finalize() {
            write!(out, "{b:02x}").unwrap();
        }
        out
    }

    struct TrackedEngine {
        id: String,
        events: Arc<Mutex<Vec<String>>>,
    }

    impl Engine for TrackedEngine {
        fn generate(
            &mut self,
            _req: &GenerateRequest,
            _ctl: &mut Control<'_>,
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

    fn default_ctl() -> Control<'static> {
        Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)))
    }

    fn verified_single(dir: &TempDir, id: &str, filename: &str) -> VerifiedModel {
        let payload = format!("weights-{id}");
        fs::write(dir.0.join(filename), payload.as_bytes()).unwrap();
        let manifest = ModelManifest {
            model_id: id.into(),
            role: ModelRole::Interpreter,
            version: "1.0".into(),
            package: ModelPackage::Single {
                file: ModelFile {
                    name: filename.into(),
                    bytes: payload.len() as u64,
                    sha256: sha256_hex(payload.as_bytes()),
                },
            },
        };
        let mut ctl = default_ctl();
        manifest.verify(&dir.0, &mut ctl).unwrap()
    }

    fn verified_explainer(dir: &TempDir, id: &str) -> VerifiedModel {
        let base = b"explainer-base-weights";
        let adapter = b"explainer-lora-weights";
        fs::write(dir.0.join("v5-base.gguf"), base).unwrap();
        fs::write(dir.0.join("v5-lora.gguf"), adapter).unwrap();
        let base_sha = sha256_hex(base);
        let manifest = ModelManifest {
            model_id: id.into(),
            role: ModelRole::Explainer,
            version: "5.0".into(),
            package: ModelPackage::BaseWithAdapter {
                base: ModelFile {
                    name: "v5-base.gguf".into(),
                    bytes: base.len() as u64,
                    sha256: base_sha.clone(),
                },
                adapter: ModelFile {
                    name: "v5-lora.gguf".into(),
                    bytes: adapter.len() as u64,
                    sha256: sha256_hex(adapter),
                },
                lora_scale: 1.0,
                compatible_base_sha256: Some(base_sha),
            },
        };
        let mut ctl = default_ctl();
        manifest.verify(&dir.0, &mut ctl).unwrap()
    }

    #[test]
    fn loading_second_model_unloads_previous_before_factory_runs() {
        let dir = TempDir::new("unload_first");
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut slot: ModelSlot<TrackedEngine> = ModelSlot::new();
        assert!(!slot.is_loaded());

        let mut ctl = default_ctl();
        let ev_a = Arc::clone(&events);
        slot.load(
            verified_single(&dir, "granite", "granite.gguf"),
            &mut ctl,
            move |m, _| {
                ev_a.lock().unwrap().push(format!("load:{}", m.model_id()));
                Ok(TrackedEngine {
                    id: m.model_id().to_owned(),
                    events: Arc::clone(&ev_a),
                })
            },
        )
        .unwrap();
        assert_eq!(slot.current().unwrap().model_id(), "granite");

        let ev_b = Arc::clone(&events);
        slot.load(verified_explainer(&dir, "v5"), &mut ctl, move |m, _| {
            ev_b.lock().unwrap().push(format!("load:{}", m.model_id()));
            Ok(TrackedEngine {
                id: m.model_id().to_owned(),
                events: Arc::clone(&ev_b),
            })
        })
        .unwrap();

        assert_eq!(slot.current().unwrap().model_id(), "v5");
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
        let dir = TempDir::new("failed_load");
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut slot: ModelSlot<TrackedEngine> = ModelSlot::new();
        let mut ctl = default_ctl();
        let ev = Arc::clone(&events);
        slot.load(
            verified_single(&dir, "first", "first.gguf"),
            &mut ctl,
            move |m, _| {
                Ok(TrackedEngine {
                    id: m.model_id().to_owned(),
                    events: Arc::clone(&ev),
                })
            },
        )
        .unwrap();

        let res = slot.load(verified_explainer(&dir, "broken"), &mut ctl, |_, _| {
            Err(SlmError::Engine(EngineErrorCode::LoadFailed))
        });
        assert!(matches!(
            res,
            Err(SlmError::Engine(EngineErrorCode::LoadFailed))
        ));
        assert!(!slot.is_loaded());
        assert_eq!(*events.lock().unwrap(), vec!["drop:first"]);
    }

    #[test]
    fn load_revalidates_size_and_mtime_before_calling_factory() {
        let dir = TempDir::new("toctou_recheck");
        let verified = verified_single(&dir, "granite", "granite.gguf");

        // Sustitución posterior a verify(): tamaño distinto.
        fs::write(
            dir.0.join("granite.gguf"),
            b"tampered-payload-different-len",
        )
        .unwrap();

        let mut slot: ModelSlot<TrackedEngine> = ModelSlot::new();
        let mut ctl = default_ctl();
        let mut factory_called = false;
        let res = slot.load(verified, &mut ctl, |_, _| {
            factory_called = true;
            unreachable!("factory must not be called on tampered file");
        });

        assert_eq!(
            res.err(),
            Some(SlmError::HashMismatch {
                file: "granite.gguf".into()
            })
        );
        assert!(!factory_called);
        assert!(!slot.is_loaded());
    }

    #[test]
    fn load_aborts_when_cancelled_during_loading_progress() {
        let dir = TempDir::new("cancel_loading");
        let verified = verified_single(&dir, "granite", "granite.gguf");
        let cancel = CancelToken::new();
        let cancel_cb = cancel.clone();
        let mut ctl = Control::new(
            cancel,
            Deadline::after(Duration::from_secs(5)),
            move |progress| {
                if progress == Progress::Loading {
                    cancel_cb.cancel();
                }
            },
        );

        let mut slot: ModelSlot<TrackedEngine> = ModelSlot::new();
        let mut factory_called = false;
        let res = slot.load(verified, &mut ctl, |_, _| {
            factory_called = true;
            unreachable!("factory must not be called after cancellation");
        });

        assert_eq!(res.err(), Some(SlmError::Cancelled));
        assert!(!factory_called);
        assert!(!slot.is_loaded());
    }
}
