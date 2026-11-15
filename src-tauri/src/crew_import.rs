//! Lleva al núcleo el historial de un miembro de la tripulación que ya está
//! guardado en este dispositivo (`telemetry_readings` de SQLite).
//!
//! MySQL hace de API de tierra: el feed guarda cada lectura en SQLite con la hora
//! en que el dispositivo la recibió (`observed_at`). Aquí no se consulta MySQL:
//! se leen esas lecturas locales y se cargan como DEMO con
//! `Store::import_synthetic` (sustituyen la DEMO anterior, nunca se suben a
//! Tierra y no se mezclan con datos reales). La hora de cada registro es la
//! `observed_at` guardada; el núcleo no inventa horas.
use merken_core::{catalog::Indicator, time::Timestamp};
use merken_store::{ListTelemetryInput, Store, StoreError, TelemetryItem};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
pub struct CrewImportInput {
    /// Clave del miembro en la telemetría local (el ID de MySQL).
    pub astronaut_key: String,
    pub display_name: String,
}

/// Indicadores del feed que el núcleo evalúa (el ánimo llega como etiqueta de
/// texto y no es una puntuación; HRV y body battery no están en el núcleo).
const CORE_INDICATORS: [Indicator; 9] = [
    Indicator::HeartRate,
    Indicator::BloodPressure,
    Indicator::BodyTemperature,
    Indicator::Glucose,
    Indicator::OxygenInBlood,
    Indicator::Sleep,
    Indicator::Nutrition,
    Indicator::SpaceRadiation,
    Indicator::DistanceFromEarth,
];
/// El feed guarda una lectura por minuto aunque no cambie: se queda la última
/// de cada tramo de 10 minutos para que el historial no sea solo repeticiones.
const BUCKET_MS: i64 = 10 * 60_000;
/// Lecturas por indicador que se pasan al núcleo (las más recientes).
const PER_INDICATOR: usize = 48;
/// Lecturas por indicador que se piden a SQLite (máximo de `list_telemetry`).
const LIST_LIMIT: u32 = 1000;

pub fn astronaut_id(astronaut_key: &str) -> String {
    format!("local-crew-{}", astronaut_key.trim())
}

fn invalid(field: &str, message: &str) -> StoreError {
    StoreError {
        code: "invalid_value".into(),
        field: Some(field.into()),
        message: message.into(),
    }
}

fn indicator_name(indicator: Indicator) -> Option<String> {
    serde_json::to_value(indicator)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
}

/// De lecturas en cualquier orden, la última de cada tramo, de la más antigua
/// a la más reciente, como mucho `PER_INDICATOR`.
fn thin(mut items: Vec<TelemetryItem>) -> Vec<TelemetryItem> {
    items.sort_by_key(|item| (item.observed_at.millis(), item.reading_id));
    let mut kept: Vec<TelemetryItem> = Vec::new();
    for item in items {
        let bucket = item.observed_at.millis().div_euclid(BUCKET_MS);
        match kept.last_mut() {
            Some(last) if last.observed_at.millis().div_euclid(BUCKET_MS) == bucket => *last = item,
            _ => kept.push(item),
        }
    }
    let skip = kept.len().saturating_sub(PER_INDICATOR);
    kept.into_iter().skip(skip).collect()
}

/// Registros con la forma de sincronización que acepta `import_synthetic`,
/// sacados del historial local. Los valores que el núcleo no acepte se
/// rechazan y se cuentan en `import_synthetic`.
pub fn local_records(
    store: &Store,
    input: &CrewImportInput,
    now: Timestamp,
) -> Result<Vec<Value>, StoreError> {
    let key = input.astronaut_key.trim();
    let astronaut = astronaut_id(key);
    let mut records = Vec::new();
    for indicator in CORE_INDICATORS {
        let name =
            indicator_name(indicator).ok_or_else(|| invalid("indicator", "Unknown indicator"))?;
        let items = store.list_telemetry(ListTelemetryInput {
            astronaut_key: key.to_owned(),
            indicator: Some(name.clone()),
            from: None,
            to: None,
            limit: Some(LIST_LIMIT),
        })?;
        for item in thin(items) {
            let measured_at = item.observed_at;
            records.push(json!({
                "record_id": format!("{astronaut}-{name}-{}", measured_at.millis()),
                "astronaut_id": astronaut,
                "indicator": name,
                "value": item.value,
                "unit": indicator.unit(),
                "measured_at": measured_at.to_rfc3339(),
                "logged_at": now.to_rfc3339(),
                "source": "earth",
                "interpreted_by": null,
                "replaces": null,
                "external_id": null,
                "note": null,
                "schema_version": 1,
                "app_version": env!("CARGO_PKG_VERSION"),
            }));
        }
    }
    if records.is_empty() {
        return Err(invalid(
            "astronaut_key",
            "No readings for this crew member are saved on this device yet",
        ));
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merken_store::{RecordTelemetryInput, TelemetryReadingInput};

    fn reading(indicator: &str, value: Value, unit: &str) -> TelemetryReadingInput {
        TelemetryReadingInput {
            astronaut_key: "2".into(),
            astronaut_name: "Alex Morgan".into(),
            indicator: indicator.into(),
            value,
            unit: unit.into(),
        }
    }

    #[test]
    fn imports_the_local_history_with_its_saved_times() -> Result<(), StoreError> {
        let mut store = Store::open_in_memory()?;
        let start =
            Timestamp::parse_rfc3339("2026-11-15T12:00:00Z").map_err(|_| invalid("t", "t"))?;
        // Una lectura cada 5 minutos durante una hora: 13 lecturas, 7 tramos de 10 min.
        for step in 0..13_i64 {
            let at = start.saturating_add_ms(step * 5 * 60_000);
            store.record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("heart_rate", json!({ "bpm": 70 + step }), "bpm"),
                        reading("mood", json!({ "label": "CALM" }), ""),
                    ],
                },
                at,
                "test",
            )?;
        }
        let input = CrewImportInput {
            astronaut_key: "2".into(),
            display_name: "Alex Morgan".into(),
        };
        let now = start.saturating_add_ms(3_600_000);
        let records = local_records(&store, &input, now)?;
        assert_eq!(
            records.len(),
            7,
            "one heart-rate reading per 10-minute slot, no mood"
        );
        assert!(records.iter().all(|r| r["indicator"] == "heart_rate"));
        assert_eq!(records[0]["measured_at"], "2026-11-15T12:05:00.000Z");
        assert_eq!(records[6]["value"], json!({ "bpm": 82 }));
        let imported = store.import_synthetic(&astronaut_id("2"), "Alex Morgan", &records, now)?;
        assert_eq!((imported.imported, imported.rejected), (7, 0));
        Ok(())
    }

    #[test]
    fn refuses_a_member_with_no_local_readings() -> Result<(), StoreError> {
        let store = Store::open_in_memory()?;
        let input = CrewImportInput {
            astronaut_key: "9".into(),
            display_name: "Nobody".into(),
        };
        assert!(local_records(&store, &input, Timestamp::from_millis(0)).is_err());
        Ok(())
    }
}
