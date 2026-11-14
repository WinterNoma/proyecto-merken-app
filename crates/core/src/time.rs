//! Instantes UTC y tiempo transcurrido de misión.
use crate::{ErrorCode, ValidationError};
use ::time::{OffsetDateTime, format_description::well_known::Rfc3339};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Milisegundos transcurridos desde la época Unix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);
impl Timestamp {
    pub const fn from_millis(ms: i64) -> Self {
        Self(ms)
    }
    pub const fn millis(self) -> i64 {
        self.0
    }
    pub fn parse_rfc3339(s: &str) -> Result<Self, ValidationError> {
        let parsed = OffsetDateTime::parse(s, &Rfc3339).map_err(|_| ValidationError {
            code: ErrorCode::InvalidValue,
            field: "timestamp".into(),
            message: "Invalid RFC 3339 timestamp".into(),
        })?;
        let ms = parsed.unix_timestamp_nanos().div_euclid(1_000_000);
        i64::try_from(ms).map(Self).map_err(|_| ValidationError {
            code: ErrorCode::InvalidValue,
            field: "timestamp".into(),
            message: "Timestamp is out of range".into(),
        })
    }
    /// Formato fijo `AAAA-MM-DDTHH:MM:SS.mmmZ`. Fuera del rango de años
    /// 0..=9999 (lo que admite RFC 3339) se satura al extremo más cercano (nunca entra en pánico).
    pub fn to_rfc3339(self) -> String {
        const MIN_MS: i64 = -62_167_219_200_000; // 0000-01-01T00:00:00Z
        const MAX_MS: i64 = 253_402_300_799_999; // 9999-12-31T23:59:59.999Z
        let ms = self.0.clamp(MIN_MS, MAX_MS);
        let days = ms.div_euclid(86_400_000);
        let rem = ms.rem_euclid(86_400_000);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let mut year = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = mp + if mp < 10 { 3 } else { -9 };
        if month <= 2 {
            year += 1;
        }
        format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
            rem / 3_600_000,
            rem / 60_000 % 60,
            rem / 1_000 % 60,
            rem % 1_000
        )
    }
    pub fn saturating_add_ms(self, ms: i64) -> Self {
        Self(self.0.saturating_add(ms))
    }
}
impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}
impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse_rfc3339(&s).map_err(|e| serde::de::Error::custom(e.message))
    }
}
/// Formatea el tiempo transcurrido respecto de la época de misión.
pub fn mission_elapsed(epoch: Timestamp, at: Timestamp) -> String {
    let diff = i128::from(at.0) - i128::from(epoch.0);
    let secs = diff.abs() / 1_000;
    let sign = if diff < 0 { '-' } else { '+' };
    format!(
        "T{sign}{:02}d {:02}:{:02}:{:02}",
        secs / 86_400,
        secs / 3_600 % 24,
        secs / 60 % 60,
        secs % 60
    )
}
