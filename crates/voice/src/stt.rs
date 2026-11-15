//! Offline speech recognition. Transcribed text is returned, never saved.
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::{VoiceError, check_path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SttModel {
    Whisper,
    Canary,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SttLanguage {
    Es,
    En,
    #[default]
    Auto,
}

impl SttLanguage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Es => "es",
            Self::En => "en",
            Self::Auto => "auto",
        }
    }
}

impl FromStr for SttLanguage {
    type Err = VoiceError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "es" => Ok(Self::Es),
            "en" => Ok(Self::En),
            "auto" | "" => Ok(Self::Auto),
            _ => Err(VoiceError::LoadFailed),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SttConfig {
    pub threads: u8,
    pub language: SttLanguage,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            threads: 2,
            language: SttLanguage::Auto,
        }
    }
}

impl SttConfig {
    pub fn validate(&self) -> Result<(), VoiceError> {
        if !(1..=4).contains(&self.threads) {
            return Err(VoiceError::LoadFailed);
        }
        Ok(())
    }
}

/// Model files are checked on discovery and again before creating a recognizer.
#[derive(Clone, Debug)]
pub struct SttPaths {
    model: SttModel,
    encoder: PathBuf,
    decoder: PathBuf,
    tokens: PathBuf,
}

impl SttPaths {
    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Self, VoiceError> {
        let dir = dir.as_ref();
        checked_path(dir, true)?;
        let canary = Self {
            model: SttModel::Canary,
            encoder: dir.join("encoder.int8.onnx"),
            decoder: dir.join("decoder.int8.onnx"),
            tokens: dir.join("tokens.txt"),
        };
        let canary_present = [&canary.encoder, &canary.decoder, &canary.tokens]
            .iter()
            .all(|p| std::fs::symlink_metadata(p).is_ok());
        let mut whisper = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(|_| VoiceError::ModelMissing)? {
            let entry = entry.map_err(|_| VoiceError::ModelMissing)?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(prefix) = name.strip_suffix("-tokens.txt") else {
                continue;
            };
            if prefix.is_empty() {
                continue;
            }
            let tokens = entry.path();
            let int8_encoder = dir.join(format!("{prefix}-encoder.int8.onnx"));
            let int8_decoder = dir.join(format!("{prefix}-decoder.int8.onnx"));
            let encoder = dir.join(format!("{prefix}-encoder.onnx"));
            let decoder = dir.join(format!("{prefix}-decoder.onnx"));
            let exists = |p: &Path| std::fs::symlink_metadata(p).is_ok();
            let pair = if exists(&int8_encoder) && exists(&int8_decoder) {
                Some((int8_encoder, int8_decoder))
            } else if exists(&encoder) && exists(&decoder) {
                Some((encoder, decoder))
            } else {
                None
            };
            if let Some((encoder, decoder)) = pair {
                whisper.push(Self {
                    model: SttModel::Whisper,
                    encoder,
                    decoder,
                    tokens,
                });
            }
        }
        if canary_present {
            if !whisper.is_empty() {
                return Err(VoiceError::ModelMissing);
            }
            canary.check()?;
            return Ok(canary);
        }
        if whisper.len() != 1 {
            return Err(VoiceError::ModelMissing);
        }
        let paths = whisper.pop().ok_or(VoiceError::ModelMissing)?;
        paths.check()?;
        Ok(paths)
    }

    pub const fn model(&self) -> SttModel {
        self.model
    }

    fn check(&self) -> Result<(), VoiceError> {
        for p in [&self.encoder, &self.decoder, &self.tokens] {
            checked_path(p, false)?;
        }
        Ok(())
    }
}

fn checked_path(path: &Path, directory: bool) -> Result<(), VoiceError> {
    let s = path.to_str().ok_or(VoiceError::LoadFailed)?;
    if s.contains('\0') {
        return Err(VoiceError::LoadFailed);
    }
    check_path(path, directory)
}

/// Validate mono normalized PCM, including a duration between 0.1 and 30 seconds.
pub fn validate_audio(samples: &[f32], sample_rate: i32) -> Result<(), VoiceError> {
    if !(8_000..=48_000).contains(&sample_rate) {
        return Err(VoiceError::AudioInvalid);
    }
    let rate = usize::try_from(sample_rate).map_err(|_| VoiceError::AudioInvalid)?;
    if samples.len() < rate.div_ceil(10)
        || samples.len() > rate * 30
        || samples
            .iter()
            .any(|s| !s.is_finite() || !(-1.0001..=1.0001).contains(s))
    {
        return Err(VoiceError::AudioInvalid);
    }
    Ok(())
}

pub struct SttEngine {
    #[cfg(feature = "stt")]
    engine: Option<sherpa_onnx::OfflineRecognizer>,
    #[cfg(feature = "stt")]
    paths: SttPaths,
    #[cfg(feature = "stt")]
    config: SttConfig,
}

impl SttEngine {
    pub fn load(paths: SttPaths, config: SttConfig) -> Result<Self, VoiceError> {
        config.validate()?;
        paths.check()?;
        #[cfg(not(feature = "stt"))]
        {
            Err(VoiceError::Unavailable)
        }
        #[cfg(feature = "stt")]
        {
            let engine = create_recognizer(&paths, config)?;
            Ok(Self {
                engine: Some(engine),
                paths,
                config,
            })
        }
    }

    pub fn transcribe(
        &mut self,
        samples: &[f32],
        sample_rate: i32,
        language: Option<SttLanguage>,
    ) -> Result<String, VoiceError> {
        validate_audio(samples, sample_rate)?;
        #[cfg(not(feature = "stt"))]
        {
            let _ = language;
            Err(VoiceError::Unavailable)
        }
        #[cfg(feature = "stt")]
        {
            if let Some(language) = language
                && language != self.config.language
            {
                self.config.language = language;
                drop(self.engine.take());
            }
            if self.engine.is_none() {
                self.paths.check()?;
                self.engine = Some(create_recognizer(&self.paths, self.config)?);
            }
            let engine = self.engine.as_ref().ok_or(VoiceError::LoadFailed)?;
            let stream = engine.create_stream();
            stream.accept_waveform(sample_rate, samples);
            engine.decode(&stream);
            let result = stream.get_result().ok_or(VoiceError::TranscriptionFailed)?;
            Ok(result.text.trim().to_owned())
        }
    }
}

#[cfg(feature = "stt")]
fn create_recognizer(
    paths: &SttPaths,
    config: SttConfig,
) -> Result<sherpa_onnx::OfflineRecognizer, VoiceError> {
    use sherpa_onnx::{
        OfflineCanaryModelConfig, OfflineModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
        OfflineWhisperModelConfig,
    };
    fn path_string(path: &Path) -> Result<String, VoiceError> {
        let s = path.to_str().ok_or(VoiceError::LoadFailed)?;
        if s.contains('\0') {
            return Err(VoiceError::LoadFailed);
        }
        Ok(s.to_owned())
    }
    let encoder = Some(path_string(&paths.encoder)?);
    let decoder = Some(path_string(&paths.decoder)?);
    let mut model = OfflineModelConfig {
        tokens: Some(path_string(&paths.tokens)?),
        num_threads: i32::from(config.threads),
        provider: Some("cpu".into()),
        ..Default::default()
    };
    match paths.model {
        SttModel::Whisper => {
            model.model_type = Some("whisper".into());
            model.whisper = OfflineWhisperModelConfig {
                encoder,
                decoder,
                language: Some(
                    if config.language == SttLanguage::Auto {
                        ""
                    } else {
                        config.language.as_str()
                    }
                    .into(),
                ),
                task: Some("transcribe".into()),
                tail_paddings: -1,
                ..Default::default()
            };
        }
        SttModel::Canary => {
            model.model_type = Some("canary".into());
            // Canary cannot auto-detect; Auto deliberately defaults to Spanish.
            let language = if config.language == SttLanguage::Auto {
                "es"
            } else {
                config.language.as_str()
            };
            model.canary = OfflineCanaryModelConfig {
                encoder,
                decoder,
                src_lang: Some(language.into()),
                tgt_lang: Some(language.into()),
                use_pnc: true,
            };
        }
    }
    let native = OfflineRecognizerConfig {
        model_config: model,
        ..Default::default()
    };
    OfflineRecognizer::create(&native).ok_or(VoiceError::LoadFailed)
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "merken-stt-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
        fn files(&self, names: &[&str]) {
            for name in names {
                std::fs::write(self.0.join(name), []).expect("create model file");
            }
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn audio_validation() {
        let valid = vec![0.0; 800];
        assert_eq!(validate_audio(&valid, 8_000), Ok(()));
        assert_eq!(validate_audio(&vec![1.0001; 800], 8_000), Ok(()));
        assert_eq!(validate_audio(&[], 8_000), Err(VoiceError::AudioInvalid));
        assert_eq!(
            validate_audio(&vec![0.0; 799], 8_000),
            Err(VoiceError::AudioInvalid)
        );
        assert_eq!(
            validate_audio(&vec![0.0; 240_001], 8_000),
            Err(VoiceError::AudioInvalid)
        );
        assert_eq!(validate_audio(&vec![0.0; 240_000], 8_000), Ok(()));
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1.0002, -1.0002] {
            assert_eq!(
                validate_audio(&vec![value; 800], 8_000),
                Err(VoiceError::AudioInvalid)
            );
        }
        for rate in [-1, 0, 7_999, 48_001] {
            assert_eq!(validate_audio(&valid, rate), Err(VoiceError::AudioInvalid));
        }
    }

    #[test]
    fn language_config_and_codes() {
        for language in [SttLanguage::Es, SttLanguage::En, SttLanguage::Auto] {
            assert_eq!(language.as_str().parse::<SttLanguage>(), Ok(language));
        }
        assert_eq!("".parse::<SttLanguage>(), Ok(SttLanguage::Auto));
        assert_eq!("fr".parse::<SttLanguage>(), Err(VoiceError::LoadFailed));
        assert_eq!(
            SttConfig::default(),
            SttConfig {
                threads: 2,
                language: SttLanguage::Auto
            }
        );
        for threads in 1..=4 {
            assert!(
                SttConfig {
                    threads,
                    ..SttConfig::default()
                }
                .validate()
                .is_ok()
            );
        }
        for threads in [0, 5] {
            assert_eq!(
                SttConfig {
                    threads,
                    ..SttConfig::default()
                }
                .validate(),
                Err(VoiceError::LoadFailed)
            );
        }
        assert_eq!(VoiceError::AudioInvalid.code(), "audio_invalid");
        assert_eq!(
            VoiceError::TranscriptionFailed.code(),
            "transcription_failed"
        );
        assert_eq!(VoiceError::ModelMissing.code(), "model_missing");
        assert_eq!(VoiceError::LoadFailed.code(), "load_failed");
        assert_eq!(VoiceError::Unavailable.code(), "unavailable");
    }

    #[test]
    fn model_discovery() {
        let whisper = TempDir::new();
        whisper.files(&[
            "base-encoder.int8.onnx",
            "base-decoder.int8.onnx",
            "base-tokens.txt",
        ]);
        assert_eq!(
            SttPaths::from_dir(&whisper.0).expect("whisper").model(),
            SttModel::Whisper
        );
        let plain = TempDir::new();
        plain.files(&["base-encoder.onnx", "base-decoder.onnx", "base-tokens.txt"]);
        assert_eq!(
            SttPaths::from_dir(&plain.0).expect("plain whisper").model(),
            SttModel::Whisper
        );
        plain.files(&["base-encoder.int8.onnx", "base-decoder.int8.onnx"]);
        assert!(
            SttPaths::from_dir(&plain.0)
                .expect("int8 preferred")
                .encoder
                .ends_with("base-encoder.int8.onnx")
        );
        let canary = TempDir::new();
        canary.files(&["encoder.int8.onnx", "decoder.int8.onnx", "tokens.txt"]);
        assert_eq!(
            SttPaths::from_dir(&canary.0).expect("canary").model(),
            SttModel::Canary
        );
        let missing = TempDir::new();
        missing.files(&["base-encoder.int8.onnx", "base-tokens.txt"]);
        assert!(matches!(
            SttPaths::from_dir(&missing.0),
            Err(VoiceError::ModelMissing)
        ));
        assert!(matches!(
            SttPaths::from_dir(missing.0.join("absent")),
            Err(VoiceError::ModelMissing)
        ));
        whisper.files(&["encoder.int8.onnx", "decoder.int8.onnx", "tokens.txt"]);
        assert!(matches!(
            SttPaths::from_dir(&whisper.0),
            Err(VoiceError::ModelMissing)
        ));
        plain.files(&[
            "other-encoder.onnx",
            "other-decoder.onnx",
            "other-tokens.txt",
        ]);
        assert!(matches!(
            SttPaths::from_dir(&plain.0),
            Err(VoiceError::ModelMissing)
        ));
    }
}
