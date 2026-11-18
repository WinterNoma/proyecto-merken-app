mod records;

pub use pulsys_core::{
    catalog::BodyZone,
    evaluate::{Direction, Escalation, NotEvaluatedReason, SourceKind, Status},
    record::RecordSource,
    time::Timestamp,
};
pub use pulsys_store::pulse::{
    DemoTarget, PreparedPulse, PulseOutboxState, PulsePoint, PulseReceipt, PulseTarget,
    PulseWarning, SaveOutcome,
};
pub use records::{
    HistoryQuery, PulseEvaluation, PulseInput, SymptomEvaluation, ZoneSnapshot, ZoneState,
    confirm_pulse, effective_pulse_history, prepare_pulse, zone_status,
};

use pulsys_store::{Store, StoreError, pulse::checked_timestamp};
use std::{fmt, fs, path::Path};
use time::OffsetDateTime;

const DATABASE_FILE: &str = "pulsys.sqlite3";

pub type Clock = Box<dyn Fn() -> Result<Timestamp, ApiError> + Send>;

pub struct App {
    store: Store,
    app_version: String,
    clock: Clock,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiError {
    pub code: String,
    pub field: Option<String>,
    pub message: String,
}

impl ApiError {
    fn into_store_error(self) -> StoreError {
        StoreError {
            code: self.code,
            field: self.field,
            message: self.message,
        }
    }

    fn new(code: &str, field: Option<&str>, message: &str) -> Self {
        Self {
            code: code.into(),
            field: field.map(str::to_owned),
            message: message.into(),
        }
    }
}

impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        Self {
            code: error.code,
            field: error.field,
            message: error.message,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

impl App {
    fn now(&self) -> Result<Timestamp, ApiError> {
        let now = (self.clock)()?;
        checked_timestamp(now.millis(), "now").map_err(ApiError::from)
    }
}

pub fn system_clock() -> Result<Timestamp, ApiError> {
    let ms = OffsetDateTime::now_utc()
        .unix_timestamp_nanos()
        .div_euclid(1_000_000);
    let ms = i64::try_from(ms).map_err(|_| {
        ApiError::new(
            "invalid_value",
            Some("now"),
            "The system clock is outside the supported UTC range",
        )
    })?;
    checked_timestamp(ms, "now").map_err(ApiError::from)
}

pub fn open_store(private_dir: &Path, app_version: String, clock: Clock) -> Result<App, ApiError> {
    if !private_dir.is_absolute() {
        return Err(ApiError::new(
            "invalid_value",
            Some("private_dir"),
            "Expected an absolute application directory",
        ));
    }
    fs::create_dir_all(private_dir).map_err(|_| {
        ApiError::new(
            "storage_error",
            Some("private_dir"),
            "The application directory could not be opened",
        )
    })?;
    let store = Store::open(&private_dir.join(DATABASE_FILE))?;
    Ok(App {
        store,
        app_version,
        clock,
    })
}

pub fn close_store(app: App) -> Result<(), ApiError> {
    app.store.close().map_err(ApiError::from)
}
