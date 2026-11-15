//! Almacén de modelos por directorio con exclusión `RwLock` (P1 TOCTOU, RNF-13).
//!
//! # Frontera de seguridad en Android
//!
//! Los pesos del modelo residen en el almacenamiento privado de la aplicación Android, donde
//! únicamente la propia aplicación tiene permisos de lectura y escritura; esa frontera del
//! sistema operativo impide manipulaciones por procesos externos. Dentro del proceso de la
//! aplicación, [`ModelStore`] coordina todas las escrituras (instalación, actualización y
//! borrado) y las lecturas críticas (verificación SHA-256 + carga en [`ModelSlot`]) mediante
//! un `RwLock` compartido por directorio canónico:
//! - toda instalación o actualización toma el bloqueo de **escritura**;
//! - la verificación y carga mantienen el bloqueo de **lectura** de forma continua hasta que
//!   la fábrica del motor devuelve, garantizando que los bytes verificados coincidan con los
//!   cargados por ruta (p. ej. `llama.cpp`).

use crate::{
    control::Control,
    engine::Engine,
    error::{EngineErrorCode, SlmError},
    manifest::{
        HASH_BUF_BYTES, ModelFile, ModelManifest, ModelPackage, VerifiedModel, is_valid_sha256_hex,
        safe_relative_components,
    },
    slot::ModelSlot,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fmt::Write as _,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

static STORE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, std::sync::Weak<RwLock<()>>>>> =
    OnceLock::new();
static INSTALL_SEQ: AtomicU64 = AtomicU64::new(0);

fn lock_registry() -> &'static Mutex<HashMap<PathBuf, std::sync::Weak<RwLock<()>>>> {
    STORE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lexically_normalize(path: &Path) -> Result<PathBuf, SlmError> {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return Err(SlmError::Unsupported(
                        "Model directory path escapes filesystem root".to_owned(),
                    ));
                }
            }
            Component::Normal(part) => out.push(part),
        }
    }
    Ok(out)
}

fn canonical_dir_key(dir: &Path) -> Result<PathBuf, SlmError> {
    if dir.as_os_str().is_empty() {
        return Err(SlmError::Unsupported(
            "Model directory path cannot be empty".to_owned(),
        ));
    }

    let abs = if dir.is_relative() {
        std::env::current_dir()?.join(dir)
    } else {
        dir.to_path_buf()
    };
    let normalized = lexically_normalize(&abs)?;

    let mut probe = normalized.as_path();
    let mut tail: Vec<OsString> = Vec::new();
    loop {
        match fs::canonicalize(probe) {
            Ok(mut canon) => {
                for part in tail.into_iter().rev() {
                    canon.push(part);
                }
                return Ok(canon);
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let Some(name) = probe.file_name() else {
                    return Err(SlmError::from(err));
                };
                tail.push(name.to_os_string());
                let Some(parent) = probe.parent() else {
                    return Err(SlmError::from(err));
                };
                probe = parent;
            }
            Err(err) => return Err(SlmError::from(err)),
        }
    }
}

pub(crate) fn lock_for_dir(dir: &Path) -> Result<(PathBuf, Arc<RwLock<()>>), SlmError> {
    let key = canonical_dir_key(dir)?;
    let mut map = lock_registry()
        .lock()
        .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;
    map.retain(|_, weak| weak.strong_count() > 0);
    if let Some(existing) = map.get(&key).and_then(std::sync::Weak::upgrade) {
        return Ok((key, existing));
    }
    let created = Arc::new(RwLock::new(()));
    map.insert(key.clone(), Arc::downgrade(&created));
    Ok((key, created))
}

pub(crate) fn with_dir_read_lock<T, F>(dir: &Path, f: F) -> Result<T, SlmError>
where
    F: FnOnce() -> Result<T, SlmError>,
{
    let (_key, lock) = lock_for_dir(dir)?;
    let _guard = lock
        .read()
        .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;
    f()
}

struct TempFilesGuard {
    paths: Vec<PathBuf>,
}

impl TempFilesGuard {
    fn new() -> Self {
        Self { paths: Vec::new() }
    }

    fn track(&mut self, path: PathBuf) {
        self.paths.push(path);
    }

    fn clear_committed(&mut self) {
        self.paths.clear();
    }
}

impl Drop for TempFilesGuard {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}

enum InstallPayload<'a> {
    Bytes(&'a [u8]),
    File(&'a Path),
}

/// Gestor exclusivo por directorio de modelos con sincronización `RwLock`.
///
/// Todas las escrituras en el directorio de modelos deben realizarse a través de esta API,
/// que adquiere el bloqueo de escritura. La verificación y carga en [`ModelSlot`] toman el
/// bloqueo de lectura y lo mantienen hasta que la fábrica del motor termina de cargar.
#[derive(Clone, Debug)]
pub struct ModelStore {
    root: PathBuf,
    lock: Arc<RwLock<()>>,
}

impl ModelStore {
    /// Abre o prepara un almacén ligado al directorio `root` (almacenamiento privado de la app).
    ///
    /// Rechaza rutas vacías, enlaces simbólicos en `root` o rutas existentes que no sean
    /// un directorio.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, SlmError> {
        let raw_root = root.as_ref();
        match fs::symlink_metadata(raw_root) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(SlmError::Unsupported(
                        "Model store root cannot be a symlink".to_owned(),
                    ));
                }
                if !meta.is_dir() {
                    return Err(SlmError::Unsupported(
                        "Model root path must be a directory".to_owned(),
                    ));
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(SlmError::from(err)),
        }

        let (canonical_root, lock) = lock_for_dir(raw_root)?;
        Ok(Self {
            root: canonical_root,
            lock,
        })
    }

    /// Devuelve la ruta canónica del directorio de modelos gestionado por este almacén.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Verifica la integridad del manifiesto bajo bloqueo de lectura.
    pub fn verify(
        &self,
        manifest: &ModelManifest,
        ctl: &mut Control<'_>,
    ) -> Result<VerifiedModel, SlmError> {
        let _read_guard = self
            .lock
            .read()
            .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;
        manifest.verify_unlocked(&self.root, ctl)
    }

    /// Verifica el manifiesto y carga el modelo en `slot` bajo un único bloqueo de lectura.
    ///
    /// El bloqueo de lectura permanece retenido de forma continua desde el inicio de la
    /// verificación SHA-256 hasta que `factory` devuelve, impidiendo instalaciones o
    /// sustituciones concurrentes mientras el motor abre los archivos por ruta.
    pub fn verify_and_load<'a, E: Engine, F>(
        &self,
        manifest: &ModelManifest,
        slot: &'a mut ModelSlot<E>,
        ctl: &mut Control<'_>,
        factory: F,
    ) -> Result<&'a mut E, SlmError>
    where
        F: FnMut(&VerifiedModel, &mut Control<'_>) -> Result<E, SlmError>,
    {
        let _read_guard = self
            .lock
            .read()
            .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;
        let verified = manifest.verify_unlocked(&self.root, ctl)?;
        slot.load_unlocked(verified, ctl, factory)
    }

    /// Carga un [`VerifiedModel`] previamente verificado en `slot` manteniendo el bloqueo
    /// de lectura hasta que `factory` devuelve.
    pub fn load_verified<'a, E: Engine, F>(
        &self,
        model: VerifiedModel,
        slot: &'a mut ModelSlot<E>,
        ctl: &mut Control<'_>,
        factory: F,
    ) -> Result<&'a mut E, SlmError>
    where
        F: FnMut(&VerifiedModel, &mut Control<'_>) -> Result<E, SlmError>,
    {
        if model.dir() != self.root {
            return Err(SlmError::Unsupported(
                "VerifiedModel belongs to a different model store directory".to_owned(),
            ));
        }
        let _read_guard = self
            .lock
            .read()
            .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;
        slot.load_unlocked(model, ctl, factory)
    }

    /// Instala o actualiza un paquete completo desde memoria bajo bloqueo de escritura.
    ///
    /// Espera a cualquier verificación o carga en curso antes de escribir, valida SHA-256 y
    /// tamaño en archivos temporales dentro del almacén y solo reemplaza los archivos finales
    /// si todo el paquete supera la comprobación.
    pub fn install_bytes(
        &self,
        manifest: &ModelManifest,
        files: &[(&str, &[u8])],
        ctl: &mut Control<'_>,
    ) -> Result<VerifiedModel, SlmError> {
        let payloads: Vec<(&str, InstallPayload<'_>)> = files
            .iter()
            .map(|(name, data)| (*name, InstallPayload::Bytes(data)))
            .collect();
        self.install_internal(manifest, &payloads, ctl)
    }

    /// Instala o actualiza un paquete completo copiando desde rutas origen bajo bloqueo de escritura.
    pub fn install_from_files(
        &self,
        manifest: &ModelManifest,
        sources: &[(&str, &Path)],
        ctl: &mut Control<'_>,
    ) -> Result<VerifiedModel, SlmError> {
        let payloads: Vec<(&str, InstallPayload<'_>)> = sources
            .iter()
            .map(|(name, path)| (*name, InstallPayload::File(path)))
            .collect();
        self.install_internal(manifest, &payloads, ctl)
    }

    /// Elimina del directorio del almacén los archivos declarados en `manifest` bajo bloqueo de escritura.
    pub fn remove_package(&self, manifest: &ModelManifest) -> Result<(), SlmError> {
        manifest.validate()?;
        let _write_guard = self
            .lock
            .write()
            .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;

        let specs = package_specs(&manifest.package);
        for spec in specs {
            let parts = safe_relative_components(&spec.name)?;
            let Some(target) = resolve_existing_file_no_symlinks(&self.root, &parts)? else {
                continue;
            };
            fs::remove_file(target)?;
        }
        Ok(())
    }

    fn install_internal(
        &self,
        manifest: &ModelManifest,
        inputs: &[(&str, InstallPayload<'_>)],
        ctl: &mut Control<'_>,
    ) -> Result<VerifiedModel, SlmError> {
        ctl.check()?;
        manifest.validate()?;

        let specs = package_specs(&manifest.package);
        let ordered_pairs = match_specs_and_inputs(&specs, inputs)?;

        let _write_guard = self
            .lock
            .write()
            .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;

        ensure_root_dir_no_symlink(&self.root)?;

        let mut staging = TempFilesGuard::new();
        let mut commits: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(ordered_pairs.len());

        for (spec, payload) in ordered_pairs {
            ctl.check()?;
            if !is_valid_sha256_hex(&spec.sha256) {
                return Err(SlmError::HashMismatch {
                    file: spec.name.clone(),
                });
            }

            let parts = safe_relative_components(&spec.name)?;
            let target_path = ensure_target_parent_no_symlinks(&self.root, &parts)?;
            let parent = target_path.parent().ok_or_else(|| {
                SlmError::Io("Model target path has no parent directory".to_owned())
            })?;

            let seq = INSTALL_SEQ.fetch_add(1, Ordering::SeqCst);
            let tmp_path = parent.join(format!(".merken-installing-{}-{seq}", std::process::id()));
            staging.track(tmp_path.clone());

            write_and_hash_payload(&tmp_path, spec, payload, ctl)?;
            commits.push((tmp_path, target_path));
        }

        ctl.check()?;
        for (tmp_path, target_path) in &commits {
            fs::rename(tmp_path, target_path)?;
        }
        staging.clear_committed();

        manifest.verify_unlocked(&self.root, ctl)
    }
}

fn package_specs(package: &ModelPackage) -> Vec<&ModelFile> {
    match package {
        ModelPackage::Single { file } => vec![file],
        ModelPackage::BaseWithAdapter { base, adapter, .. } => vec![base, adapter],
    }
}

fn match_specs_and_inputs<'a>(
    specs: &[&'a ModelFile],
    inputs: &'a [(&'a str, InstallPayload<'a>)],
) -> Result<Vec<(&'a ModelFile, &'a InstallPayload<'a>)>, SlmError> {
    if inputs.len() != specs.len() {
        return Err(SlmError::Unsupported(
            "Provided files do not match manifest package entries".to_owned(),
        ));
    }

    let mut seen_names = HashSet::with_capacity(inputs.len());
    for (name, _) in inputs {
        if !seen_names.insert(*name) {
            return Err(SlmError::Unsupported(
                "Duplicate file entry provided for installation".to_owned(),
            ));
        }
    }

    let mut out = Vec::with_capacity(specs.len());
    for spec in specs {
        let Some((_, payload)) = inputs.iter().find(|(name, _)| *name == spec.name) else {
            return Err(SlmError::Unsupported(
                "Missing file entry declared in manifest package".to_owned(),
            ));
        };
        out.push((*spec, payload));
    }
    Ok(out)
}

fn ensure_root_dir_no_symlink(root: &Path) -> Result<(), SlmError> {
    match fs::symlink_metadata(root) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(SlmError::Unsupported(
                    "Model store root cannot be a symlink".to_owned(),
                ));
            }
            if !meta.is_dir() {
                return Err(SlmError::Unsupported(
                    "Model root path must be a directory".to_owned(),
                ));
            }
            Ok(())
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
            fs::create_dir_all(root)?;
            let meta = fs::symlink_metadata(root)?;
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return Err(SlmError::Unsupported(
                    "Model store root must be a regular directory".to_owned(),
                ));
            }
            Ok(())
        }
        Err(err) => Err(SlmError::from(err)),
    }
}

fn ensure_target_parent_no_symlinks(
    canonical_root: &Path,
    parts: &[OsString],
) -> Result<PathBuf, SlmError> {
    let mut cur = canonical_root.to_path_buf();
    let last_idx = parts
        .len()
        .checked_sub(1)
        .ok_or_else(|| SlmError::Unsupported("Model file path is invalid".to_owned()))?;

    for (idx, part) in parts.iter().enumerate() {
        cur.push(part);
        if idx == last_idx {
            if let Ok(meta) = fs::symlink_metadata(&cur) {
                if meta.file_type().is_symlink() {
                    return Err(SlmError::Unsupported(
                        "Symlinks are not allowed in model directory".to_owned(),
                    ));
                }
                if !meta.is_file() {
                    return Err(SlmError::Io("Model entry is not a regular file".to_owned()));
                }
            }
            return Ok(cur);
        }

        match fs::symlink_metadata(&cur) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(SlmError::Unsupported(
                        "Symlinks are not allowed in model directory".to_owned(),
                    ));
                }
                if !meta.is_dir() {
                    return Err(SlmError::Io(
                        "Model parent path is not a directory".to_owned(),
                    ));
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                fs::create_dir(&cur)?;
            }
            Err(err) => return Err(SlmError::from(err)),
        }
    }

    Err(SlmError::Unsupported(
        "Model file path is invalid".to_owned(),
    ))
}

fn resolve_existing_file_no_symlinks(
    canonical_root: &Path,
    parts: &[OsString],
) -> Result<Option<PathBuf>, SlmError> {
    let mut cur = canonical_root.to_path_buf();
    let last_idx = parts
        .len()
        .checked_sub(1)
        .ok_or_else(|| SlmError::Unsupported("Model file path is invalid".to_owned()))?;

    for (idx, part) in parts.iter().enumerate() {
        cur.push(part);
        match fs::symlink_metadata(&cur) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(SlmError::Unsupported(
                        "Symlinks are not allowed in model directory".to_owned(),
                    ));
                }
                if idx == last_idx {
                    if !meta.is_file() {
                        return Err(SlmError::Io("Model entry is not a regular file".to_owned()));
                    }
                    return Ok(Some(cur));
                } else if !meta.is_dir() {
                    return Err(SlmError::Io(
                        "Model parent path is not a directory".to_owned(),
                    ));
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(SlmError::from(err)),
        }
    }
    Ok(None)
}

fn write_and_hash_payload(
    tmp_path: &Path,
    spec: &ModelFile,
    payload: &InstallPayload<'_>,
    ctl: &mut Control<'_>,
) -> Result<(), SlmError> {
    let mut dst = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tmp_path)?;

    let mut hasher = Sha256::new();
    let mut total: u64 = 0;

    match payload {
        InstallPayload::Bytes(bytes) => {
            let expected_len = u64::try_from(bytes.len())
                .map_err(|_| SlmError::Io("Byte count conversion failed".to_owned()))?;
            if expected_len != spec.bytes {
                return Err(SlmError::HashMismatch {
                    file: spec.name.clone(),
                });
            }
            for chunk in bytes.chunks(HASH_BUF_BYTES) {
                ctl.check()?;
                dst.write_all(chunk)?;
                hasher.update(chunk);
                let chunk_len = u64::try_from(chunk.len())
                    .map_err(|_| SlmError::Io("Byte count conversion failed".to_owned()))?;
                total = total
                    .checked_add(chunk_len)
                    .ok_or_else(|| SlmError::Io("File byte count overflow".to_owned()))?;
                ctl.check()?;
            }
        }
        InstallPayload::File(src_path) => {
            let mut src = File::open(src_path)?;
            let mut buf = [0_u8; HASH_BUF_BYTES];
            loop {
                ctl.check()?;
                let n = src.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                let chunk = buf
                    .get(..n)
                    .ok_or_else(|| SlmError::Io("Read buffer length exceeded".to_owned()))?;
                let chunk_len = u64::try_from(n)
                    .map_err(|_| SlmError::Io("Byte count conversion failed".to_owned()))?;
                total = total
                    .checked_add(chunk_len)
                    .ok_or_else(|| SlmError::Io("File byte count overflow".to_owned()))?;
                if total > spec.bytes {
                    return Err(SlmError::HashMismatch {
                        file: spec.name.clone(),
                    });
                }
                dst.write_all(chunk)?;
                hasher.update(chunk);
                ctl.check()?;
            }
        }
    }

    dst.flush()?;
    dst.sync_all()?;

    if total != spec.bytes {
        return Err(SlmError::HashMismatch {
            file: spec.name.clone(),
        });
    }

    let mut actual_hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        write!(actual_hex, "{byte:02x}")
            .map_err(|_| SlmError::Io("SHA-256 formatting failed".to_owned()))?;
    }
    if !actual_hex.eq_ignore_ascii_case(&spec.sha256) {
        return Err(SlmError::HashMismatch {
            file: spec.name.clone(),
        });
    }

    Ok(())
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
        manifest::ModelRole,
    };
    use std::{sync::mpsc, thread, time::Duration};

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let n = TEMP_SEQ.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "merken_slm_store_{label}_{}_{}",
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

    fn default_ctl() -> Control<'static> {
        Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(10)))
    }

    struct StubEngine(String);

    impl Engine for StubEngine {
        fn generate(
            &mut self,
            _req: &GenerateRequest,
            _ctl: &mut Control<'_>,
        ) -> Result<String, SlmError> {
            Ok(self.0.clone())
        }

        fn model_id(&self) -> &str {
            &self.0
        }
    }

    fn single_manifest(id: &str, name: &str, bytes: &[u8]) -> ModelManifest {
        ModelManifest {
            model_id: id.to_owned(),
            role: ModelRole::Interpreter,
            version: "1.0".to_owned(),
            package: ModelPackage::Single {
                file: ModelFile {
                    name: name.to_owned(),
                    bytes: bytes.len() as u64,
                    sha256: sha256_hex(bytes),
                },
            },
        }
    }

    #[test]
    fn install_bytes_and_verify_and_load_work_for_pair_and_reject_bad_hash() {
        let dir = TempDir::new("install_pair");
        let store = ModelStore::new(&dir.0).unwrap();
        let base = b"base-weights-v1";
        let lora = b"lora-weights-v1";
        let base_sha = sha256_hex(base);

        let manifest = ModelManifest {
            model_id: "space-health-v5".into(),
            role: ModelRole::Explainer,
            version: "5.0.0".into(),
            package: ModelPackage::BaseWithAdapter {
                base: ModelFile {
                    name: "v5/base.gguf".into(),
                    bytes: base.len() as u64,
                    sha256: base_sha.clone(),
                },
                adapter: ModelFile {
                    name: "v5/lora.gguf".into(),
                    bytes: lora.len() as u64,
                    sha256: sha256_hex(lora),
                },
                lora_scale: 1.0,
                compatible_base_sha256: Some(base_sha),
            },
        };

        let mut ctl = default_ctl();
        let err = store
            .install_bytes(
                &manifest,
                &[("v5/base.gguf", base), ("v5/lora.gguf", b"wrong-lora-xxxx")],
                &mut ctl,
            )
            .unwrap_err();
        assert_eq!(
            err,
            SlmError::HashMismatch {
                file: "v5/lora.gguf".into()
            }
        );
        assert!(!dir.0.join("v5/base.gguf").exists());

        let verified = store
            .install_bytes(
                &manifest,
                &[("v5/base.gguf", base), ("v5/lora.gguf", lora)],
                &mut ctl,
            )
            .unwrap();
        assert_eq!(verified.model_id(), "space-health-v5");

        let mut slot: ModelSlot<StubEngine> = ModelSlot::new();
        let engine = store
            .verify_and_load(&manifest, &mut slot, &mut ctl, |m, _| {
                Ok(StubEngine(m.model_id().to_owned()))
            })
            .unwrap();
        assert_eq!(engine.model_id(), "space-health-v5");
    }

    #[test]
    fn install_from_files_and_remove_package_work() {
        let dir = TempDir::new("install_files");
        let staging = TempDir::new("staging");
        let payload = b"model-from-file-payload";
        let src = staging.0.join("download.gguf");
        fs::write(&src, payload).unwrap();

        let manifest = single_manifest("granite", "granite.gguf", payload);
        let store = ModelStore::new(&dir.0).unwrap();
        let mut ctl = default_ctl();
        let verified = store
            .install_from_files(&manifest, &[("granite.gguf", &src)], &mut ctl)
            .unwrap();
        assert_eq!(verified.base_file().bytes(), payload.len() as u64);
        assert!(dir.0.join("granite.gguf").exists());

        store.remove_package(&manifest).unwrap();
        assert!(!dir.0.join("granite.gguf").exists());
    }

    #[test]
    fn concurrent_install_waits_until_in_flight_load_finishes() {
        let dir = TempDir::new("concurrent_rwlock");
        let payload_v1 = b"model-bytes-version-1-same-len";
        let payload_v2 = b"model-bytes-version-2-same-len";
        let manifest_v1 = single_manifest("granite-v1", "model.gguf", payload_v1);
        let manifest_v2 = single_manifest("granite-v2", "model.gguf", payload_v2);

        let store_a = ModelStore::new(&dir.0).unwrap();
        let store_b = ModelStore::new(&dir.0).unwrap();
        let mut ctl = default_ctl();
        store_a
            .install_bytes(&manifest_v1, &[("model.gguf", payload_v1)], &mut ctl)
            .unwrap();

        let events = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let install_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (started_tx, started_rx) = mpsc::channel();
        let (proceed_tx, proceed_rx) = mpsc::channel();

        let events_load = Arc::clone(&events);
        let manifest_load = manifest_v1.clone();
        let load_handle = thread::spawn(move || {
            let mut slot: ModelSlot<StubEngine> = ModelSlot::new();
            let mut load_ctl = default_ctl();
            store_a
                .verify_and_load(&manifest_load, &mut slot, &mut load_ctl, |m, _| {
                    events_load.lock().unwrap().push("factory_enter");
                    started_tx.send(()).unwrap();
                    proceed_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    let loaded_bytes = fs::read(m.base_file().path()).unwrap();
                    assert_eq!(loaded_bytes, payload_v1);
                    events_load.lock().unwrap().push("factory_exit");
                    Ok(StubEngine(m.model_id().to_owned()))
                })
                .unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let events_install = Arc::clone(&events);
        let done_flag = Arc::clone(&install_done);
        let manifest_install = manifest_v2.clone();
        let install_handle = thread::spawn(move || {
            let mut install_ctl = default_ctl();
            store_b
                .install_bytes(
                    &manifest_install,
                    &[("model.gguf", payload_v2)],
                    &mut install_ctl,
                )
                .unwrap();
            events_install.lock().unwrap().push("install_done");
            done_flag.store(true, Ordering::SeqCst);
        });

        // Mientras `factory` mantiene el bloqueo de lectura, la instalación concurrente espera.
        thread::sleep(Duration::from_millis(80));
        assert!(
            !install_done.load(Ordering::SeqCst),
            "concurrent install must wait for in-flight load to release read lock"
        );

        proceed_tx.send(()).unwrap();
        load_handle.join().unwrap();
        install_handle.join().unwrap();

        assert!(install_done.load(Ordering::SeqCst));
        assert_eq!(
            *events.lock().unwrap(),
            vec!["factory_enter", "factory_exit", "install_done"]
        );

        let verify_store = ModelStore::new(&dir.0).unwrap();
        assert_eq!(
            verify_store.verify(&manifest_v1, &mut ctl),
            Err(SlmError::HashMismatch {
                file: "model.gguf".into()
            })
        );
        assert!(verify_store.verify(&manifest_v2, &mut ctl).is_ok());
    }
}
