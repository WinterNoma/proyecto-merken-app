//! Visible English messages for the Tauri boundary (RNF-14).
pub const SLM_BUSY: &str = "The assistant is busy. Try again shortly";
pub const TEXT_TOO_LONG: &str = "Text exceeds the maximum length";
pub const REQUEST_ID: &str =
    "Request ID must contain 1 to 64 letters, digits, underscores or hyphens";
pub const SLM_UNAVAILABLE: &str = "The assistant is unavailable";
pub const SLM_INTERRUPTED: &str = "The assistant was interrupted";
pub const SLM_TIMEOUT: &str = "The assistant response timed out";
pub const SOURCE: &str = "Source must be text or voice";
pub const INDICATOR_NOT_FOUND: &str = "Indicator not found";
pub const STORAGE_UNAVAILABLE: &str = "The local database is unavailable. Restart the app";
pub const SIM_INTERRUPTED: &str = "The simulation download was interrupted";
pub const UNKNOWN_SCENARIO: &str =
    "Unknown demo scenario. Use routine, persistent_change or alarm_event";
pub const ASTRONAUT_NOT_FOUND: &str = "Astronaut not found in the simulation";

pub const VOICE_UNAVAILABLE: &str = "Local voice is unavailable";
#[cfg(feature = "voice-tts")]
pub const VOICE_MODEL_MISSING: &str = "The local voice model is missing";
#[cfg(feature = "voice-tts")]
pub const VOICE_TEXT_INVALID: &str = "The text is invalid for local voice";
#[cfg(feature = "voice-tts")]
pub const VOICE_TEXT_UNSUPPORTED: &str = "The text contains unsupported words";
#[cfg(feature = "voice-tts")]
pub const VOICE_CANCELLED: &str = "Voice synthesis was cancelled";
#[cfg(feature = "voice-tts")]
pub const VOICE_SYNTHESIS_FAILED: &str = "Local voice synthesis failed";
#[cfg(feature = "voice-tts")]
pub const VOICE_BUSY: &str = "Local voice is busy. Try again shortly";
#[cfg(feature = "voice-tts")]
pub const VOICE_CONFIG_INVALID: &str = "The voice or speed is invalid";
