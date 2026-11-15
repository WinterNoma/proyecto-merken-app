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
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::OsString,
    fmt::Write as _,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    marker::PhantomData,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(2);

static STORE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, std::sync::Weak<RwLock<()>>>>> =
    OnceLock::new();
static INSTALL_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum LockMode {
    Read,
    Write,
}

#[derive(Clone, Copy)]
struct HeldLock {
    lock_id: usize,
    mode: LockMode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadAccess {
    NeedAcquire,
    AlreadyHeld,
}

thread_local! {
    static HELD_STORE_LOCKS: RefCell<Vec<HeldLock>> = const { RefCell::new(Vec::new()) };
}

fn lock_registry() -> &'static Mutex<HashMap<PathBuf, std::sync::Weak<RwLock<()>>>> {
    STORE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn component_eq(a: Component<'_>, b: Component<'_>) -> bool {
    #[cfg(windows)]
    {
        a.as_os_str().to_string_lossy().to_lowercase()
            == b.as_os_str().to_string_lossy().to_lowercase()
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

fn path_starts_with_components(path: &Path, prefix: &Path) -> bool {
    let mut path_comps = path.components();
    for prefix_comp in prefix.components() {
        let Some(path_comp) = path_comps.next() else {
            return false;
        };
        if !component_eq(path_comp, prefix_comp) {
            return false;
        }
    }
    true
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    path_starts_with_components(a, b) && path_starts_with_components(b, a)
}

fn paths_overlap(a: &Path, b: &Path) -> bool {
    path_starts_with_components(a, b) || path_starts_with_components(b, a)
}

fn read_lock_reentrancy(lock_id: usize) -> Result<ReadAccess, SlmError> {
    HELD_STORE_LOCKS
        .try_with(|cell| {
            let held = cell
                .try_borrow()
                .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;
            if held
                .iter()
                .any(|entry| entry.lock_id == lock_id && entry.mode == LockMode::Write)
            {
                return Err(SlmError::Engine(EngineErrorCode::LockReentrant));
            }
            if held
                .iter()
                .any(|entry| entry.lock_id == lock_id && entry.mode == LockMode::Read)
            {
                return Ok(ReadAccess::AlreadyHeld);
            }
            Ok(ReadAccess::NeedAcquire)
        })
        .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?
}

fn ensure_write_not_reentrant() -> Result<(), SlmError> {
    HELD_STORE_LOCKS
        .try_with(|cell| {
            let held = cell
                .try_borrow()
                .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;
            if !held.is_empty() {
                return Err(SlmError::Engine(EngineErrorCode::LockReentrant));
            }
            Ok(())
        })
        .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?
}

fn push_held_lock(lock_id: usize, mode: LockMode) -> Result<(), SlmError> {
    HELD_STORE_LOCKS
        .try_with(|cell| {
            let mut held = cell
                .try_borrow_mut()
                .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;
            held.push(HeldLock { lock_id, mode });
            Ok(())
        })
        .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?
}

fn pop_held_lock(lock_id: usize, mode: LockMode) {
    let _ = HELD_STORE_LOCKS.try_with(|cell| {
        if let Ok(mut held) = cell.try_borrow_mut()
            && let Some(pos) = held
                .iter()
                .rposition(|entry| entry.lock_id == lock_id && entry.mode == mode)
        {
            held.remove(pos);
        }
    });
}

fn sleep_poll_step(ctl: &Control<'_>) {
    let wait = ctl
        .deadline
        .remaining()
        .map_or(LOCK_POLL_INTERVAL, |rem| rem.min(LOCK_POLL_INTERVAL));
    if !wait.is_zero() {
        std::thread::sleep(wait);
    }
}

enum StoreReadGuard<'a> {
    Owned {
        guard: Option<RwLockReadGuard<'a, ()>>,
        lock_id: usize,
    },
    Nested,
}

impl Drop for StoreReadGuard<'_> {
    fn drop(&mut self) {
        if let Self::Owned { guard, lock_id } = self {
            drop(guard.take());
            pop_held_lock(*lock_id, LockMode::Read);
        }
    }
}

struct StoreWriteGuard<'a> {
    guard: Option<RwLockWriteGuard<'a, ()>>,
    lock_id: usize,
    _marker: PhantomData<&'a ()>,
}

impl Drop for StoreWriteGuard<'_> {
    fn drop(&mut self) {
        drop(self.guard.take());
        pop_held_lock(self.lock_id, LockMode::Write);
    }
}

fn acquire_read_lock<'a>(
    lock: &'a Arc<RwLock<()>>,
    ctl: &mut Control<'_>,
) -> Result<StoreReadGuard<'a>, SlmError> {
    let lock_id = Arc::as_ptr(lock) as usize;
    if read_lock_reentrancy(lock_id)? == ReadAccess::AlreadyHeld {
        ctl.check()?;
        return Ok(StoreReadGuard::Nested);
    }

    loop {
        ctl.check()?;
        match lock.try_read() {
            Ok(guard) => {
                push_held_lock(lock_id, LockMode::Read)?;
                return Ok(StoreReadGuard::Owned {
                    guard: Some(guard),
                    lock_id,
                });
            }
            Err(TryLockError::Poisoned(_)) => {
                return Err(SlmError::Engine(EngineErrorCode::LockPoisoned));
            }
            Err(TryLockError::WouldBlock) => {
                ctl.check()?;
                sleep_poll_step(ctl);
            }
        }
    }
}

fn acquire_write_lock<'a>(
    lock: &'a Arc<RwLock<()>>,
    ctl: &mut Control<'_>,
) -> Result<StoreWriteGuard<'a>, SlmError> {
    let lock_id = Arc::as_ptr(lock) as usize;
    ensure_write_not_reentrant()?;

    loop {
        ctl.check()?;
        match lock.try_write() {
            Ok(guard) => {
                push_held_lock(lock_id, LockMode::Write)?;
                return Ok(StoreWriteGuard {
                    guard: Some(guard),
                    lock_id,
                    _marker: PhantomData,
                });
            }
            Err(TryLockError::Poisoned(_)) => {
                return Err(SlmError::Engine(EngineErrorCode::LockPoisoned));
            }
            Err(TryLockError::WouldBlock) => {
                ctl.check()?;
                sleep_poll_step(ctl);
            }
        }
    }
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
    for (existing_key, weak) in map.iter() {
        if weak.strong_count() == 0 {
            continue;
        }
        if paths_equal(existing_key, &key) {
            if let Some(existing) = weak.upgrade() {
                return Ok((existing_key.clone(), existing));
            }
        } else if paths_overlap(existing_key, &key) {
            return Err(SlmError::Unsupported(
                "Model store root overlaps with an active model store".to_owned(),
            ));
        }
    }
    let created = Arc::new(RwLock::new(()));
    map.insert(key.clone(), Arc::downgrade(&created));
    Ok((key, created))
}

pub(crate) fn with_dir_read_lock<T, F>(
    dir: &Path,
    ctl: &mut Control<'_>,
    f: F,
) -> Result<T, SlmError>
where
    F: FnOnce(&mut Control<'_>) -> Result<T, SlmError>,
{
    let (_key, lock) = lock_for_dir(dir)?;
    let _guard = acquire_read_lock(&lock, ctl)?;
    f(ctl)
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
        let _read_guard = acquire_read_lock(&self.lock, ctl)?;
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
        let _read_guard = acquire_read_lock(&self.lock, ctl)?;
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
        let _read_guard = acquire_read_lock(&self.lock, ctl)?;
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
    ///
    /// Adquiere el bloqueo de escritura de forma acotada y cancelable mediante `ctl`, y rechaza
    /// escrituras reentrantes o cruzadas mientras el hilo actual retenga algún bloqueo de almacén.
    pub fn remove_package(
        &self,
        manifest: &ModelManifest,
        ctl: &mut Control<'_>,
    ) -> Result<(), SlmError> {
        ctl.check()?;
        manifest.validate()?;
        let _write_guard = acquire_write_lock(&self.lock, ctl)?;

        let specs = package_specs(&manifest.package);
        for spec in specs {
            ctl.check()?;
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

        let _write_guard = acquire_write_lock(&self.lock, ctl)?;

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
        control::{CancelToken, Deadline, Progress},
        engine::GenerateRequest,
        manifest::ModelRole,
    };
    use std::{
        sync::{Barrier, mpsc},
        thread,
        time::{Duration, Instant},
    };

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

        store.remove_package(&manifest, &mut ctl).unwrap();
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
        let (contended_tx, contended_rx) = mpsc::channel();
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
        let contended_tx = Mutex::new(Some(contended_tx));
        let install_checks = Arc::new(AtomicU64::new(0));
        let install_handle = thread::spawn(move || {
            let start = Instant::now();
            let checks = Arc::clone(&install_checks);
            let deadline = Deadline::with_clock(
                start,
                Duration::from_secs(10),
                Arc::new(move || {
                    let n = checks.fetch_add(1, Ordering::SeqCst);
                    // Check 0: entrada de install_internal; Check 1: previo al primer try_write;
                    // Check 2: tras el primer WouldBlock dentro del bucle de espera.
                    if n == 2
                        && let Some(tx) = contended_tx.lock().unwrap().take()
                    {
                        tx.send(()).unwrap();
                    }
                    start
                }),
            );
            let mut install_ctl = Control::without_progress(CancelToken::new(), deadline);
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

        // Esperamos de forma determinista a que el escritor haya chocado con el bloqueo de lectura.
        contended_rx.recv_timeout(Duration::from_secs(5)).unwrap();
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

    #[test]
    fn overlapping_parent_and_child_roots_are_rejected_and_released_on_drop() {
        let dir = TempDir::new("overlap_roots");
        let child_dir = dir.0.join("v5");
        let nested_child_dir = child_dir.join("sub");

        let parent_store = ModelStore::new(&dir.0).unwrap();
        let payload = b"parent-v5-weights";
        let manifest_in_child = single_manifest("granite-v5", "v5/base.gguf", payload);
        let mut ctl = default_ctl();
        parent_store
            .install_bytes(&manifest_in_child, &[("v5/base.gguf", payload)], &mut ctl)
            .unwrap();

        let (started_tx, started_rx) = mpsc::channel();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let manifest_load = manifest_in_child.clone();
        let load_handle = thread::spawn(move || {
            let mut slot: ModelSlot<StubEngine> = ModelSlot::new();
            let mut load_ctl = default_ctl();
            parent_store
                .verify_and_load(&manifest_load, &mut slot, &mut load_ctl, |m, _| {
                    started_tx.send(()).unwrap();
                    proceed_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(StubEngine(m.model_id().to_owned()))
                })
                .unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(
            ModelStore::new(&child_dir),
            Err(SlmError::Unsupported(_))
        ));
        assert!(matches!(
            ModelStore::new(&nested_child_dir),
            Err(SlmError::Unsupported(_))
        ));

        proceed_tx.send(()).unwrap();
        load_handle.join().unwrap();

        // Al soltar `parent_store` (al finalizar el hilo), la raíz queda liberada y el hijo abre.
        let child_store = ModelStore::new(&child_dir).unwrap();
        let child_manifest = single_manifest("granite-child", "base.gguf", payload);
        let (child_started_tx, child_started_rx) = mpsc::channel();
        let (child_proceed_tx, child_proceed_rx) = mpsc::channel();
        let child_load_handle = thread::spawn(move || {
            let mut slot: ModelSlot<StubEngine> = ModelSlot::new();
            let mut load_ctl = default_ctl();
            child_store
                .verify_and_load(&child_manifest, &mut slot, &mut load_ctl, |m, _| {
                    child_started_tx.send(()).unwrap();
                    child_proceed_rx
                        .recv_timeout(Duration::from_secs(5))
                        .unwrap();
                    Ok(StubEngine(m.model_id().to_owned()))
                })
                .unwrap();
        });

        child_started_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(matches!(
            ModelStore::new(&dir.0),
            Err(SlmError::Unsupported(_))
        ));
        child_proceed_tx.send(()).unwrap();
        child_load_handle.join().unwrap();

        // Al soltar también el hijo, vuelve a permitirse la raíz padre.
        let _reopened_parent = ModelStore::new(&dir.0).unwrap();

        // Directorios hermanos (incluso con prefijo léxico común) no se solapan.
        let siblings = TempDir::new("siblings");
        let _sib_a = ModelStore::new(siblings.0.join("models")).unwrap();
        let _sib_b = ModelStore::new(siblings.0.join("models_v5")).unwrap();
    }

    #[test]
    fn install_with_deadline_times_out_while_load_holds_read_lock() {
        let dir = TempDir::new("lock_timeout");
        let payload_v1 = b"model-v1-bytes";
        let payload_v2 = b"model-v2-bytes";
        let manifest_v1 = single_manifest("granite-v1", "model.gguf", payload_v1);
        let manifest_v2 = single_manifest("granite-v2", "model.gguf", payload_v2);

        let store_a = ModelStore::new(&dir.0).unwrap();
        let store_b = ModelStore::new(&dir.0).unwrap();
        let mut ctl = default_ctl();
        store_a
            .install_bytes(&manifest_v1, &[("model.gguf", payload_v1)], &mut ctl)
            .unwrap();

        let (started_tx, started_rx) = mpsc::channel();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let manifest_load = manifest_v1.clone();
        let load_handle = thread::spawn(move || {
            let mut slot: ModelSlot<StubEngine> = ModelSlot::new();
            let mut load_ctl = default_ctl();
            store_a
                .verify_and_load(&manifest_load, &mut slot, &mut load_ctl, |m, _| {
                    started_tx.send(()).unwrap();
                    proceed_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(StubEngine(m.model_id().to_owned()))
                })
                .unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let mut timeout_ctl = Control::without_progress(
            CancelToken::new(),
            Deadline::after(Duration::from_millis(25)),
        );
        let res = store_b.install_bytes(
            &manifest_v2,
            &[("model.gguf", payload_v2)],
            &mut timeout_ctl,
        );
        assert_eq!(res, Err(SlmError::Timeout));

        proceed_tx.send(()).unwrap();
        load_handle.join().unwrap();

        assert_eq!(fs::read(dir.0.join("model.gguf")).unwrap(), payload_v1);
    }

    #[test]
    fn install_and_verify_waiting_on_lock_respect_cancel_and_timeout() {
        let dir = TempDir::new("lock_cancel_and_read_timeout");
        let payload_v1 = b"model-v1-bytes";
        let payload_v2 = b"model-v2-bytes";
        let manifest_v1 = single_manifest("granite-v1", "model.gguf", payload_v1);
        let manifest_v2 = single_manifest("granite-v2", "model.gguf", payload_v2);

        let store_a = ModelStore::new(&dir.0).unwrap();
        let store_b = ModelStore::new(&dir.0).unwrap();
        let mut ctl = default_ctl();
        store_a
            .install_bytes(&manifest_v1, &[("model.gguf", payload_v1)], &mut ctl)
            .unwrap();

        // 1. Escritor cancelado mientras un lector mantiene el bloqueo de lectura.
        let (started_tx, started_rx) = mpsc::channel();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let store_reader = store_a.clone();
        let manifest_load = manifest_v1.clone();
        let load_handle = thread::spawn(move || {
            let mut slot: ModelSlot<StubEngine> = ModelSlot::new();
            let mut load_ctl = default_ctl();
            store_reader
                .verify_and_load(&manifest_load, &mut slot, &mut load_ctl, |m, _| {
                    started_tx.send(()).unwrap();
                    proceed_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(StubEngine(m.model_id().to_owned()))
                })
                .unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let cancel = CancelToken::new();
        let cancel_on_wait = cancel.clone();
        let checks = Arc::new(AtomicU64::new(0));
        let checks_clock = Arc::clone(&checks);
        let start = Instant::now();
        let deadline = Deadline::with_clock(
            start,
            Duration::from_secs(5),
            Arc::new(move || {
                if checks_clock.fetch_add(1, Ordering::SeqCst) == 2 {
                    cancel_on_wait.cancel();
                }
                start
            }),
        );
        let mut cancel_ctl = Control::without_progress(cancel, deadline);
        let install_res =
            store_b.install_bytes(&manifest_v2, &[("model.gguf", payload_v2)], &mut cancel_ctl);
        assert_eq!(install_res, Err(SlmError::Cancelled));

        proceed_tx.send(()).unwrap();
        load_handle.join().unwrap();

        // 2. Lector con plazo mientras un escritor mantiene el bloqueo de escritura.
        let (write_started_tx, write_started_rx) = mpsc::channel();
        let (write_proceed_tx, write_proceed_rx) = mpsc::channel();
        let write_started_tx = Mutex::new(Some(write_started_tx));
        let write_proceed_rx = Mutex::new(Some(write_proceed_rx));
        let writer_checks = Arc::new(AtomicU64::new(0));
        let writer_checks_clock = Arc::clone(&writer_checks);
        let writer_start = Instant::now();
        let writer_deadline = Deadline::with_clock(
            writer_start,
            Duration::from_secs(10),
            Arc::new(move || {
                // Check 0: entrada; Check 1: adquisición del write lock; Check 2: ya dentro con lock tomado.
                if writer_checks_clock.fetch_add(1, Ordering::SeqCst) == 2 {
                    if let Some(tx) = write_started_tx.lock().unwrap().take() {
                        tx.send(()).unwrap();
                    }
                    if let Some(rx) = write_proceed_rx.lock().unwrap().take() {
                        rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    }
                }
                writer_start
            }),
        );
        let manifest_writer = manifest_v2.clone();
        let write_handle = thread::spawn(move || {
            let mut writer_ctl = Control::without_progress(CancelToken::new(), writer_deadline);
            store_a
                .install_bytes(
                    &manifest_writer,
                    &[("model.gguf", payload_v2)],
                    &mut writer_ctl,
                )
                .unwrap();
        });

        write_started_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap();

        let mut verify_timeout_ctl = Control::without_progress(
            CancelToken::new(),
            Deadline::after(Duration::from_millis(25)),
        );
        assert_eq!(
            store_b.verify(&manifest_v1, &mut verify_timeout_ctl),
            Err(SlmError::Timeout)
        );

        let read_cancel = CancelToken::new();
        let read_cancel_cb = read_cancel.clone();
        let read_checks = Arc::new(AtomicU64::new(0));
        let read_checks_clock = Arc::clone(&read_checks);
        let read_start = Instant::now();
        let read_deadline = Deadline::with_clock(
            read_start,
            Duration::from_secs(5),
            Arc::new(move || {
                if read_checks_clock.fetch_add(1, Ordering::SeqCst) == 1 {
                    read_cancel_cb.cancel();
                }
                read_start
            }),
        );
        let mut verify_cancel_ctl = Control::without_progress(read_cancel, read_deadline);
        assert_eq!(
            store_b.verify(&manifest_v1, &mut verify_cancel_ctl),
            Err(SlmError::Cancelled)
        );

        write_proceed_tx.send(()).unwrap();
        write_handle.join().unwrap();
    }

    #[test]
    fn reentrant_write_from_factory_or_loading_callback_fails_without_deadlock() {
        let dir = TempDir::new("reentrant_lock");
        let other_dir = TempDir::new("reentrant_other");
        let payload = b"model-bytes-reentrant";
        let manifest = single_manifest("granite", "model.gguf", payload);
        let other_payload = b"other-store-bytes";
        let other_manifest = single_manifest("other", "other.gguf", other_payload);

        let store = ModelStore::new(&dir.0).unwrap();
        let other_store = ModelStore::new(&other_dir.0).unwrap();
        let mut ctl = default_ctl();
        store
            .install_bytes(&manifest, &[("model.gguf", payload)], &mut ctl)
            .unwrap();
        other_store
            .install_bytes(&other_manifest, &[("other.gguf", other_payload)], &mut ctl)
            .unwrap();

        // 1. Reentrada desde `factory` de `verify_and_load`: tanto en el mismo almacén como en
        // otro almacén distinto, cualquier escritura (`remove_package` o `install_bytes`) falla.
        let mut slot: ModelSlot<StubEngine> = ModelSlot::new();
        let mut factory_ctl = default_ctl();
        store
            .verify_and_load(&manifest, &mut slot, &mut factory_ctl, |m, _| {
                let mut inner_ctl = default_ctl();
                assert_eq!(
                    store.remove_package(&manifest, &mut inner_ctl),
                    Err(SlmError::Engine(EngineErrorCode::LockReentrant))
                );
                assert_eq!(
                    store.install_bytes(&manifest, &[("model.gguf", payload)], &mut inner_ctl),
                    Err(SlmError::Engine(EngineErrorCode::LockReentrant))
                );
                assert_eq!(
                    other_store.install_bytes(
                        &other_manifest,
                        &[("other.gguf", other_payload)],
                        &mut inner_ctl,
                    ),
                    Err(SlmError::Engine(EngineErrorCode::LockReentrant))
                );
                assert_eq!(
                    other_store.remove_package(&other_manifest, &mut inner_ctl),
                    Err(SlmError::Engine(EngineErrorCode::LockReentrant))
                );
                Ok(StubEngine(m.model_id().to_owned()))
            })
            .unwrap();
        assert!(other_dir.0.join("other.gguf").exists());

        // 2. Reentrada desde el callback `Progress::Loading` de `verify_and_load`.
        let loading_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let loading_seen_cb = Arc::clone(&loading_seen);
        let store_for_cb = store.clone();
        let manifest_for_cb = manifest.clone();
        let mut loading_ctl = Control::new(
            CancelToken::new(),
            Deadline::after(Duration::from_secs(5)),
            move |progress| {
                if progress == Progress::Loading {
                    loading_seen_cb.store(true, Ordering::SeqCst);
                    let mut cb_ctl = default_ctl();
                    assert_eq!(
                        store_for_cb.remove_package(&manifest_for_cb, &mut cb_ctl),
                        Err(SlmError::Engine(EngineErrorCode::LockReentrant))
                    );
                }
            },
        );
        store
            .verify_and_load(&manifest, &mut slot, &mut loading_ctl, |m, _| {
                Ok(StubEngine(m.model_id().to_owned()))
            })
            .unwrap();
        assert!(loading_seen.load(Ordering::SeqCst));

        // 3. Reentrada desde `ModelSlot::load` (callback `Loading` y `factory`).
        let verified = store.verify(&manifest, &mut ctl).unwrap();
        let slot_cb_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let slot_cb_seen_inner = Arc::clone(&slot_cb_seen);
        let store_for_slot_cb = store.clone();
        let manifest_for_slot_cb = manifest.clone();
        let mut slot_ctl = Control::new(
            CancelToken::new(),
            Deadline::after(Duration::from_secs(5)),
            move |progress| {
                if progress == Progress::Loading {
                    slot_cb_seen_inner.store(true, Ordering::SeqCst);
                    let mut cb_ctl = default_ctl();
                    assert_eq!(
                        store_for_slot_cb.remove_package(&manifest_for_slot_cb, &mut cb_ctl),
                        Err(SlmError::Engine(EngineErrorCode::LockReentrant))
                    );
                }
            },
        );
        slot.load(verified, &mut slot_ctl, |m, _| {
            let mut inner_ctl = default_ctl();
            assert_eq!(
                store.remove_package(&manifest, &mut inner_ctl),
                Err(SlmError::Engine(EngineErrorCode::LockReentrant))
            );
            Ok(StubEngine(m.model_id().to_owned()))
        })
        .unwrap();
        assert!(slot_cb_seen.load(Ordering::SeqCst));

        // 4. Escritura cruzada mientras el hilo ya retiene un bloqueo de escritura (`install_bytes`).
        let write_cb_seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let write_cb_seen_inner = Arc::clone(&write_cb_seen);
        let other_for_write_cb = other_store.clone();
        let other_manifest_for_write_cb = other_manifest.clone();
        let write_checks = Arc::new(AtomicU64::new(0));
        let write_checks_clock = Arc::clone(&write_checks);
        let write_start = Instant::now();
        let write_deadline = Deadline::with_clock(
            write_start,
            Duration::from_secs(5),
            Arc::new(move || {
                if write_checks_clock.fetch_add(1, Ordering::SeqCst) == 2 {
                    write_cb_seen_inner.store(true, Ordering::SeqCst);
                    let mut cb_ctl = default_ctl();
                    assert_eq!(
                        other_for_write_cb
                            .remove_package(&other_manifest_for_write_cb, &mut cb_ctl),
                        Err(SlmError::Engine(EngineErrorCode::LockReentrant))
                    );
                }
                write_start
            }),
        );
        let mut write_cb_ctl = Control::without_progress(CancelToken::new(), write_deadline);
        store
            .install_bytes(&manifest, &[("model.gguf", payload)], &mut write_cb_ctl)
            .unwrap();
        assert!(write_cb_seen.load(Ordering::SeqCst));

        // 5. Al soltar el bloqueo tras la carga, `remove_package` vuelve a funcionar normalmente.
        store.remove_package(&manifest, &mut ctl).unwrap();
        assert!(!dir.0.join("model.gguf").exists());
        other_store
            .remove_package(&other_manifest, &mut ctl)
            .unwrap();
        assert!(!other_dir.0.join("other.gguf").exists());
    }

    #[test]
    fn cross_store_remove_from_factories_fails_without_deadlock_using_barrier() {
        let dir_a = TempDir::new("cross_barrier_a");
        let dir_b = TempDir::new("cross_barrier_b");
        let payload_a = b"model-a-weights";
        let payload_b = b"model-b-weights";
        let manifest_a = single_manifest("model-a", "a.gguf", payload_a);
        let manifest_b = single_manifest("model-b", "b.gguf", payload_b);

        let store_a = ModelStore::new(&dir_a.0).unwrap();
        let store_b = ModelStore::new(&dir_b.0).unwrap();
        let mut ctl = default_ctl();
        store_a
            .install_bytes(&manifest_a, &[("a.gguf", payload_a)], &mut ctl)
            .unwrap();
        store_b
            .install_bytes(&manifest_b, &[("b.gguf", payload_b)], &mut ctl)
            .unwrap();

        let barrier = Arc::new(Barrier::new(2));

        let t1_store_a = store_a.clone();
        let t1_store_b = store_b.clone();
        let t1_manifest_a = manifest_a.clone();
        let t1_manifest_b = manifest_b.clone();
        let t1_barrier = Arc::clone(&barrier);
        let handle_a = thread::spawn(move || {
            let mut slot: ModelSlot<StubEngine> = ModelSlot::new();
            let mut load_ctl = default_ctl();
            t1_store_a
                .verify_and_load(&t1_manifest_a, &mut slot, &mut load_ctl, |m, _| {
                    t1_barrier.wait();
                    let mut inner_ctl = default_ctl();
                    let err = t1_store_b
                        .remove_package(&t1_manifest_b, &mut inner_ctl)
                        .unwrap_err();
                    assert_eq!(err, SlmError::Engine(EngineErrorCode::LockReentrant));
                    Ok(StubEngine(m.model_id().to_owned()))
                })
                .unwrap();
        });

        let t2_store_a = store_a.clone();
        let t2_store_b = store_b.clone();
        let t2_manifest_a = manifest_a.clone();
        let t2_manifest_b = manifest_b.clone();
        let t2_barrier = Arc::clone(&barrier);
        let handle_b = thread::spawn(move || {
            let mut slot: ModelSlot<StubEngine> = ModelSlot::new();
            let mut load_ctl = default_ctl();
            t2_store_b
                .verify_and_load(&t2_manifest_b, &mut slot, &mut load_ctl, |m, _| {
                    t2_barrier.wait();
                    let mut inner_ctl = default_ctl();
                    let err = t2_store_a
                        .remove_package(&t2_manifest_a, &mut inner_ctl)
                        .unwrap_err();
                    assert_eq!(err, SlmError::Engine(EngineErrorCode::LockReentrant));
                    Ok(StubEngine(m.model_id().to_owned()))
                })
                .unwrap();
        });

        handle_a.join().unwrap();
        handle_b.join().unwrap();

        assert!(store_a.verify(&manifest_a, &mut ctl).is_ok());
        assert!(store_b.verify(&manifest_b, &mut ctl).is_ok());
    }

    #[test]
    fn remove_package_waiting_on_lock_respects_cancel_and_timeout() {
        let dir = TempDir::new("remove_cancel_and_timeout");
        let payload = b"model-remove-lock-bytes";
        let manifest = single_manifest("granite", "model.gguf", payload);

        let store_a = ModelStore::new(&dir.0).unwrap();
        let store_b = ModelStore::new(&dir.0).unwrap();
        let mut ctl = default_ctl();
        store_a
            .install_bytes(&manifest, &[("model.gguf", payload)], &mut ctl)
            .unwrap();

        let (started_tx, started_rx) = mpsc::channel();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let store_reader = store_a.clone();
        let manifest_load = manifest.clone();
        let load_handle = thread::spawn(move || {
            let mut slot: ModelSlot<StubEngine> = ModelSlot::new();
            let mut load_ctl = default_ctl();
            store_reader
                .verify_and_load(&manifest_load, &mut slot, &mut load_ctl, |m, _| {
                    started_tx.send(()).unwrap();
                    proceed_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(StubEngine(m.model_id().to_owned()))
                })
                .unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let mut timeout_ctl = Control::without_progress(
            CancelToken::new(),
            Deadline::after(Duration::from_millis(25)),
        );
        assert_eq!(
            store_b.remove_package(&manifest, &mut timeout_ctl),
            Err(SlmError::Timeout)
        );

        let cancel = CancelToken::new();
        let cancel_on_wait = cancel.clone();
        let checks = Arc::new(AtomicU64::new(0));
        let checks_clock = Arc::clone(&checks);
        let start = Instant::now();
        let deadline = Deadline::with_clock(
            start,
            Duration::from_secs(5),
            Arc::new(move || {
                // Check 0: entrada de remove_package; Check 1: previo a try_write;
                // Check 2: tras el primer WouldBlock.
                if checks_clock.fetch_add(1, Ordering::SeqCst) == 2 {
                    cancel_on_wait.cancel();
                }
                start
            }),
        );
        let mut cancel_ctl = Control::without_progress(cancel, deadline);
        assert_eq!(
            store_b.remove_package(&manifest, &mut cancel_ctl),
            Err(SlmError::Cancelled)
        );

        proceed_tx.send(()).unwrap();
        load_handle.join().unwrap();

        assert!(dir.0.join("model.gguf").exists());
        store_b.remove_package(&manifest, &mut ctl).unwrap();
        assert!(!dir.0.join("model.gguf").exists());
    }
}
