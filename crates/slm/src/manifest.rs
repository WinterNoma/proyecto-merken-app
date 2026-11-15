//! Manifiesto de modelos y verificación de integridad en streaming (RNF-13).

use crate::{control::Control, error::SlmError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fmt::Write as _,
    fs::{self, File},
    io::{ErrorKind, Read},
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

pub(crate) const HASH_BUF_BYTES: usize = 64 * 1024;

/// Papel funcional de un modelo dentro de `merken-slm`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    /// Respaldo del intérprete de texto libre (RF-14).
    Interpreter,
    /// Explicador de evaluaciones y sugerencias curadas (RF-15).
    Explainer,
    /// Transcripción de voz local (RF-18).
    SpeechToText,
}

/// Especificación de un archivo individual incluido en un manifiesto.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFile {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

/// Paquete de archivos declarado en un manifiesto de modelo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelPackage {
    /// Modelo monolítico en un único archivo (p. ej. intérprete o transcripción).
    Single { file: ModelFile },
    /// Modelo base GGUF acompañado de un adaptador LoRA obligatorio (p. ej. explicador v5).
    BaseWithAdapter {
        base: ModelFile,
        adapter: ModelFile,
        lora_scale: f32,
        /// SHA-256 esperado del modelo base compatible con este adaptador, para fijar el par.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        compatible_base_sha256: Option<String>,
    },
}

/// Manifiesto verificable de un paquete de modelo local (p. ej. base GGUF + LoRA).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelManifest {
    pub model_id: String,
    pub role: ModelRole,
    pub version: String,
    pub package: ModelPackage,
}

/// Archivo ya verificado en disco mediante tamaño, fecha de modificación y SHA-256.
///
/// Tipo opaco: solo [`ModelManifest::verify`] puede construirlo tras comprobar rutas,
/// ausencia de enlaces simbólicos e integridad criptográfica.
///
/// ```compile_fail
/// use merken_slm::VerifiedFile;
/// use std::path::PathBuf;
///
/// let _fake = VerifiedFile {
///     name: "model.gguf".into(),
///     path: PathBuf::from("model.gguf"),
///     bytes: 0,
///     sha256: String::new(),
/// };
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedFile {
    name: String,
    path: PathBuf,
    bytes: u64,
    sha256: String,
    modified: SystemTime,
}

impl VerifiedFile {
    /// Nombre relativo declarado en el manifiesto.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Ruta canónica verificada dentro del directorio raíz de modelos.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Tamaño exacto verificado en bytes.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Resumen SHA-256 hexadecimal en minúsculas verificado en disco.
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Fecha de modificación observada durante la verificación.
    pub fn modified(&self) -> SystemTime {
        self.modified
    }
}

#[derive(Clone, Debug, PartialEq)]
enum VerifiedPackage {
    Single {
        file: VerifiedFile,
    },
    BaseWithAdapter {
        base: VerifiedFile,
        adapter: VerifiedFile,
        lora_scale: f32,
    },
}

/// Modelo verificado en disco y listo para cargarse en [`crate::ModelSlot`].
///
/// Tipo opaco: solo [`ModelManifest::verify`] o [`crate::ModelStore`] pueden construirlo.
///
/// # Mitigación frente a TOCTOU
///
/// Los modelos residen en el almacenamiento privado de la aplicación Android, donde
/// únicamente la propia aplicación tiene permisos de escritura; esa restricción es la
/// frontera frente a procesos externos. Dentro del proceso, [`crate::ModelStore`] coordina
/// todas las operaciones de instalación, actualización, verificación y carga mediante un
/// `RwLock` por directorio: la verificación y la carga en [`crate::ModelSlot`] mantienen el
/// bloqueo de lectura hasta que la fábrica del motor devuelve, bloqueando cualquier
/// instalación concurrente. Además, [`crate::ModelSlot`] vuelve a comprobar ausencia de
/// enlaces simbólicos, confinamiento canónico, tamaño y fecha de modificación justo antes
/// de entregar las rutas a la fábrica.
///
/// ```compile_fail
/// use merken_slm::{ModelRole, VerifiedModel};
/// use std::path::PathBuf;
///
/// let _fake = VerifiedModel {
///     model_id: "fake".into(),
///     role: ModelRole::Explainer,
///     version: "1.0".into(),
///     dir: PathBuf::from("."),
///     files: vec![],
/// };
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct VerifiedModel {
    model_id: String,
    role: ModelRole,
    version: String,
    dir: PathBuf,
    package: VerifiedPackage,
}

impl VerifiedModel {
    /// Identificador del modelo declarado en el manifiesto.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Papel funcional del modelo verificado.
    pub fn role(&self) -> ModelRole {
        self.role
    }

    /// Versión del modelo declarada en el manifiesto.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Directorio raíz canónico donde se verificó el paquete.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Archivo principal del paquete (único archivo en `Single`, o modelo base en `BaseWithAdapter`).
    pub fn base_file(&self) -> &VerifiedFile {
        match &self.package {
            VerifiedPackage::Single { file } => file,
            VerifiedPackage::BaseWithAdapter { base, .. } => base,
        }
    }

    /// Archivo del adaptador LoRA cuando el paquete es `BaseWithAdapter`.
    pub fn adapter_file(&self) -> Option<&VerifiedFile> {
        match &self.package {
            VerifiedPackage::Single { .. } => None,
            VerifiedPackage::BaseWithAdapter { adapter, .. } => Some(adapter),
        }
    }

    /// Escala del adaptador LoRA cuando el paquete es `BaseWithAdapter`.
    pub fn lora_scale(&self) -> Option<f32> {
        match &self.package {
            VerifiedPackage::Single { .. } => None,
            VerifiedPackage::BaseWithAdapter { lora_scale, .. } => Some(*lora_scale),
        }
    }

    /// Lista de archivos verificados del paquete en orden determinista.
    pub fn files(&self) -> Vec<&VerifiedFile> {
        match &self.package {
            VerifiedPackage::Single { file } => vec![file],
            VerifiedPackage::BaseWithAdapter { base, adapter, .. } => vec![base, adapter],
        }
    }

    /// Busca la ruta en disco de un archivo verificado por su nombre en el manifiesto.
    pub fn file_path(&self, name: &str) -> Option<&Path> {
        self.files()
            .into_iter()
            .find(|f| f.name() == name)
            .map(VerifiedFile::path)
    }

    /// Revalida antes de entregar las rutas a la fábrica de `ModelSlot` que los archivos
    /// siguen sin enlaces simbólicos, bajo la raíz canónica y con el mismo tamaño y fecha
    /// de modificación observados al verificar.
    ///
    /// Mitigación principal TOCTOU: almacenamiento privado de la app Android + bloqueo de
    /// lectura mantenido por [`crate::ModelStore`] durante verificación y carga.
    pub(crate) fn revalidate_before_load(&self) -> Result<(), SlmError> {
        let files = self.files();
        let mut present_count = 0usize;
        let mut missing_count = 0usize;

        for verified in &files {
            match inspect_candidate(&self.dir, verified.name())? {
                CandidateState::Missing => {
                    missing_count += 1;
                }
                CandidateState::Present { path, meta } => {
                    present_count += 1;
                    if path != verified.path() {
                        return Err(SlmError::HashMismatch {
                            file: verified.name().to_owned(),
                        });
                    }
                    let modified = meta.modified()?;
                    if meta.len() != verified.bytes() || modified != verified.modified() {
                        return Err(SlmError::HashMismatch {
                            file: verified.name().to_owned(),
                        });
                    }
                }
            }
        }

        if missing_count > 0 {
            return Err(if present_count == 0 {
                SlmError::ModelMissing
            } else {
                SlmError::IncompletePair
            });
        }

        Ok(())
    }
}

pub(crate) fn safe_relative_components(rel: &str) -> Result<Vec<OsString>, SlmError> {
    if rel.trim().is_empty() {
        return Err(SlmError::Unsupported(
            "Model file name cannot be empty".to_owned(),
        ));
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return Err(SlmError::Unsupported(
            "Model file path must be relative".to_owned(),
        ));
    }
    let mut parts = Vec::new();
    for comp in path.components() {
        match comp {
            Component::Normal(part) => parts.push(part.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SlmError::Unsupported(
                    "Model file path contains disallowed traversal components".to_owned(),
                ));
            }
        }
    }
    if parts.is_empty() {
        return Err(SlmError::Unsupported(
            "Model file path is invalid".to_owned(),
        ));
    }
    Ok(parts)
}

enum CandidateState {
    Present { path: PathBuf, meta: fs::Metadata },
    Missing,
}

fn inspect_candidate(canonical_root: &Path, rel: &str) -> Result<CandidateState, SlmError> {
    let parts = safe_relative_components(rel)?;
    let mut cur = canonical_root.to_path_buf();

    for (idx, part) in parts.iter().enumerate() {
        let is_last = idx + 1 == parts.len();
        cur.push(part);

        match fs::symlink_metadata(&cur) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(SlmError::Unsupported(
                        "Symlinks are not allowed in model directory".to_owned(),
                    ));
                }
                if is_last {
                    if !meta.is_file() {
                        return Err(SlmError::Io("Model entry is not a regular file".to_owned()));
                    }
                    let canonical_file = fs::canonicalize(&cur)?;
                    if !canonical_file.starts_with(canonical_root) {
                        return Err(SlmError::Unsupported(
                            "Resolved model path escapes root directory".to_owned(),
                        ));
                    }
                    return Ok(CandidateState::Present {
                        path: canonical_file,
                        meta,
                    });
                } else if !meta.is_dir() {
                    return Err(SlmError::Io(
                        "Model parent path is not a directory".to_owned(),
                    ));
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Ok(CandidateState::Missing);
            }
            Err(err) => return Err(SlmError::from(err)),
        }
    }

    Err(SlmError::Unsupported(
        "Model file path is invalid".to_owned(),
    ))
}

pub(crate) fn is_valid_sha256_hex(hex: &str) -> bool {
    hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

fn compute_sha256_streaming(
    path: &Path,
    expected_bytes: u64,
    display_name: &str,
    ctl: &mut Control<'_>,
) -> Result<(String, SystemTime), SlmError> {
    ctl.check()?;

    let mut file = File::open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(SlmError::Io("Model entry is not a regular file".to_owned()));
    }
    if meta.len() != expected_bytes {
        return Err(SlmError::HashMismatch {
            file: display_name.to_owned(),
        });
    }
    let modified = meta.modified()?;

    let mut hasher = Sha256::new();
    let mut buf = [0_u8; HASH_BUF_BYTES];
    let mut total_read: u64 = 0;

    loop {
        ctl.check()?;
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let chunk = buf
            .get(..n)
            .ok_or_else(|| SlmError::Io("Read buffer length exceeded".to_owned()))?;
        hasher.update(chunk);
        total_read = total_read
            .checked_add(
                u64::try_from(n)
                    .map_err(|_| SlmError::Io("Byte count conversion failed".to_owned()))?,
            )
            .ok_or_else(|| SlmError::Io("File byte count overflow".to_owned()))?;
        if total_read > expected_bytes {
            return Err(SlmError::HashMismatch {
                file: display_name.to_owned(),
            });
        }
        ctl.check()?;
    }

    if total_read != expected_bytes {
        return Err(SlmError::HashMismatch {
            file: display_name.to_owned(),
        });
    }

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(hex, "{byte:02x}")
            .map_err(|_| SlmError::Io("SHA-256 formatting failed".to_owned()))?;
    }
    Ok((hex, modified))
}

fn verify_one_file(
    spec: &ModelFile,
    canonical_file: PathBuf,
    ctl: &mut Control<'_>,
) -> Result<VerifiedFile, SlmError> {
    ctl.check()?;
    if !is_valid_sha256_hex(&spec.sha256) {
        return Err(SlmError::HashMismatch {
            file: spec.name.clone(),
        });
    }
    let (actual_hex, modified) =
        compute_sha256_streaming(&canonical_file, spec.bytes, &spec.name, ctl)?;
    if !actual_hex.eq_ignore_ascii_case(&spec.sha256) {
        return Err(SlmError::HashMismatch {
            file: spec.name.clone(),
        });
    }
    Ok(VerifiedFile {
        name: spec.name.clone(),
        path: canonical_file,
        bytes: spec.bytes,
        sha256: actual_hex,
        modified,
    })
}

impl ModelManifest {
    /// Deserializa un manifiesto desde JSON y valida su contrato estructural.
    pub fn from_json(json: &str) -> Result<Self, SlmError> {
        let manifest: Self = serde_json::from_str(json)
            .map_err(|_| SlmError::Unsupported("Invalid model manifest JSON".to_owned()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Comprueba las invariantes estructurales del manifiesto (rol, paquete, rutas y par LoRA).
    pub fn validate(&self) -> Result<(), SlmError> {
        if self.model_id.trim().is_empty() || self.version.trim().is_empty() {
            return Err(SlmError::Unsupported(
                "Manifest model_id and version must not be empty".to_owned(),
            ));
        }

        if self.role == ModelRole::Explainer
            && !matches!(self.package, ModelPackage::BaseWithAdapter { .. })
        {
            return Err(SlmError::Unsupported(
                "Explainer role requires a base_with_adapter package".to_owned(),
            ));
        }

        match &self.package {
            ModelPackage::Single { file } => {
                safe_relative_components(&file.name)?;
            }
            ModelPackage::BaseWithAdapter {
                base,
                adapter,
                lora_scale,
                compatible_base_sha256,
            } => {
                let base_parts = safe_relative_components(&base.name)?;
                let adapter_parts = safe_relative_components(&adapter.name)?;
                if base_parts == adapter_parts {
                    return Err(SlmError::Unsupported(
                        "Base and adapter must reference distinct files".to_owned(),
                    ));
                }
                if !lora_scale.is_finite() || *lora_scale <= 0.0 {
                    return Err(SlmError::Unsupported(
                        "LoRA scale must be finite and positive".to_owned(),
                    ));
                }
                if let Some(expected_base) = compatible_base_sha256
                    && (!is_valid_sha256_hex(expected_base)
                        || !expected_base.eq_ignore_ascii_case(&base.sha256))
                {
                    return Err(SlmError::Unsupported(
                        "Adapter compatible_base_sha256 does not match base sha256".to_owned(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Comprueba existencia, ausencia de enlaces simbólicos, confinamiento canónico, tamaño
    /// y SHA-256 en streaming de los archivos en `dir`, respetando cancelación y plazo en `ctl`.
    ///
    /// Adquiere el bloqueo de lectura compartido del directorio para evitar instalaciones
    /// concurrentes durante la verificación. Para mantener el mismo bloqueo de lectura de
    /// forma continua hasta terminar la carga del motor, usa [`crate::ModelStore::verify_and_load`].
    ///
    /// Si faltan todos los archivos devuelve [`SlmError::ModelMissing`].
    /// Si falta parte de un paquete `BaseWithAdapter` devuelve [`SlmError::IncompletePair`].
    pub fn verify(&self, dir: &Path, ctl: &mut Control<'_>) -> Result<VerifiedModel, SlmError> {
        crate::store::with_dir_read_lock(dir, ctl, |ctl| self.verify_unlocked(dir, ctl))
    }

    pub(crate) fn verify_unlocked(
        &self,
        dir: &Path,
        ctl: &mut Control<'_>,
    ) -> Result<VerifiedModel, SlmError> {
        ctl.check()?;
        self.validate()?;

        let canonical_root = match fs::canonicalize(dir) {
            Ok(p) => p,
            Err(err) if err.kind() == ErrorKind::NotFound => return Err(SlmError::ModelMissing),
            Err(err) => return Err(SlmError::from(err)),
        };
        let root_meta = fs::symlink_metadata(&canonical_root)?;
        if !root_meta.is_dir() {
            return Err(SlmError::Unsupported(
                "Model root path must be a directory".to_owned(),
            ));
        }

        let package = match &self.package {
            ModelPackage::Single { file } => {
                let CandidateState::Present { path, .. } =
                    inspect_candidate(&canonical_root, &file.name)?
                else {
                    return Err(SlmError::ModelMissing);
                };
                let verified_file = verify_one_file(file, path, ctl)?;
                VerifiedPackage::Single {
                    file: verified_file,
                }
            }
            ModelPackage::BaseWithAdapter {
                base,
                adapter,
                lora_scale,
                ..
            } => {
                let base_state = inspect_candidate(&canonical_root, &base.name)?;
                let adapter_state = inspect_candidate(&canonical_root, &adapter.name)?;
                match (base_state, adapter_state) {
                    (CandidateState::Missing, CandidateState::Missing) => {
                        return Err(SlmError::ModelMissing);
                    }
                    (CandidateState::Missing, CandidateState::Present { .. })
                    | (CandidateState::Present { .. }, CandidateState::Missing) => {
                        return Err(SlmError::IncompletePair);
                    }
                    (
                        CandidateState::Present {
                            path: base_path, ..
                        },
                        CandidateState::Present {
                            path: adapter_path, ..
                        },
                    ) => {
                        let verified_base = verify_one_file(base, base_path, ctl)?;
                        let verified_adapter = verify_one_file(adapter, adapter_path, ctl)?;
                        VerifiedPackage::BaseWithAdapter {
                            base: verified_base,
                            adapter: verified_adapter,
                            lora_scale: *lora_scale,
                        }
                    }
                }
            }
        };

        ctl.check()?;
        Ok(VerifiedModel {
            model_id: self.model_id.clone(),
            role: self.role,
            version: self.version.clone(),
            dir: canonical_root,
            package,
        })
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
    use serde_json::json;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    };

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let n = TEMP_SEQ.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "merken_slm_manifest_{label}_{}_{}",
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
        Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)))
    }

    #[test]
    fn verify_succeeds_for_valid_base_and_lora_pair() {
        let dir = TempDir::new("pair_ok");
        let base_bytes = b"simulated-gguf-base-weights";
        let lora_bytes = b"simulated-lora-adapter";
        fs::write(dir.0.join("base.gguf"), base_bytes).unwrap();
        fs::write(dir.0.join("lora.gguf"), lora_bytes).unwrap();

        let base_sha = sha256_hex(base_bytes);
        let manifest = ModelManifest {
            model_id: "space-health-v5".into(),
            role: ModelRole::Explainer,
            version: "5.0.0".into(),
            package: ModelPackage::BaseWithAdapter {
                base: ModelFile {
                    name: "base.gguf".into(),
                    bytes: base_bytes.len() as u64,
                    sha256: base_sha.to_uppercase(),
                },
                adapter: ModelFile {
                    name: "lora.gguf".into(),
                    bytes: lora_bytes.len() as u64,
                    sha256: sha256_hex(lora_bytes),
                },
                lora_scale: 0.75,
                compatible_base_sha256: Some(base_sha),
            },
        };

        let mut ctl = default_ctl();
        let verified = manifest.verify(&dir.0, &mut ctl).unwrap();
        assert_eq!(verified.model_id(), "space-health-v5");
        assert_eq!(verified.role(), ModelRole::Explainer);
        assert_eq!(verified.version(), "5.0.0");
        assert_eq!(verified.files().len(), 2);
        assert_eq!(verified.lora_scale(), Some(0.75));
        assert_eq!(
            verified.file_path("lora.gguf"),
            Some(fs::canonicalize(dir.0.join("lora.gguf")).unwrap().as_path())
        );
        assert_eq!(
            verified.adapter_file().map(VerifiedFile::name),
            Some("lora.gguf")
        );
    }

    #[test]
    fn verify_rejects_missing_directory_or_files_and_incomplete_pair() {
        let dir = TempDir::new("pair_incomplete");
        let base_bytes = b"base-only";
        let manifest = ModelManifest {
            model_id: "space-health-v5".into(),
            role: ModelRole::Explainer,
            version: "5.0.0".into(),
            package: ModelPackage::BaseWithAdapter {
                base: ModelFile {
                    name: "base.gguf".into(),
                    bytes: base_bytes.len() as u64,
                    sha256: sha256_hex(base_bytes),
                },
                adapter: ModelFile {
                    name: "lora.gguf".into(),
                    bytes: 4,
                    sha256: sha256_hex(b"lora"),
                },
                lora_scale: 1.0,
                compatible_base_sha256: None,
            },
        };

        let mut ctl = default_ctl();
        assert_eq!(
            manifest.verify(&dir.0, &mut ctl),
            Err(SlmError::ModelMissing)
        );
        assert_eq!(
            manifest.verify(&dir.0.join("missing_subdir"), &mut ctl),
            Err(SlmError::ModelMissing)
        );

        fs::write(dir.0.join("base.gguf"), base_bytes).unwrap();
        assert_eq!(
            manifest.verify(&dir.0, &mut ctl),
            Err(SlmError::IncompletePair)
        );
    }

    #[test]
    fn explainer_without_adapter_is_rejected_on_parse_and_verify() {
        let dir = TempDir::new("explainer_single");
        let base_bytes = b"only-base";
        fs::write(dir.0.join("base.gguf"), base_bytes).unwrap();

        let raw_json = json!({
            "model_id": "space-health-v5",
            "role": "explainer",
            "version": "5.0.0",
            "package": {
                "kind": "single",
                "file": {
                    "name": "base.gguf",
                    "bytes": base_bytes.len(),
                    "sha256": sha256_hex(base_bytes),
                }
            }
        })
        .to_string();

        assert!(matches!(
            ModelManifest::from_json(&raw_json),
            Err(SlmError::Unsupported(_))
        ));

        let direct = ModelManifest {
            model_id: "space-health-v5".into(),
            role: ModelRole::Explainer,
            version: "5.0.0".into(),
            package: ModelPackage::Single {
                file: ModelFile {
                    name: "base.gguf".into(),
                    bytes: base_bytes.len() as u64,
                    sha256: sha256_hex(base_bytes),
                },
            },
        };
        let mut ctl = default_ctl();
        assert!(matches!(
            direct.verify(&dir.0, &mut ctl),
            Err(SlmError::Unsupported(_))
        ));
    }

    #[test]
    fn incompatible_base_sha256_on_adapter_is_rejected() {
        let json_str = json!({
            "model_id": "space-health-v5",
            "role": "explainer",
            "version": "5.0.0",
            "package": {
                "kind": "base_with_adapter",
                "base": {
                    "name": "base.gguf",
                    "bytes": 4,
                    "sha256": sha256_hex(b"base"),
                },
                "adapter": {
                    "name": "lora.gguf",
                    "bytes": 4,
                    "sha256": sha256_hex(b"lora"),
                },
                "lora_scale": 1.0,
                "compatible_base_sha256": sha256_hex(b"other-base"),
            }
        })
        .to_string();

        assert!(matches!(
            ModelManifest::from_json(&json_str),
            Err(SlmError::Unsupported(_))
        ));
    }

    #[test]
    fn manifest_from_json_does_not_leak_serde_errors() {
        let secret_payload = r#"{"model_id": "[I have HIV]", "role": "not_a_real_role_12345"}"#;
        let err = ModelManifest::from_json(secret_payload).unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains("HIV"));
        assert!(!msg.contains("not_a_real_role_12345"));
    }

    #[test]
    fn verify_rejects_hash_or_size_mismatch_and_traversal() {
        let dir = TempDir::new("hash_bad");
        let data = b"corrupted-bytes";
        fs::write(dir.0.join("model.gguf"), data).unwrap();

        let bad_hash = ModelManifest {
            model_id: "granite-350m".into(),
            role: ModelRole::Interpreter,
            version: "4.0".into(),
            package: ModelPackage::Single {
                file: ModelFile {
                    name: "model.gguf".into(),
                    bytes: data.len() as u64,
                    sha256: sha256_hex(b"expected-bytes"),
                },
            },
        };
        let mut ctl = default_ctl();
        assert_eq!(
            bad_hash.verify(&dir.0, &mut ctl),
            Err(SlmError::HashMismatch {
                file: "model.gguf".into()
            })
        );

        let bad_size = ModelManifest {
            package: ModelPackage::Single {
                file: ModelFile {
                    name: "model.gguf".into(),
                    bytes: (data.len() as u64) + 1,
                    sha256: sha256_hex(data),
                },
            },
            ..bad_hash.clone()
        };
        assert_eq!(
            bad_size.verify(&dir.0, &mut ctl),
            Err(SlmError::HashMismatch {
                file: "model.gguf".into()
            })
        );

        let traversal = ModelManifest {
            package: ModelPackage::Single {
                file: ModelFile {
                    name: "../secret.gguf".into(),
                    bytes: 1,
                    sha256: sha256_hex(b"x"),
                },
            },
            ..bad_hash
        };
        assert!(matches!(
            traversal.verify(&dir.0, &mut ctl),
            Err(SlmError::Unsupported(_))
        ));
    }

    fn create_file_symlink(link: &Path, target: &Path) -> Result<(), String> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).map_err(|e| e.to_string())
        }
        #[cfg(windows)]
        {
            match std::os::windows::fs::symlink_file(target, link) {
                Ok(()) => Ok(()),
                Err(err)
                    if err.kind() == ErrorKind::PermissionDenied
                        || err.raw_os_error() == Some(1314) =>
                {
                    Err("windows symlink privilege not held".to_owned())
                }
                Err(err) => Err(err.to_string()),
            }
        }
    }

    fn create_dir_symlink(link: &Path, target: &Path) -> Result<(), String> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).map_err(|e| e.to_string())
        }
        #[cfg(windows)]
        {
            match std::os::windows::fs::symlink_dir(target, link) {
                Ok(()) => Ok(()),
                Err(err)
                    if err.kind() == ErrorKind::PermissionDenied
                        || err.raw_os_error() == Some(1314) =>
                {
                    Err("windows symlink privilege not held".to_owned())
                }
                Err(err) => Err(err.to_string()),
            }
        }
    }

    #[test]
    fn verify_rejects_file_and_intermediate_dir_symlinks_pointing_outside_root() {
        let root = TempDir::new("symlink_root");
        let outside = TempDir::new("symlink_outside");
        let payload = b"outside-weights-with-valid-hash";
        let outside_file = outside.0.join("outside.gguf");
        fs::write(&outside_file, payload).unwrap();

        let link_file = root.0.join("base.gguf");
        if let Err(reason) = create_file_symlink(&link_file, &outside_file) {
            eprintln!("skipping file symlink regression test: {reason}");
            return;
        }

        let manifest_file_link = ModelManifest {
            model_id: "granite-350m".into(),
            role: ModelRole::Interpreter,
            version: "4.0".into(),
            package: ModelPackage::Single {
                file: ModelFile {
                    name: "base.gguf".into(),
                    bytes: payload.len() as u64,
                    sha256: sha256_hex(payload),
                },
            },
        };
        let mut ctl = default_ctl();
        assert!(matches!(
            manifest_file_link.verify(&root.0, &mut ctl),
            Err(SlmError::Unsupported(_))
        ));

        let dir_link = root.0.join("nested");
        if let Err(reason) = create_dir_symlink(&dir_link, &outside.0) {
            eprintln!("skipping dir symlink regression test: {reason}");
            return;
        }
        let manifest_dir_link = ModelManifest {
            model_id: "granite-350m".into(),
            role: ModelRole::Interpreter,
            version: "4.0".into(),
            package: ModelPackage::Single {
                file: ModelFile {
                    name: "nested/outside.gguf".into(),
                    bytes: payload.len() as u64,
                    sha256: sha256_hex(payload),
                },
            },
        };
        assert!(matches!(
            manifest_dir_link.verify(&root.0, &mut ctl),
            Err(SlmError::Unsupported(_))
        ));
    }

    #[test]
    fn verify_checks_cancellation_and_deadline_between_hash_blocks() {
        let dir = TempDir::new("hash_cancel");
        // > 2 bloques de 64 KiB para garantizar varias iteraciones del bucle de hash.
        let data = vec![0xAB_u8; 160 * 1024];
        fs::write(dir.0.join("large.gguf"), &data).unwrap();

        let manifest = ModelManifest {
            model_id: "granite-350m".into(),
            role: ModelRole::Interpreter,
            version: "4.0".into(),
            package: ModelPackage::Single {
                file: ModelFile {
                    name: "large.gguf".into(),
                    bytes: data.len() as u64,
                    sha256: sha256_hex(&data),
                },
            },
        };

        let cancel = CancelToken::new();
        cancel.cancel();
        let mut ctl_cancel =
            Control::without_progress(cancel, Deadline::after(Duration::from_secs(5)));
        assert_eq!(
            manifest.verify(&dir.0, &mut ctl_cancel),
            Err(SlmError::Cancelled)
        );

        // Reloj determinista que vence tras las primeras comprobaciones (entre bloques del hash).
        let start = Instant::now();
        let ticks = Arc::new(AtomicU64::new(0));
        let ticks_clock = Arc::clone(&ticks);
        let deadline = Deadline::with_clock(
            start,
            Duration::from_secs(5),
            Arc::new(move || {
                let n = ticks_clock.fetch_add(1, Ordering::SeqCst);
                if n >= 3 {
                    start + Duration::from_secs(10)
                } else {
                    start
                }
            }),
        );
        let mut ctl_timeout = Control::without_progress(CancelToken::new(), deadline);
        assert_eq!(
            manifest.verify(&dir.0, &mut ctl_timeout),
            Err(SlmError::Timeout)
        );
        assert!(ticks.load(Ordering::SeqCst) >= 4);
    }
}
