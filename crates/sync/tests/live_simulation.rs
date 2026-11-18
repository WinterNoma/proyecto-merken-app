//! Pruebas contra la API de simulación real. Necesitan red, así que están
//! marcadas `#[ignore]`: `cargo test -p pulsys-sync -- --ignored`.
//! La prueba completa solo corre si existe `PULSYS_SIM_TOKEN`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use pulsys_sync::simulation::{DEFAULT_BASE_URL, SimClient};

#[test]
#[ignore = "necesita red"]
fn wrong_token_is_reported_as_unauthorized() {
    let client = SimClient::new(DEFAULT_BASE_URL, "not-a-valid-token").unwrap();
    assert_eq!(client.list_astronauts().unwrap_err().code, "unauthorized");
}

#[test]
#[ignore = "necesita red y PULSYS_SIM_TOKEN"]
fn downloads_two_days_of_one_astronaut() {
    let Ok(token) = std::env::var("PULSYS_SIM_TOKEN") else {
        eprintln!("PULSYS_SIM_TOKEN no está definido: se omite");
        return;
    };
    let client = SimClient::new(DEFAULT_BASE_URL, &token).unwrap();
    let astronauts = client.list_astronauts().unwrap();
    assert!(!astronauts.is_empty());
    let got = client
        .records_since(&astronauts[0].astronaut_id, "2026-11-12T00:00:00Z")
        .unwrap();
    assert!(got.len() > 100, "{}", got.len());
    // Rango de fechas recibido, para comparar con el reloj del móvil.
    let dates: Vec<&str> = got
        .iter()
        .filter_map(|r| r.get("measured_at").and_then(|v| v.as_str()))
        .collect();
    eprintln!(
        "{} registros, de {:?} a {:?}",
        got.len(),
        dates.iter().min(),
        dates.iter().max()
    );
}
