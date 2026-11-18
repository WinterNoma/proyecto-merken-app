//! Local Kokoro text-to-speech.
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

mod normalize;
mod stt;
use normalize::{Lexicon, normalize_text};
pub use stt::{SttConfig, SttEngine, SttLanguage, SttModel, SttPaths, validate_audio};

/// English speakers in the sherpa-onnx Kokoro v1.0 package.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum Voice {
    AfAlloy = 0,
    AfAoede = 1,
    AfBella = 2,
    #[default]
    AfHeart = 3,
    AfJessica = 4,
    AfKore = 5,
    AfNicole = 6,
    AfNova = 7,
    AfRiver = 8,
    AfSarah = 9,
    AfSky = 10,
    AmAdam = 11,
    AmEcho = 12,
    AmEric = 13,
    AmFenrir = 14,
    AmLiam = 15,
    AmMichael = 16,
    AmOnyx = 17,
    AmPuck = 18,
    AmSanta = 19,
    BfAlice = 20,
    BfEmma = 21,
    BfIsabella = 22,
    BfLily = 23,
    BmDaniel = 24,
    BmFable = 25,
    BmGeorge = 26,
    BmLewis = 27,
}
impl Voice {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AfAlloy => "af_alloy",
            Self::AfAoede => "af_aoede",
            Self::AfBella => "af_bella",
            Self::AfHeart => "af_heart",
            Self::AfJessica => "af_jessica",
            Self::AfKore => "af_kore",
            Self::AfNicole => "af_nicole",
            Self::AfNova => "af_nova",
            Self::AfRiver => "af_river",
            Self::AfSarah => "af_sarah",
            Self::AfSky => "af_sky",
            Self::AmAdam => "am_adam",
            Self::AmEcho => "am_echo",
            Self::AmEric => "am_eric",
            Self::AmFenrir => "am_fenrir",
            Self::AmLiam => "am_liam",
            Self::AmMichael => "am_michael",
            Self::AmOnyx => "am_onyx",
            Self::AmPuck => "am_puck",
            Self::AmSanta => "am_santa",
            Self::BfAlice => "bf_alice",
            Self::BfEmma => "bf_emma",
            Self::BfIsabella => "bf_isabella",
            Self::BfLily => "bf_lily",
            Self::BmDaniel => "bm_daniel",
            Self::BmFable => "bm_fable",
            Self::BmGeorge => "bm_george",
            Self::BmLewis => "bm_lewis",
        }
    }
    pub const fn id(self) -> i32 {
        self as i32
    }
}
impl FromStr for Voice {
    type Err = VoiceError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "af_alloy" => Ok(Self::AfAlloy),
            "af_aoede" => Ok(Self::AfAoede),
            "af_bella" => Ok(Self::AfBella),
            "af_heart" => Ok(Self::AfHeart),
            "af_jessica" => Ok(Self::AfJessica),
            "af_kore" => Ok(Self::AfKore),
            "af_nicole" => Ok(Self::AfNicole),
            "af_nova" => Ok(Self::AfNova),
            "af_river" => Ok(Self::AfRiver),
            "af_sarah" => Ok(Self::AfSarah),
            "af_sky" => Ok(Self::AfSky),
            "am_adam" => Ok(Self::AmAdam),
            "am_echo" => Ok(Self::AmEcho),
            "am_eric" => Ok(Self::AmEric),
            "am_fenrir" => Ok(Self::AmFenrir),
            "am_liam" => Ok(Self::AmLiam),
            "am_michael" => Ok(Self::AmMichael),
            "am_onyx" => Ok(Self::AmOnyx),
            "am_puck" => Ok(Self::AmPuck),
            "am_santa" => Ok(Self::AmSanta),
            "bf_alice" => Ok(Self::BfAlice),
            "bf_emma" => Ok(Self::BfEmma),
            "bf_isabella" => Ok(Self::BfIsabella),
            "bf_lily" => Ok(Self::BfLily),
            "bm_daniel" => Ok(Self::BmDaniel),
            "bm_fable" => Ok(Self::BmFable),
            "bm_george" => Ok(Self::BmGeorge),
            "bm_lewis" => Ok(Self::BmLewis),
            _ => Err(VoiceError::LoadFailed),
        }
    }
}

/// Stable, privacy-preserving error codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceError {
    ModelMissing,
    LoadFailed,
    AudioInvalid,
    TranscriptionFailed,
    TextInvalid,
    SynthesisFailed,
    TextUnsupported,
    Cancelled,
    Unavailable,
}
impl VoiceError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ModelMissing => "model_missing",
            Self::LoadFailed => "load_failed",
            Self::AudioInvalid => "audio_invalid",
            Self::TranscriptionFailed => "transcription_failed",
            Self::TextInvalid => "text_invalid",
            Self::SynthesisFailed => "synthesis_failed",
            Self::TextUnsupported => "text_unsupported",
            Self::Cancelled => "cancelled",
            Self::Unavailable => "unavailable",
        }
    }
}
impl fmt::Display for VoiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for VoiceError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TtsConfig {
    pub voice: Voice,
    pub speed: f32,
    pub threads: u8,
}
impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            voice: Voice::default(),
            speed: 1.0,
            threads: 2,
        }
    }
}
impl TtsConfig {
    /// An invalid configuration returns load_failed.
    pub fn validate(&self) -> Result<(), VoiceError> {
        if !(0.7..=1.3).contains(&self.speed) || !(1..=4).contains(&self.threads) {
            return Err(VoiceError::LoadFailed);
        }
        Ok(())
    }
}

/// Paths are checked at construction and again before loading.
#[derive(Clone, Debug)]
pub struct KokoroPaths {
    model: PathBuf,
    voices: PathBuf,
    tokens: PathBuf,
    lexicon: PathBuf,
    supplemental_lexicon: PathBuf,
}
impl KokoroPaths {
    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Self, VoiceError> {
        let dir = dir.as_ref();
        check_path(dir, true)?;
        let paths = Self {
            model: dir.join("model.int8.onnx"),
            voices: dir.join("voices.bin"),
            tokens: dir.join("tokens.txt"),
            lexicon: dir.join("lexicon-us-en.txt"),
            supplemental_lexicon: dir.join("pulsys-us-en.txt"),
        };
        paths.check()?;
        Ok(paths)
    }
    fn check(&self) -> Result<(), VoiceError> {
        for path in [&self.lexicon, &self.supplemental_lexicon] {
            if path.to_str().ok_or(VoiceError::LoadFailed)?.contains(',') {
                return Err(VoiceError::LoadFailed);
            }
        }
        for path in [
            &self.model,
            &self.voices,
            &self.tokens,
            &self.lexicon,
            &self.supplemental_lexicon,
        ] {
            check_path(path, false)?;
        }
        Ok(())
    }
}
/// The model folder and each model file must be real entries (no symlinks).
/// Only the entry itself is inspected: the folders above it belong to the OS, and
/// inside an Android app SELinux blocks `lstat` on some of them (`/data/user`…),
/// which made every model look missing on the phone.
fn check_path(path: &Path, directory: bool) -> Result<(), VoiceError> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| VoiceError::ModelMissing)?;
    if meta.file_type().is_symlink()
        || (directory && !meta.is_dir())
        || (!directory && !meta.is_file())
    {
        return Err(VoiceError::ModelMissing);
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pcm {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}
impl Pcm {
    /// Encode a mono, little-endian PCM16 RIFF/WAVE file.
    pub fn to_wav_bytes(&self) -> Result<Vec<u8>, VoiceError> {
        let count = u32::try_from(self.samples.len()).map_err(|_| VoiceError::SynthesisFailed)?;
        let size = count.checked_mul(2).ok_or(VoiceError::SynthesisFailed)?;
        let riff = size.checked_add(36).ok_or(VoiceError::SynthesisFailed)?;
        let byte_rate = self
            .sample_rate
            .checked_mul(2)
            .filter(|_| self.sample_rate > 0)
            .ok_or(VoiceError::SynthesisFailed)?;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&riff.to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&2_u16.to_le_bytes());
        out.extend_from_slice(&16_u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&size.to_le_bytes());
        for &sample in &self.samples {
            let sample = if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let scaled = if sample < 0.0 {
                sample * 32768.0
            } else {
                sample * 32767.0
            };
            out.extend_from_slice(&(scaled.round() as i16).to_le_bytes());
        }
        Ok(out)
    }
}

const CHUNK_CHARS: usize = 180;
fn chunks(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        let mut sentence_end = None;
        let mut word_end = None;
        let mut hard_end = remaining.len();
        for (count, (idx, ch)) in remaining.char_indices().enumerate() {
            if count == CHUNK_CHARS {
                hard_end = idx;
                break;
            }
            if ch == ' ' {
                word_end = Some(idx);
            }
            if matches!(ch, '.' | '!' | '?') {
                sentence_end = Some(idx + ch.len_utf8());
                break;
            }
        }
        let end = sentence_end.unwrap_or_else(|| {
            if hard_end == remaining.len() {
                hard_end
            } else {
                word_end
                    .filter(|&n| n > 0)
                    .unwrap_or_else(|| remaining.find(' ').unwrap_or(remaining.len()))
            }
        });
        if let Some(part) = remaining.get(..end) {
            let part = part.trim();
            if !part.is_empty() {
                result.push(part.to_owned());
            }
        }
        remaining = remaining.get(end..).unwrap_or("").trim_start();
    }
    result
}

pub struct TtsEngine {
    lexicon: Lexicon,
    #[cfg(feature = "tts")]
    engine: sherpa_onnx::OfflineTts,
    #[cfg(feature = "tts")]
    config: TtsConfig,
}
impl TtsEngine {
    pub fn load(paths: KokoroPaths, config: TtsConfig) -> Result<Self, VoiceError> {
        config.validate()?;
        paths.check()?;
        #[cfg(not(feature = "tts"))]
        {
            Err(VoiceError::Unavailable)
        }
        #[cfg(feature = "tts")]
        {
            use sherpa_onnx::{
                OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig, OfflineTtsModelConfig,
            };
            fn path_string(path: &Path) -> Result<String, VoiceError> {
                let value = path.to_str().ok_or(VoiceError::LoadFailed)?;
                if value.contains('\0') {
                    return Err(VoiceError::LoadFailed);
                }
                Ok(value.to_owned())
            }
            let lexicon = Lexicon::load_many(&[&paths.lexicon, &paths.supplemental_lexicon])?;
            let lexicon_paths = format!(
                "{},{}",
                path_string(&paths.lexicon)?,
                path_string(&paths.supplemental_lexicon)?
            );
            let native = OfflineTtsConfig {
                model: OfflineTtsModelConfig {
                    kokoro: OfflineTtsKokoroModelConfig {
                        model: Some(path_string(&paths.model)?),
                        voices: Some(path_string(&paths.voices)?),
                        tokens: Some(path_string(&paths.tokens)?),
                        lexicon: Some(lexicon_paths),
                        ..Default::default()
                    },
                    num_threads: i32::from(config.threads),
                    provider: Some("cpu".into()),
                    ..Default::default()
                },
                max_num_sentences: 100,
                silence_scale: 0.2,
                ..Default::default()
            };
            let engine = OfflineTts::create(&native).ok_or(VoiceError::LoadFailed)?;
            if engine.num_speakers() <= config.voice.id() {
                return Err(VoiceError::LoadFailed);
            }
            Ok(Self {
                engine,
                config,
                lexicon,
            })
        }
    }
    /// Return the unsupported words after applying the same normalization as synthesis.
    pub fn unsupported_words(&self, text: &str) -> Result<Vec<String>, VoiceError> {
        let normalized = normalize_text(text)?;
        Ok(self.lexicon.check(&normalized).1)
    }

    pub fn synthesize(&mut self, text: &str) -> Result<Pcm, VoiceError> {
        self.synthesize_streaming(text, |_| true)
    }

    /// Deliver complete sentences as they are synthesized. Cancellation occurs between sentences.
    pub fn synthesize_streaming(
        &mut self,
        text: &str,
        mut on_chunk: impl FnMut(&Pcm) -> bool,
    ) -> Result<Pcm, VoiceError> {
        let normalized = normalize_text(text)?;
        let (prepared, unsupported) = self.lexicon.check(&normalized);
        if !unsupported.is_empty() {
            return Err(VoiceError::TextUnsupported);
        }
        let parts = chunks(&prepared);
        #[cfg(not(feature = "tts"))]
        {
            let _ = (parts, &mut on_chunk);
            Err(VoiceError::Unavailable)
        }
        #[cfg(feature = "tts")]
        {
            let options = sherpa_onnx::GenerationConfig {
                sid: self.config.voice.id(),
                speed: self.config.speed,
                ..Default::default()
            };
            let mut samples = Vec::new();
            let mut rate = None;
            for part in parts {
                let audio = self
                    .engine
                    .generate_with_config(&part, &options, None::<fn(&[f32], f32) -> bool>)
                    .ok_or(VoiceError::SynthesisFailed)?;
                let sample_rate = u32::try_from(audio.sample_rate())
                    .ok()
                    .filter(|&r| r > 0)
                    .ok_or(VoiceError::SynthesisFailed)?;
                if rate.is_some_and(|r| r != sample_rate) || audio.samples().is_empty() {
                    return Err(VoiceError::SynthesisFailed);
                }
                rate = Some(sample_rate);
                let chunk = Pcm {
                    samples: audio.samples().to_vec(),
                    sample_rate,
                };
                if !on_chunk(&chunk) {
                    return Err(VoiceError::Cancelled);
                }
                samples.extend_from_slice(&chunk.samples);
            }
            Ok(Pcm {
                samples,
                sample_rate: rate.ok_or(VoiceError::SynthesisFailed)?,
            })
        }
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
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn config_ranges_and_voices() {
        assert_eq!(TtsConfig::default().voice, Voice::AfHeart);
        for voice in [
            Voice::AfAlloy,
            Voice::AfAoede,
            Voice::AfBella,
            Voice::AfHeart,
            Voice::AfJessica,
            Voice::AfKore,
            Voice::AfNicole,
            Voice::AfNova,
            Voice::AfRiver,
            Voice::AfSarah,
            Voice::AfSky,
            Voice::AmAdam,
            Voice::AmEcho,
            Voice::AmEric,
            Voice::AmFenrir,
            Voice::AmLiam,
            Voice::AmMichael,
            Voice::AmOnyx,
            Voice::AmPuck,
            Voice::AmSanta,
            Voice::BfAlice,
            Voice::BfEmma,
            Voice::BfIsabella,
            Voice::BfLily,
            Voice::BmDaniel,
            Voice::BmFable,
            Voice::BmGeorge,
            Voice::BmLewis,
        ] {
            assert_eq!(voice.as_str().parse::<Voice>(), Ok(voice));
        }
        for speed in [0.7, 1.3] {
            assert!(
                TtsConfig {
                    speed,
                    ..TtsConfig::default()
                }
                .validate()
                .is_ok()
            );
        }
        for speed in [0.69, 1.31, f32::NAN, f32::INFINITY] {
            assert_eq!(
                TtsConfig {
                    speed,
                    ..TtsConfig::default()
                }
                .validate(),
                Err(VoiceError::LoadFailed)
            );
        }
        for threads in [0, 5] {
            assert_eq!(
                TtsConfig {
                    threads,
                    ..TtsConfig::default()
                }
                .validate(),
                Err(VoiceError::LoadFailed)
            );
        }
    }
    #[test]
    fn text_validation_and_chunking() {
        assert_eq!(
            normalize_text("  Hello\n  world!\r"),
            Ok("Hello world !".into())
        );
        assert_eq!(normalize_text(" \n"), Err(VoiceError::TextInvalid));
        assert_eq!(normalize_text("a\tb"), Err(VoiceError::TextInvalid));
        assert_eq!(
            normalize_text(&"x".repeat(601)),
            Err(VoiceError::TextInvalid)
        );
        let input = format!(
            "{}! {} {}",
            "é".repeat(90),
            "a".repeat(100),
            "b".repeat(150)
        );
        let parts = chunks(&input);
        assert!(parts.iter().all(|part| part.chars().count() <= CHUNK_CHARS));
        assert_eq!(parts.join("").replace(' ', ""), input.replace(' ', ""));
        assert!(parts[0].ends_with('!'));
    }
    #[test]
    fn exact_wav_bytes() {
        let pcm = Pcm {
            samples: vec![-1.0, 1.0],
            sample_rate: 24_000,
        };
        assert_eq!(
            pcm.to_wav_bytes().unwrap(),
            vec![
                82, 73, 70, 70, 40, 0, 0, 0, 87, 65, 86, 69, 102, 109, 116, 32, 16, 0, 0, 0, 1, 0,
                1, 0, 192, 93, 0, 0, 128, 187, 0, 0, 2, 0, 16, 0, 100, 97, 116, 97, 4, 0, 0, 0, 0,
                128, 255, 127
            ]
        );
    }
    #[test]
    fn model_paths_require_files_and_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("pulsys-voice-{}-{unique}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        assert!(matches!(
            KokoroPaths::from_dir(&dir),
            Err(VoiceError::ModelMissing)
        ));
        for name in ["model.int8.onnx", "voices.bin", "tokens.txt"] {
            std::fs::write(dir.join(name), []).unwrap();
        }
        assert!(matches!(
            KokoroPaths::from_dir(&dir),
            Err(VoiceError::ModelMissing)
        ));
        std::fs::write(dir.join("lexicon-us-en.txt"), b"x || x\n").unwrap();
        assert!(matches!(
            KokoroPaths::from_dir(&dir),
            Err(VoiceError::ModelMissing)
        ));
        std::fs::write(dir.join("pulsys-us-en.txt"), b"y y\n").unwrap();
        let comma_dir = dir.join("model,comma");
        std::fs::create_dir(&comma_dir).unwrap();
        assert!(matches!(
            KokoroPaths::from_dir(&comma_dir),
            Err(VoiceError::LoadFailed)
        ));
        let paths = KokoroPaths::from_dir(&dir).unwrap();
        #[cfg(not(feature = "tts"))]
        assert!(matches!(
            TtsEngine::load(paths, TtsConfig::default()),
            Err(VoiceError::Unavailable)
        ));
        #[cfg(feature = "tts")]
        let _ = paths;
        std::fs::remove_dir_all(dir).unwrap();
    }
}
