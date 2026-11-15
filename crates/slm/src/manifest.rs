//! Manifiesto de modelos y verificación de integridad en streaming (RNF-13).

use crate::error::SlmError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt::Write as _,
    fs::{self, File},
    io::{ErrorKind, Read},
    path::{Component, Path, PathBuf},
};

const HASH_BUF_BYTES: usize = 64 * 1024;

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

/// Manifiesto verificable de un paquete de modelo local (p. ej. base GGUF + LoRA).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelManifest {
    pub model_id: String,
    pub role: ModelRole,
    pub version: String,
    pub files: Vec<ModelFile>,
    /// Si es `true`, exige que todos los archivos existan (par base + LoRA indivisible).
    pub requires_all: bool,
}

/// Archivo ya verificado en disco mediante tamaño y SHA-256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedFile {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
}

/// Modelo verificado en disco y listo para cargarse en [`crate::ModelSlot`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedModel {
    pub model_id: String,
    pub role: ModelRole,
    pub version: String,
    pub dir: PathBuf,
    pub files: Vec<VerifiedFile>,
}

impl VerifiedModel {
    /// Busca la ruta en disco de un archivo verificado por su nombre en el manifiesto.
    pub fn file_path(&self, name: &str) -> Option<&Path> {
        self.files
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.path.as_path())
    }
}

fn safe_relative_path(dir: &Path, rel: &str) -> Result<PathBuf, SlmError> {
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
    let mut joined = dir.to_path_buf();
    let mut has_normal = false;
    for comp in path.components() {
        match comp {
            Component::Normal(part) => {
                has_normal = true;
                joined.push(part);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SlmError::Unsupported(
                    "Model file path contains disallowed traversal components".to_owned(),
                ));
            }
        }
    }
    if !has_normal {
        return Err(SlmError::Unsupported(
            "Model file path is invalid".to_owned(),
        ));
    }
    Ok(joined)
}

fn is_valid_sha256_hex(hex: &str) -> bool {
    hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

fn compute_sha256_streaming(
    path: &Path,
    expected_bytes: u64,
    display_name: &str,
) -> Result<String, SlmError> {
    let mut file = File::open(path)?;
    let meta = file.metadata()?;
    if meta.len() != expected_bytes {
        return Err(SlmError::HashMismatch {
            file: display_name.to_owned(),
        });
    }

    let mut hasher = Sha256::new();
    let mut buf = [0_u8; HASH_BUF_BYTES];
    let mut total_read: u64 = 0;

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let chunk = buf
            .get(..n)
            .ok_or_else(|| SlmError::Io("Read buffer length exceeded".to_owned()))?;
        hasher.update(chunk);
        total_read = total_read
            .checked_add(u64::try_from(n).map_err(|e| SlmError::Io(e.to_string()))?)
            .ok_or_else(|| SlmError::Io("File byte count overflow".to_owned()))?;
        if total_read > expected_bytes {
            return Err(SlmError::HashMismatch {
                file: display_name.to_owned(),
            });
        }
    }

    if total_read != expected_bytes {
        return Err(SlmError::HashMismatch {
            file: display_name.to_owned(),
        });
    }

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(hex, "{byte:02x}").map_err(|e| SlmError::Io(e.to_string()))?;
    }
    Ok(hex)
}

impl ModelManifest {
    /// Deserializa un manifiesto desde JSON.
    pub fn from_json(json: &str) -> Result<Self, SlmError> {
        serde_json::from_str(json)
            .map_err(|e| SlmError::Unsupported(format!("Invalid model manifest JSON: {e}")))
    }

    /// Comprueba existencia, tamaño y SHA-256 en streaming de los archivos en `dir`.
    ///
    /// Si faltan todos los archivos devuelve [`SlmError::ModelMissing`].
    /// Si `requires_all` es `true` y falta parte del conjunto (p. ej. base sin LoRA o LoRA sin
    /// base), devuelve [`SlmError::IncompletePair`].
    pub fn verify(&self, dir: &Path) -> Result<VerifiedModel, SlmError> {
        if self.files.is_empty() {
            return Err(SlmError::ModelMissing);
        }

        let mut candidates = Vec::with_capacity(self.files.len());
        let mut present_count: usize = 0;
        let mut missing_count: usize = 0;

        for spec in &self.files {
            let full_path = safe_relative_path(dir, &spec.name)?;
            match fs::metadata(&full_path) {
                Ok(meta) => {
                    if !meta.is_file() {
                        return Err(SlmError::Io(format!(
                            "Model entry '{}' is not a regular file",
                            spec.name
                        )));
                    }
                    present_count += 1;
                    candidates.push((spec, full_path, true));
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    missing_count += 1;
                    candidates.push((spec, full_path, false));
                }
                Err(err) => return Err(SlmError::Io(err.to_string())),
            }
        }

        if present_count == 0 {
            return Err(SlmError::ModelMissing);
        }

        if self.requires_all && missing_count > 0 {
            return Err(SlmError::IncompletePair);
        }

        let mut verified = Vec::with_capacity(present_count);
        for (spec, full_path, exists) in candidates {
            if !exists {
                continue;
            }
            if !is_valid_sha256_hex(&spec.sha256) {
                return Err(SlmError::HashMismatch {
                    file: spec.name.clone(),
                });
            }
            let actual_hex = compute_sha256_streaming(&full_path, spec.bytes, &spec.name)?;
            if !actual_hex.eq_ignore_ascii_case(&spec.sha256) {
                return Err(SlmError::HashMismatch {
                    file: spec.name.clone(),
                });
            }
            verified.push(VerifiedFile {
                name: spec.name.clone(),
                path: full_path,
                bytes: spec.bytes,
                sha256: actual_hex,
            });
        }

        Ok(VerifiedModel {
            model_id: self.model_id.clone(),
            role: self.role,
            version: self.version.clone(),
            dir: dir.to_path_buf(),
            files: verified,
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
    use std::sync::atomic::{AtomicU64, Ordering};

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

    #[test]
    fn verify_succeeds_for_valid_base_and_lora_pair() {
        let dir = TempDir::new("pair_ok");
        let base_bytes = b"simulated-gguf-base-weights";
        let lora_bytes = b"simulated-lora-adapter";
        fs::write(dir.0.join("base.gguf"), base_bytes).unwrap();
        fs::write(dir.0.join("lora.gguf"), lora_bytes).unwrap();

        let manifest = ModelManifest {
            model_id: "space-health-v5".into(),
            role: ModelRole::Explainer,
            version: "5.0.0".into(),
            requires_all: true,
            files: vec![
                ModelFile {
                    name: "base.gguf".into(),
                    bytes: base_bytes.len() as u64,
                    sha256: sha256_hex(base_bytes).to_uppercase(),
                },
                ModelFile {
                    name: "lora.gguf".into(),
                    bytes: lora_bytes.len() as u64,
                    sha256: sha256_hex(lora_bytes),
                },
            ],
        };

        let verified = manifest.verify(&dir.0).unwrap();
        assert_eq!(verified.model_id, "space-health-v5");
        assert_eq!(verified.files.len(), 2);
        assert_eq!(
            verified.file_path("lora.gguf"),
            Some(dir.0.join("lora.gguf").as_path())
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
            requires_all: true,
            files: vec![
                ModelFile {
                    name: "base.gguf".into(),
                    bytes: base_bytes.len() as u64,
                    sha256: sha256_hex(base_bytes),
                },
                ModelFile {
                    name: "lora.gguf".into(),
                    bytes: 4,
                    sha256: sha256_hex(b"lora"),
                },
            ],
        };

        assert_eq!(manifest.verify(&dir.0), Err(SlmError::ModelMissing));

        fs::write(dir.0.join("base.gguf"), base_bytes).unwrap();
        assert_eq!(manifest.verify(&dir.0), Err(SlmError::IncompletePair));
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
            requires_all: true,
            files: vec![ModelFile {
                name: "model.gguf".into(),
                bytes: data.len() as u64,
                sha256: sha256_hex(b"expected-bytes"),
            }],
        };
        assert_eq!(
            bad_hash.verify(&dir.0),
            Err(SlmError::HashMismatch {
                file: "model.gguf".into()
            })
        );

        let bad_size = ModelManifest {
            files: vec![ModelFile {
                name: "model.gguf".into(),
                bytes: (data.len() as u64) + 1,
                sha256: sha256_hex(data),
            }],
            ..bad_hash.clone()
        };
        assert_eq!(
            bad_size.verify(&dir.0),
            Err(SlmError::HashMismatch {
                file: "model.gguf".into()
            })
        );

        let traversal = ModelManifest {
            files: vec![ModelFile {
                name: "../secret.gguf".into(),
                bytes: 1,
                sha256: sha256_hex(b"x"),
            }],
            ..bad_hash
        };
        assert!(matches!(
            traversal.verify(&dir.0),
            Err(SlmError::Unsupported(_))
        ));
    }
}
