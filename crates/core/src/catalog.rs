//! Catálogo de indicadores y zonas corporales.
use crate::{ErrorCode, ValidationError};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

macro_rules! catalog_enum {
    (@count $($variant:ident),+) => { <[()]>::len(&[$(catalog_enum!(@unit $variant)),+]) };
    (@unit $variant:ident) => { () };
    ($name:ident, $field:literal, { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
        impl $name {
            pub const ALL: [Self; catalog_enum!(@count $($variant),+)] = [$(Self::$variant),+];
            pub const fn as_str(self) -> &'static str { match self { $(Self::$variant => $text),+ } }
            pub fn parse(s: &str) -> Result<Self, ValidationError> { s.parse() }
        }
        impl FromStr for $name {
            type Err = ValidationError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s { $($text => Ok(Self::$variant),)+ _ => Err(ValidationError { code: ErrorCode::InvalidValue, field: $field.to_owned(), message: "Unknown catalog value".to_owned() }) }
            }
        }
    };
}

catalog_enum!(Indicator, "indicator", { HeartRate => "heart_rate", BloodPressure => "blood_pressure", BodyTemperature => "body_temperature", Glucose => "glucose", Sleep => "sleep", Mood => "mood", Headache => "headache", Symptom => "symptom" });

impl Indicator {
    /// Devuelve la unidad exigida por el contrato.
    pub const fn unit(self) -> &'static str {
        match self {
            Self::HeartRate => "bpm",
            Self::BloodPressure => "mmHg",
            Self::BodyTemperature => "°C",
            Self::Glucose => "mg/dL",
            Self::Sleep => "h",
            Self::Mood | Self::Headache | Self::Symptom => "score_0_10",
        }
    }
}

catalog_enum!(BodyZone, "body_zone", { Head => "head", Neck => "neck", Chest => "chest", Abdomen => "abdomen", Back => "back", LeftArm => "left_arm", RightArm => "right_arm", LeftHand => "left_hand", RightHand => "right_hand", LeftLeg => "left_leg", RightLeg => "right_leg", LeftFoot => "left_foot", RightFoot => "right_foot", Other => "other" });
catalog_enum!(SleepQuality, "quality", { Poor => "poor", Fair => "fair", Good => "good" });
