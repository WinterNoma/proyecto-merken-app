//! Lleva al núcleo las últimas lecturas de un miembro de la tripulación de MySQL.
//!
//! Se guardan como DEMO con `Store::import_synthetic` (igual que la API de
//! simulación): sustituyen la DEMO anterior, nunca se suben a Tierra y no se
//! mezclan con datos reales del dispositivo. La hora de medida es la que da la
//! base de datos de tierra; sin ella no se importa nada (el núcleo no inventa horas).
use merken_core::{catalog::Indicator, time::Timestamp};
use merken_store::StoreError;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
pub struct CrewImportInput {
    pub crew_id: u32,
    pub display_name: String,
    /// Última recepción del miembro en la base de tierra (RFC 3339).
    pub measured_at: Option<String>,
    #[serde(default)]
    pub garmin_linked: bool,
    #[serde(default)]
    pub readings: CrewReadings,
}

#[derive(Debug, Default, Deserialize)]
pub struct CrewReadings {
    pub heart_rate: Option<f64>,
    pub systolic: Option<f64>,
    pub diastolic: Option<f64>,
    pub body_temperature: Option<f64>,
    pub glucose: Option<f64>,
    pub oxygen_in_blood: Option<f64>,
    pub sleep_hours: Option<f64>,
    pub nutrition: Option<f64>,
    pub space_radiation: Option<f64>,
    pub distance_from_earth: Option<f64>,
}

pub fn astronaut_id(crew_id: u32) -> String {
    format!("mysql-crew-{crew_id}")
}

fn invalid(field: &str, message: &str) -> StoreError {
    StoreError {
        code: "invalid_value".into(),
        field: Some(field.into()),
        message: message.into(),
    }
}

/// Registros con la forma de sincronización que acepta `import_synthetic`.
/// Las lecturas fuera de rango no se filtran aquí: el núcleo las rechaza y las cuenta.
pub fn crew_records(input: &CrewImportInput, now: Timestamp) -> Result<Vec<Value>, StoreError> {
    let measured_at = input
        .measured_at
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            invalid(
                "measured_at",
                "This crew member has no measurement time in the ground database; nothing was imported",
            )
        })?;
    let measured_at = Timestamp::parse_rfc3339(measured_at).map_err(|_| {
        invalid(
            "measured_at",
            "The ground database sent an invalid measurement time",
        )
    })?;
    let r = &input.readings;
    let mut values: Vec<(Indicator, Value)> = Vec::new();
    let mut push = |indicator: Indicator, value: Option<Value>| {
        if let Some(value) = value {
            values.push((indicator, value));
        }
    };
    push(
        Indicator::HeartRate,
        r.heart_rate.map(|v| json!({ "bpm": v })),
    );
    push(
        Indicator::BloodPressure,
        r.systolic
            .zip(r.diastolic)
            .map(|(s, d)| json!({ "systolic": s, "diastolic": d })),
    );
    push(
        Indicator::BodyTemperature,
        r.body_temperature.map(|v| json!({ "celsius": v })),
    );
    push(Indicator::Glucose, r.glucose.map(|v| json!({ "mg_dl": v })));
    push(
        Indicator::OxygenInBlood,
        r.oxygen_in_blood.map(|v| json!({ "percentage": v })),
    );
    push(
        Indicator::Sleep,
        r.sleep_hours.map(|v| json!({ "hours": v })),
    );
    push(
        Indicator::Nutrition,
        r.nutrition.map(|v| json!({ "kcal": v })),
    );
    push(
        Indicator::SpaceRadiation,
        r.space_radiation.map(|v| json!({ "usv_h": v })),
    );
    push(
        Indicator::DistanceFromEarth,
        r.distance_from_earth.map(|v| json!({ "km": v })),
    );
    if values.is_empty() {
        return Err(invalid(
            "readings",
            "This crew member has no numeric readings to import",
        ));
    }
    let astronaut = astronaut_id(input.crew_id);
    let source = if input.garmin_linked {
        "garmin"
    } else {
        "earth"
    };
    values
        .into_iter()
        .map(|(indicator, value)| {
            let name = serde_json::to_value(indicator)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .ok_or_else(|| invalid("indicator", "Unknown indicator"))?;
            Ok(json!({
                "record_id": format!("{astronaut}-{name}-{}", measured_at.millis()),
                "astronaut_id": astronaut,
                "indicator": name,
                "value": value,
                "unit": indicator.unit(),
                "measured_at": measured_at.to_rfc3339(),
                "logged_at": now.to_rfc3339(),
                "source": source,
                "interpreted_by": null,
                "replaces": null,
                "external_id": null,
                "note": null,
                "schema_version": 1,
                "app_version": env!("CARGO_PKG_VERSION"),
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(measured_at: Option<&str>) -> CrewImportInput {
        CrewImportInput {
            crew_id: 1,
            display_name: "Test".into(),
            measured_at: measured_at.map(str::to_owned),
            garmin_linked: true,
            readings: CrewReadings {
                heart_rate: Some(93.0),
                systolic: Some(120.0),
                diastolic: Some(80.0),
                sleep_hours: Some(3.4),
                ..Default::default()
            },
        }
    }

    #[test]
    fn builds_one_record_per_reading_with_the_ground_time() {
        let now =
            Timestamp::parse_rfc3339("2026-11-15T14:00:00Z").unwrap_or(Timestamp::from_millis(0));
        let records = crew_records(&input(Some("2026-11-15T13:53:30.993Z")), now);
        let records = records.unwrap_or_default();
        assert_eq!(records.len(), 3);
        for r in &records {
            assert_eq!(r["astronaut_id"], "mysql-crew-1");
            assert_eq!(r["measured_at"], "2026-11-15T13:53:30.993Z");
            assert_eq!(r["source"], "garmin");
            let dto = serde_json::from_value::<merken_core::record::RecordDto>(r.clone());
            assert!(dto.is_ok(), "{r}");
        }
        assert_eq!(
            records[1]["value"],
            json!({ "systolic": 120.0, "diastolic": 80.0 })
        );
        assert_eq!(records[2]["unit"], "h");
    }

    #[test]
    fn refuses_members_without_a_measurement_time_or_readings() {
        let now = Timestamp::from_millis(0);
        assert!(crew_records(&input(None), now).is_err());
        assert!(crew_records(&input(Some("  ")), now).is_err());
        assert!(crew_records(&input(Some("yesterday")), now).is_err());
        let mut empty = input(Some("2026-11-15T13:53:30Z"));
        empty.readings = CrewReadings::default();
        assert!(crew_records(&empty, now).is_err());
    }
}
