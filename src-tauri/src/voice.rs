use crate::{slm_error, strings, validate_request_id};
use pulsys_store::StoreError;
use serde::{Deserialize, Serialize};

#[cfg(feature = "voice-tts")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(any(feature = "voice-tts", feature = "voice-stt"))]
use std::sync::{Arc, Mutex};
#[cfg(feature = "voice-tts")]
use tauri::Emitter;

#[derive(Deserialize)]
pub struct SpeakInput {
    request_id: String,
    text: String,
    voice: Option<String>,
    speed: Option<f32>,
}

#[derive(Serialize)]
pub struct SpeakResponse {
    request_id: String,
    chunks: usize,
    duration_ms: u64,
}

#[derive(Deserialize)]
pub struct TranscribeInput {
    request_id: String,
    pcm_f32le_base64: String,
    sample_rate: i64,
    language: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TranscribeResponse {
    request_id: String,
    text: String,
    duration_ms: u64,
}

#[cfg(feature = "voice-tts")]
#[derive(Clone, Serialize)]
struct VoiceChunkEvent {
    request_id: String,
    index: usize,
    wav_base64: String,
}

#[derive(Default)]
pub struct VoiceState {
    #[cfg(feature = "voice-tts")]
    inner: Arc<Mutex<VoiceInner>>,
    #[cfg(feature = "voice-stt")]
    stt_inner: Arc<Mutex<SttInner>>,
    #[cfg(any(feature = "voice-tts", feature = "voice-stt"))]
    models_root: std::path::PathBuf,
}

impl VoiceState {
    pub fn new(root: std::path::PathBuf) -> Self {
        #[cfg(not(any(feature = "voice-tts", feature = "voice-stt")))]
        let _ = root;
        Self {
            #[cfg(feature = "voice-tts")]
            inner: Arc::default(),
            #[cfg(feature = "voice-stt")]
            stt_inner: Arc::default(),
            #[cfg(any(feature = "voice-tts", feature = "voice-stt"))]
            models_root: root,
        }
    }
}

#[cfg(feature = "voice-tts")]
#[derive(Default)]
struct VoiceInner {
    engine: Option<(pulsys_voice::TtsConfig, pulsys_voice::TtsEngine)>,
    active: Option<(String, Arc<AtomicBool>)>,
}

fn unavailable() -> StoreError {
    slm_error("voice_unavailable", None, strings::VOICE_UNAVAILABLE)
}

#[cfg(feature = "voice-tts")]
fn voice_error(error: pulsys_voice::VoiceError) -> StoreError {
    use pulsys_voice::VoiceError;
    match error {
        VoiceError::ModelMissing => slm_error("model_missing", None, strings::VOICE_MODEL_MISSING),
        VoiceError::TextInvalid => {
            slm_error("text_invalid", Some("text"), strings::VOICE_TEXT_INVALID)
        }
        VoiceError::TextUnsupported => slm_error(
            "text_unsupported",
            Some("text"),
            strings::VOICE_TEXT_UNSUPPORTED,
        ),
        VoiceError::Cancelled => slm_error("cancelled", None, strings::VOICE_CANCELLED),
        VoiceError::Unavailable => unavailable(),
        VoiceError::LoadFailed
        | VoiceError::SynthesisFailed
        | VoiceError::AudioInvalid
        | VoiceError::TranscriptionFailed => {
            slm_error("synthesis_failed", None, strings::VOICE_SYNTHESIS_FAILED)
        }
    }
}

#[cfg(feature = "voice-tts")]
fn busy() -> StoreError {
    slm_error("busy", None, strings::VOICE_BUSY)
}
#[cfg(feature = "voice-tts")]
fn failed() -> StoreError {
    slm_error("synthesis_failed", None, strings::VOICE_SYNTHESIS_FAILED)
}

// The reservation lives in the blocking task, even if the command future is dropped.
// Drop also clears it if the task unwinds.
#[cfg(feature = "voice-tts")]
struct Reservation {
    inner: Arc<Mutex<VoiceInner>>,
    engine: Option<(pulsys_voice::TtsConfig, pulsys_voice::TtsEngine)>,
    cancelled: Arc<AtomicBool>,
}
#[cfg(feature = "voice-tts")]
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.engine = self.engine.take();
        inner.active = None;
    }
}

#[tauri::command]
pub async fn speak_text(
    app: tauri::AppHandle,
    state: tauri::State<'_, VoiceState>,
    input: SpeakInput,
) -> Result<SpeakResponse, StoreError> {
    validate_request_id(&input.request_id)?;
    #[cfg(not(feature = "voice-tts"))]
    {
        let _ = (app, state, input.text, input.voice, input.speed);
        Err(unavailable())
    }
    #[cfg(feature = "voice-tts")]
    {
        let voice = input
            .voice
            .as_deref()
            .unwrap_or("af_heart")
            .parse()
            .map_err(|_| slm_error("text_invalid", Some("voice"), strings::VOICE_CONFIG_INVALID))?;
        let config = pulsys_voice::TtsConfig {
            voice,
            speed: input.speed.unwrap_or(1.0),
            threads: 4,
        };
        config
            .validate()
            .map_err(|_| slm_error("text_invalid", Some("speed"), strings::VOICE_CONFIG_INVALID))?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let reservation = {
            let mut inner = state.inner.lock().map_err(|_| failed())?;
            if inner.active.is_some() {
                return Err(busy());
            }
            inner.active = Some((input.request_id.clone(), Arc::clone(&cancelled)));
            Reservation {
                inner: Arc::clone(&state.inner),
                engine: inner.engine.take(),
                cancelled,
            }
        };
        let root = state.models_root.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut reservation = reservation;
            if reservation.cancelled.load(Ordering::SeqCst) {
                return Err(voice_error(pulsys_voice::VoiceError::Cancelled));
            }
            if reservation
                .engine
                .as_ref()
                .is_some_and(|(old, _)| *old != config)
            {
                reservation.engine = None;
            }
            if reservation.engine.is_none() {
                let paths =
                    pulsys_voice::KokoroPaths::from_dir(root.join("kokoro-int8-multi-lang-v1_0"))
                        .map_err(voice_error)?;
                let engine = pulsys_voice::TtsEngine::load(paths, config).map_err(voice_error)?;
                reservation.engine = Some((config, engine));
            }
            if reservation.cancelled.load(Ordering::SeqCst) {
                return Err(voice_error(pulsys_voice::VoiceError::Cancelled));
            }
            let mut count = 0_usize;
            let mut callback_failed = false;
            let request_id = input.request_id;
            let token = Arc::clone(&reservation.cancelled);
            let engine = &mut reservation.engine.as_mut().ok_or_else(failed)?.1;
            let pcm = engine.synthesize_streaming(&input.text, |chunk| {
                if token.load(Ordering::SeqCst) {
                    return false;
                }
                let Ok(wav) = chunk.to_wav_bytes() else {
                    callback_failed = true;
                    return false;
                };
                let event = VoiceChunkEvent {
                    request_id: request_id.clone(),
                    index: count,
                    wav_base64: encode_base64(&wav),
                };
                if app.emit("voice-chunk", event).is_err() {
                    callback_failed = true;
                    return false;
                }
                count += 1;
                true
            });
            if callback_failed {
                return Err(failed());
            }
            let pcm = pcm.map_err(voice_error)?;
            if token.load(Ordering::SeqCst) {
                return Err(voice_error(pulsys_voice::VoiceError::Cancelled));
            }
            let duration_ms =
                (pcm.samples.len() as u64).saturating_mul(1000) / u64::from(pcm.sample_rate);
            Ok(SpeakResponse {
                request_id,
                chunks: count,
                duration_ms,
            })
        })
        .await
        .map_err(|_| failed())?
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn speak_cancel(
    state: tauri::State<'_, VoiceState>,
    request_id: String,
) -> Result<bool, StoreError> {
    validate_request_id(&request_id)?;
    #[cfg(not(feature = "voice-tts"))]
    {
        let _ = state;
        Err(unavailable())
    }
    #[cfg(feature = "voice-tts")]
    {
        let inner = state.inner.lock().map_err(|_| failed())?;
        if let Some((id, token)) = &inner.active
            && id == &request_id
        {
            token.store(true, Ordering::SeqCst);
            return Ok(true);
        }
        Ok(false)
    }
}

#[tauri::command]
pub async fn voice_release(state: tauri::State<'_, VoiceState>) -> Result<bool, StoreError> {
    #[cfg(not(feature = "voice-tts"))]
    {
        let _ = state;
        Err(unavailable())
    }
    #[cfg(feature = "voice-tts")]
    {
        let inner = Arc::clone(&state.inner);
        tauri::async_runtime::spawn_blocking(move || {
            let mut inner = inner.lock().map_err(|_| failed())?;
            if inner.active.is_some() {
                return Err(busy());
            }
            Ok(inner.engine.take().is_some())
        })
        .await
        .map_err(|_| failed())?
    }
}

#[cfg(any(feature = "voice-stt", test))]
const STT_MAX_AUDIO_BYTES: usize = 30 * 48_000 * 4;
#[cfg(any(feature = "voice-stt", test))]
const STT_MAX_BASE64_LEN: usize = STT_MAX_AUDIO_BYTES.div_ceil(3) * 4;
#[cfg(feature = "voice-stt")]
const STT_THREADS: u8 = 2;
#[cfg(any(feature = "voice-stt", test))]
const STT_MODEL_DIR_WHISPER_BASE: &str = "stt-whisper-base";
#[cfg(any(feature = "voice-stt", test))]
const STT_MODEL_DIR_WHISPER_SMALL: &str = "stt-whisper-small";
#[cfg(any(feature = "voice-stt", test))]
const STT_MODEL_DIR_CANARY: &str = "stt-canary-180m";
#[cfg(any(feature = "voice-stt", test))]
const STT_MODEL_DIRS: [&str; 3] = [
    STT_MODEL_DIR_WHISPER_BASE,
    STT_MODEL_DIR_WHISPER_SMALL,
    STT_MODEL_DIR_CANARY,
];

#[cfg(any(feature = "voice-stt", test))]
fn select_stt_model_dir(root: &std::path::Path) -> Option<std::path::PathBuf> {
    STT_MODEL_DIRS
        .iter()
        .map(|name| root.join(name))
        .find(|dir| dir.is_dir())
}

#[cfg(feature = "voice-stt")]
#[derive(Default)]
struct SttInner {
    engine: Option<(std::path::PathBuf, pulsys_voice::SttEngine)>,
    active: Option<String>,
}

#[cfg(feature = "voice-stt")]
struct SttReservation {
    inner: Arc<Mutex<SttInner>>,
    engine: Option<(std::path::PathBuf, pulsys_voice::SttEngine)>,
}

#[cfg(feature = "voice-stt")]
impl Drop for SttReservation {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.engine = self.engine.take();
        inner.active = None;
    }
}

#[cfg(feature = "voice-stt")]
fn stt_voice_error(error: pulsys_voice::VoiceError) -> StoreError {
    use pulsys_voice::VoiceError;
    // Only the error kind (no audio, text or paths): visible in logcat on Android.
    eprintln!("pulsys dictation error: {error:?}");
    match error {
        VoiceError::ModelMissing => slm_error("model_missing", None, strings::STT_MODEL_MISSING),
        VoiceError::AudioInvalid => slm_error("audio_invalid", None, strings::STT_AUDIO_INVALID),
        VoiceError::Unavailable => unavailable(),
        VoiceError::Cancelled => slm_error("cancelled", None, strings::STT_CANCELLED),
        VoiceError::LoadFailed
        | VoiceError::TranscriptionFailed
        | VoiceError::TextInvalid
        | VoiceError::SynthesisFailed
        | VoiceError::TextUnsupported => slm_error(
            "transcription_failed",
            None,
            strings::STT_TRANSCRIPTION_FAILED,
        ),
    }
}

#[cfg(feature = "voice-stt")]
fn stt_busy() -> StoreError {
    slm_error("busy", None, strings::STT_BUSY)
}

#[cfg(feature = "voice-stt")]
fn stt_failed() -> StoreError {
    slm_error(
        "transcription_failed",
        None,
        strings::STT_TRANSCRIPTION_FAILED,
    )
}

#[cfg(any(feature = "voice-stt", test))]
fn parse_sample_rate(sample_rate: i64) -> Result<i32, StoreError> {
    i32::try_from(sample_rate)
        .ok()
        .filter(|&rate| (8_000..=48_000).contains(&rate))
        .ok_or_else(|| {
            slm_error(
                "audio_invalid",
                Some("sample_rate"),
                strings::STT_SAMPLE_RATE_INVALID,
            )
        })
}

#[cfg(any(feature = "voice-stt", test))]
fn validate_language_tag(language: Option<&str>) -> Result<&str, StoreError> {
    let tag = language.unwrap_or("auto");
    match tag {
        "es" | "en" | "auto" => Ok(tag),
        _ => Err(slm_error(
            "audio_invalid",
            Some("language"),
            strings::STT_LANGUAGE_INVALID,
        )),
    }
}

#[cfg(any(feature = "voice-stt", test))]
fn b64_val(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(any(feature = "voice-stt", test))]
fn decode_base64_strict(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity((bytes.len() / 4) * 3);
    let mut saw_padding = false;
    for chunk in bytes.chunks_exact(4) {
        if saw_padding {
            return None;
        }
        let &[c0, c1, c2, c3] = chunk else {
            return None;
        };
        let v0 = b64_val(c0)?;
        let v1 = b64_val(c1)?;
        match (c2, c3) {
            (b'=', b'=') => {
                if (v1 & 0x0f) != 0 {
                    return None;
                }
                out.push((v0 << 2) | (v1 >> 4));
                saw_padding = true;
            }
            (b'=', _) => return None,
            (_, b'=') => {
                let v2 = b64_val(c2)?;
                if (v2 & 0x03) != 0 {
                    return None;
                }
                out.push((v0 << 2) | (v1 >> 4));
                out.push(((v1 & 0x0f) << 4) | (v2 >> 2));
                saw_padding = true;
            }
            (_, _) => {
                let v2 = b64_val(c2)?;
                let v3 = b64_val(c3)?;
                out.push((v0 << 2) | (v1 >> 4));
                out.push(((v1 & 0x0f) << 4) | (v2 >> 2));
                out.push(((v2 & 0x03) << 6) | v3);
            }
        }
    }
    Some(out)
}

#[cfg(any(feature = "voice-stt", test))]
fn bytes_to_f32le(bytes: &[u8]) -> Option<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut samples = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let &[b0, b1, b2, b3] = chunk else {
            return None;
        };
        let sample = f32::from_le_bytes([b0, b1, b2, b3]);
        if !sample.is_finite() {
            return None;
        }
        samples.push(sample);
    }
    Some(samples)
}

#[cfg(any(feature = "voice-stt", test))]
fn decode_pcm_f32le(pcm_b64: &str) -> Result<Vec<f32>, StoreError> {
    let invalid = || {
        slm_error(
            "audio_invalid",
            Some("pcm_f32le_base64"),
            strings::STT_AUDIO_INVALID,
        )
    };
    if pcm_b64.is_empty() || pcm_b64.len() > STT_MAX_BASE64_LEN {
        return Err(invalid());
    }
    let bytes = decode_base64_strict(pcm_b64).ok_or_else(invalid)?;
    if bytes.is_empty() || bytes.len() > STT_MAX_AUDIO_BYTES {
        return Err(invalid());
    }
    bytes_to_f32le(&bytes).ok_or_else(invalid)
}

#[cfg(feature = "voice-stt")]
fn run_transcribe_blocking(
    stt_inner: Arc<Mutex<SttInner>>,
    root: std::path::PathBuf,
    request_id: String,
    pcm_b64: String,
    sample_rate: i32,
    language: pulsys_voice::SttLanguage,
) -> Result<TranscribeResponse, StoreError> {
    let samples = decode_pcm_f32le(&pcm_b64)?;
    pulsys_voice::validate_audio(&samples, sample_rate).map_err(stt_voice_error)?;
    let mut reservation = {
        let mut inner = stt_inner.lock().map_err(|_| stt_failed())?;
        if inner.active.is_some() {
            return Err(stt_busy());
        }
        inner.active = Some(request_id.clone());
        SttReservation {
            inner: Arc::clone(&stt_inner),
            engine: inner.engine.take(),
        }
    };
    let dir = select_stt_model_dir(&root)
        .ok_or_else(|| stt_voice_error(pulsys_voice::VoiceError::ModelMissing))?;
    if reservation
        .engine
        .as_ref()
        .is_some_and(|(old_dir, _)| *old_dir != dir)
    {
        reservation.engine = None;
    }
    if reservation.engine.is_none() {
        let paths = pulsys_voice::SttPaths::from_dir(dir.clone()).map_err(stt_voice_error)?;
        let config = pulsys_voice::SttConfig {
            threads: STT_THREADS,
            language,
        };
        config.validate().map_err(stt_voice_error)?;
        let engine = pulsys_voice::SttEngine::load(paths, config).map_err(stt_voice_error)?;
        reservation.engine = Some((dir, engine));
    }
    let engine = &mut reservation.engine.as_mut().ok_or_else(stt_failed)?.1;
    let started = std::time::Instant::now();
    let text = engine
        .transcribe(&samples, sample_rate, Some(language))
        .map_err(stt_voice_error)?;
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    Ok(TranscribeResponse {
        request_id,
        text,
        duration_ms,
    })
}

#[tauri::command]
pub async fn transcribe_audio(
    app: tauri::AppHandle,
    state: tauri::State<'_, VoiceState>,
    input: TranscribeInput,
) -> Result<TranscribeResponse, StoreError> {
    validate_request_id(&input.request_id)?;
    #[cfg(not(feature = "voice-stt"))]
    {
        let _ = (
            app,
            state,
            input.pcm_f32le_base64,
            input.sample_rate,
            input.language,
        );
        Err(unavailable())
    }
    #[cfg(feature = "voice-stt")]
    {
        let sample_rate = parse_sample_rate(input.sample_rate)?;
        let lang_tag = validate_language_tag(input.language.as_deref())?;
        let language: pulsys_voice::SttLanguage = lang_tag.parse().map_err(|_| {
            slm_error(
                "audio_invalid",
                Some("language"),
                strings::STT_LANGUAGE_INVALID,
            )
        })?;
        if input.pcm_f32le_base64.is_empty() || input.pcm_f32le_base64.len() > STT_MAX_BASE64_LEN {
            return Err(slm_error(
                "audio_invalid",
                Some("pcm_f32le_base64"),
                strings::STT_AUDIO_INVALID,
            ));
        }
        let stt_inner = Arc::clone(&state.stt_inner);
        let root = state.models_root.clone();
        let request_id = input.request_id;
        let pcm_b64 = input.pcm_f32le_base64;
        tauri::async_runtime::spawn_blocking(move || {
            // En Android los modelos (Whisper incluido) vienen dentro de la APK y se
            // copian una sola vez a la raíz de modelos antes del primer uso.
            #[cfg(target_os = "android")]
            crate::recommendations::ensure_models(&app)
                .map_err(|_| slm_error("model_missing", None, strings::STT_MODEL_MISSING))?;
            #[cfg(not(target_os = "android"))]
            let _ = &app;
            run_transcribe_blocking(stt_inner, root, request_id, pcm_b64, sample_rate, language)
        })
        .await
        .map_err(|_| stt_failed())?
    }
}

#[tauri::command]
pub async fn stt_release(state: tauri::State<'_, VoiceState>) -> Result<bool, StoreError> {
    #[cfg(not(feature = "voice-stt"))]
    {
        let _ = state;
        Err(unavailable())
    }
    #[cfg(feature = "voice-stt")]
    {
        let inner = Arc::clone(&state.stt_inner);
        tauri::async_runtime::spawn_blocking(move || {
            let mut inner = inner.lock().map_err(|_| stt_failed())?;
            if inner.active.is_some() {
                return Err(stt_busy());
            }
            Ok(inner.engine.take().is_some())
        })
        .await
        .map_err(|_| stt_failed())?
    }
}

#[cfg(any(feature = "voice-tts", test))]
fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = u32::from(chunk[0]);
        let b = u32::from(*chunk.get(1).unwrap_or(&0));
        let c = u32::from(*chunk.get(2).unwrap_or(&0));
        let n = (a << 16) | (b << 8) | c;
        out.push(char::from(TABLE[((n >> 18) & 63) as usize]));
        out.push(char::from(TABLE[((n >> 12) & 63) as usize]));
        out.push(if chunk.len() > 1 {
            char::from(TABLE[((n >> 6) & 63) as usize])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(TABLE[(n & 63) as usize])
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn base64_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode_base64(input.as_bytes()), expected);
        }
        assert_eq!(encode_base64(&[0xfb, 0xff, 0xff]), "+///");
    }
    #[test]
    fn request_ids_and_unavailable_error_are_stable() {
        assert!(validate_request_id("valid_ID-1").is_ok());
        for id in ["", "bad id", "é", &"a".repeat(65)] {
            let error = validate_request_id(id).expect_err("invalid ID");
            assert_eq!(error.code, "invalid_value");
            assert_eq!(error.field.as_deref(), Some("request_id"));
        }
        let error = unavailable();
        assert_eq!(error.code, "voice_unavailable");
        assert_eq!(error.message, strings::VOICE_UNAVAILABLE);
    }

    #[test]
    fn base64_strict_decode_rfc4648_and_invalid() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(
                decode_base64_strict(encoded).as_deref(),
                Some(plain.as_bytes())
            );
        }
        assert_eq!(
            decode_base64_strict("+///").as_deref(),
            Some([0xfb, 0xff, 0xff].as_slice())
        );
        for invalid in [
            "a", "Zg=", "Zg===", "Zg=a", "Z===", "====", "Zg==Zg==", "Zm-v", "Zm_v", "Zm8 ",
            "Zm8\n", "Zm9*", "Zh==", "Zm9=",
        ] {
            assert_eq!(
                decode_base64_strict(invalid),
                None,
                "should reject {invalid:?}"
            );
        }
    }

    #[test]
    fn bytes_to_f32le_and_decode_pcm_validate_size_and_format() {
        let samples = [-1.0_f32, -0.25, 0.0, 0.5, 1.0];
        let mut raw = Vec::with_capacity(samples.len() * 4);
        for s in samples {
            raw.extend_from_slice(&s.to_le_bytes());
        }
        assert_eq!(bytes_to_f32le(&raw).as_deref(), Some(samples.as_slice()));
        assert_eq!(bytes_to_f32le(&[0, 1, 2]), None);
        assert_eq!(bytes_to_f32le(&f32::NAN.to_le_bytes()), None);
        assert_eq!(bytes_to_f32le(&f32::INFINITY.to_le_bytes()), None);

        let encoded = encode_base64(&raw);
        assert_eq!(
            decode_pcm_f32le(&encoded).ok().as_deref(),
            Some(samples.as_slice())
        );

        for bad in [
            "",
            "Zg==",
            "Zm9v",
            "not!base64",
            &"A".repeat(STT_MAX_BASE64_LEN + 4),
        ] {
            assert!(decode_pcm_f32le(bad).is_err_and(|e| {
                e.code == "audio_invalid" && e.field.as_deref() == Some("pcm_f32le_base64")
            }));
        }
    }

    #[test]
    fn stt_sample_rate_and_language_validation() {
        for ok_rate in [8_000, 16_000, 44_100, 48_000] {
            assert_eq!(parse_sample_rate(ok_rate).ok(), i32::try_from(ok_rate).ok());
        }
        for bad_rate in [7_999, 48_001, 0, -16_000, i64::from(i32::MAX) + 1, i64::MIN] {
            assert!(parse_sample_rate(bad_rate).is_err_and(|e| {
                e.code == "audio_invalid" && e.field.as_deref() == Some("sample_rate")
            }));
        }

        assert_eq!(validate_language_tag(None).ok(), Some("auto"));
        for ok_lang in ["es", "en", "auto"] {
            assert_eq!(validate_language_tag(Some(ok_lang)).ok(), Some(ok_lang));
        }
        for bad_lang in ["", "fr", "ES", "en-US"] {
            assert!(validate_language_tag(Some(bad_lang)).is_err_and(|e| {
                e.code == "audio_invalid" && e.field.as_deref() == Some("language")
            }));
        }
    }

    #[test]
    fn stt_model_dir_prefers_whisper_base_then_small_then_canary() {
        let root = std::env::temp_dir().join(format!(
            "pulsys-stt-dir-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&root);
        assert!(std::fs::create_dir_all(&root).is_ok());
        assert_eq!(select_stt_model_dir(&root), None);

        let canary = root.join(STT_MODEL_DIR_CANARY);
        assert!(std::fs::create_dir_all(&canary).is_ok());
        assert_eq!(select_stt_model_dir(&root), Some(canary.clone()));

        assert!(std::fs::remove_dir_all(&canary).is_ok());
        let whisper_small = root.join(STT_MODEL_DIR_WHISPER_SMALL);
        assert!(std::fs::create_dir_all(&whisper_small).is_ok());
        assert_eq!(select_stt_model_dir(&root), Some(whisper_small.clone()));

        assert!(std::fs::create_dir_all(&canary).is_ok());
        assert_eq!(select_stt_model_dir(&root), Some(whisper_small.clone()));

        let whisper_base = root.join(STT_MODEL_DIR_WHISPER_BASE);
        assert!(std::fs::create_dir_all(&whisper_base).is_ok());
        assert_eq!(select_stt_model_dir(&root), Some(whisper_base.clone()));

        let _ = std::fs::remove_dir_all(&whisper_small);
        assert_eq!(select_stt_model_dir(&root), Some(whisper_base));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(feature = "voice-stt")]
    #[test]
    fn stt_voice_error_mapping_is_stable() {
        use pulsys_voice::VoiceError;
        assert_eq!(
            stt_voice_error(VoiceError::ModelMissing).code,
            "model_missing"
        );
        assert_eq!(
            stt_voice_error(VoiceError::AudioInvalid).code,
            "audio_invalid"
        );
        assert_eq!(
            stt_voice_error(VoiceError::Unavailable).code,
            "voice_unavailable"
        );
        assert_eq!(stt_voice_error(VoiceError::Cancelled).code, "cancelled");
        for err in [
            VoiceError::LoadFailed,
            VoiceError::TranscriptionFailed,
            VoiceError::TextInvalid,
            VoiceError::SynthesisFailed,
            VoiceError::TextUnsupported,
        ] {
            assert_eq!(stt_voice_error(err).code, "transcription_failed");
        }
    }

    #[cfg(feature = "voice-stt")]
    #[test]
    fn stt_invalid_audio_fails_before_model_discovery_or_reservation() {
        let root = std::env::temp_dir().join(format!(
            "pulsys-stt-audio-val-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&root);
        assert!(std::fs::create_dir_all(&root).is_ok());

        let state = VoiceState::new(root.clone());

        let encode_samples = |samples: &[f32]| -> String {
            let mut raw = Vec::with_capacity(samples.len() * 4);
            for s in samples {
                raw.extend_from_slice(&s.to_le_bytes());
            }
            encode_base64(&raw)
        };

        // Too short (< 0.1 s at 16 kHz) must fail with audio_invalid, not model_missing.
        let short_b64 = encode_samples(&[0.0_f32; 8]);
        let err = run_transcribe_blocking(
            Arc::clone(&state.stt_inner),
            root.clone(),
            "req_short".into(),
            short_b64,
            16_000,
            pulsys_voice::SttLanguage::Auto,
        )
        .expect_err("short audio should be rejected");
        assert_eq!(err.code, "audio_invalid");
        assert!(state.stt_inner.lock().unwrap().active.is_none());

        // Out-of-range sample must fail with audio_invalid before model lookup.
        let clip_b64 = encode_samples(&vec![1.5_f32; 1_600]);
        let err = run_transcribe_blocking(
            Arc::clone(&state.stt_inner),
            root.clone(),
            "req_clip".into(),
            clip_b64,
            16_000,
            pulsys_voice::SttLanguage::Auto,
        )
        .expect_err("out-of-range audio should be rejected");
        assert_eq!(err.code, "audio_invalid");
        assert!(state.stt_inner.lock().unwrap().active.is_none());

        // Valid audio passes validation and fails only at model discovery.
        let valid_b64 = encode_samples(&vec![0.0_f32; 1_600]);
        let err = run_transcribe_blocking(
            Arc::clone(&state.stt_inner),
            root.clone(),
            "req_valid".into(),
            valid_b64,
            16_000,
            pulsys_voice::SttLanguage::Auto,
        )
        .expect_err("missing model directory should fail with model_missing");
        assert_eq!(err.code, "model_missing");
        assert!(state.stt_inner.lock().unwrap().active.is_none());

        let _ = std::fs::remove_dir_all(&root);
    }
}
