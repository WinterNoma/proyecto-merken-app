use crate::{slm_error, strings, validate_request_id};
use merken_store::StoreError;
use serde::{Deserialize, Serialize};

#[cfg(feature = "voice-tts")]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
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
    #[cfg(feature = "voice-tts")]
    models_root: std::path::PathBuf,
}

impl VoiceState {
    pub fn new(root: std::path::PathBuf) -> Self {
        #[cfg(not(feature = "voice-tts"))]
        let _ = root;
        Self {
            #[cfg(feature = "voice-tts")]
            inner: Arc::default(),
            #[cfg(feature = "voice-tts")]
            models_root: root,
        }
    }
}

#[cfg(feature = "voice-tts")]
#[derive(Default)]
struct VoiceInner {
    engine: Option<(merken_voice::TtsConfig, merken_voice::TtsEngine)>,
    active: Option<(String, Arc<AtomicBool>)>,
}

fn unavailable() -> StoreError {
    slm_error("voice_unavailable", None, strings::VOICE_UNAVAILABLE)
}

#[cfg(feature = "voice-tts")]
fn voice_error(error: merken_voice::VoiceError) -> StoreError {
    use merken_voice::VoiceError;
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
        VoiceError::LoadFailed | VoiceError::SynthesisFailed => {
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
    engine: Option<(merken_voice::TtsConfig, merken_voice::TtsEngine)>,
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
        let config = merken_voice::TtsConfig {
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
                return Err(voice_error(merken_voice::VoiceError::Cancelled));
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
                    merken_voice::KokoroPaths::from_dir(root.join("kokoro-int8-multi-lang-v1_0"))
                        .map_err(voice_error)?;
                let engine = merken_voice::TtsEngine::load(paths, config).map_err(voice_error)?;
                reservation.engine = Some((config, engine));
            }
            if reservation.cancelled.load(Ordering::SeqCst) {
                return Err(voice_error(merken_voice::VoiceError::Cancelled));
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
                return Err(voice_error(merken_voice::VoiceError::Cancelled));
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
}
