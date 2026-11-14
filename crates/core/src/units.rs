//! Unidades con invariantes de validación.
use crate::{ErrorCode, ValidationError};

macro_rules! positive_unit {
    ($name:ident, $field:literal) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $name(f64);
        impl $name {
            /// Devuelve el valor validado.
            pub const fn value(self) -> f64 {
                self.0
            }
        }
        impl TryFrom<f64> for $name {
            type Error = ValidationError;
            fn try_from(value: f64) -> Result<Self, Self::Error> {
                if value.is_finite() && value > 0.0 {
                    Ok(Self(value))
                } else {
                    Err(ValidationError {
                        code: ErrorCode::InvalidValue,
                        field: $field.to_owned(),
                        message: "Value must be finite and greater than zero".to_owned(),
                    })
                }
            }
        }
    };
}
positive_unit!(Bpm, "bpm");
positive_unit!(MmHg, "mmHg");
positive_unit!(Celsius, "celsius");
positive_unit!(MgDl, "mg_dl");

/// Duración de sueño en horas, entre cero y 24.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hours(f64);
impl Hours {
    pub const fn value(self) -> f64 {
        self.0
    }
}
impl TryFrom<f64> for Hours {
    type Error = ValidationError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if value.is_finite() && (0.0..=24.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError {
                code: ErrorCode::InvalidValue,
                field: "hours".to_owned(),
                message: "Hours must be between 0 and 24".to_owned(),
            })
        }
    }
}

/// Puntuación entera entre cero y diez.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Score(u8);
impl Score {
    pub const fn value(self) -> u8 {
        self.0
    }
}
impl TryFrom<u8> for Score {
    type Error = ValidationError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value <= 10 {
            Ok(Self(value))
        } else {
            Err(ValidationError {
                code: ErrorCode::InvalidValue,
                field: "score".to_owned(),
                message: "Score must be between 0 and 10".to_owned(),
            })
        }
    }
}
