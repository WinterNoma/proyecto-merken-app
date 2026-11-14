//! Cliente de la API de simulación (`/v1/astronauts`, `/v1/astronauts/{id}/records`).
//!
//! Solo datos inventados: lo que se descarga se guarda en el móvil como DEMO
//! (nunca se sube a Tierra). El token nunca va en el código: lo pasa quien
//! llama (variable de entorno `MERKEN_SIM_TOKEN` o un campo de la interfaz).
//!
//! **Logs:** nunca activar el nivel `TRACE` para `ureq` ni `ureq_proto` en
//! ninguna configuración de logging: vuelcan las cabeceras enviadas, incluida
//! `Authorization`, sin ocultarlas (revisión de Sol, M3).

use crate::SyncError;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

/// Dirección por defecto de la API de simulación.
pub const DEFAULT_BASE_URL: &str = "http://98.95.196.77:8001";
/// Tamaño de página (la API admite 1–100).
pub const PAGE_LIMIT: u32 = 100;
/// Tope de registros por importación. Si la API tiene más, la importación
/// falla (sin tocar la DEMO anterior) en vez de cargar una parte.
pub const MAX_RECORDS: usize = 20_000;
/// Tope de bytes por respuesta (una página de 100 registros ocupa ~50 KB).
pub const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
/// Tope de bytes acumulados en una importación.
pub const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SimAstronaut {
    pub astronaut_id: String,
    pub display_name: String,
}

/// IDs de la API: UUID canónico (minúsculas con guiones). Se comprueba antes
/// de meterlo en la ruta.
pub fn valid_astronaut_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|u| u.hyphenated().to_string() == id)
}

fn bad(msg: &str) -> SyncError {
    SyncError::new("bad_response", msg)
}

fn too_much() -> SyncError {
    SyncError::new(
        "too_much_data",
        "The simulation returned too much data; import fewer days",
    )
}

/// `GET /v1/astronauts` → lista de astronautas simulados.
pub fn parse_astronauts(body: &Value) -> Result<Vec<SimAstronaut>, SyncError> {
    let list = body
        .get("astronauts")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("Missing astronauts list"))?;
    list.iter()
        .map(|a| {
            let id = a.get("astronaut_id").and_then(Value::as_str);
            let name = a.get("display_name").and_then(Value::as_str);
            match (id, name) {
                (Some(id), Some(name)) if valid_astronaut_id(id) => Ok(SimAstronaut {
                    astronaut_id: id.to_owned(),
                    display_name: name.to_owned(),
                }),
                _ => Err(bad("Invalid astronaut entry")),
            }
        })
        .collect()
}

/// Una página de `/records`: los registros tal cual y el cursor siguiente
/// (`None` si no hay más). El cursor es un desplazamiento: si hay más páginas,
/// debe ser exactamente `cursor + registros de esta página` (revisión de Sol, I1).
pub fn parse_records_page(
    body: &Value,
    cursor: u64,
) -> Result<(Vec<Value>, Option<u64>), SyncError> {
    let records = body
        .get("records")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("Missing records list"))?;
    if records.len() > PAGE_LIMIT as usize {
        return Err(bad("Page is larger than requested"));
    }
    let has_more = body
        .get("has_more")
        .and_then(Value::as_bool)
        .ok_or_else(|| bad("Missing has_more"))?;
    if !has_more {
        return Ok((records.clone(), None));
    }
    let expected = u64::try_from(records.len())
        .ok()
        .filter(|&n| n > 0)
        .and_then(|n| cursor.checked_add(n))
        .ok_or_else(|| bad("Pagination cursor does not advance"))?;
    match body.get("next_cursor").and_then(Value::as_u64) {
        Some(n) if n == expected => Ok((records.clone(), Some(n))),
        _ => Err(bad("Pagination cursor does not match the page")),
    }
}

/// Traduce un estado HTTP que no es 200 a un error para la interfaz.
pub fn status_error(status: u16) -> SyncError {
    match status {
        401 | 403 => SyncError::new("unauthorized", "The simulation token was rejected"),
        404 => SyncError::new("not_found", "Astronaut not found in the simulation"),
        422 => SyncError::new("invalid_value", "The simulation API rejected the request"),
        s => SyncError::new("http_error", format!("Simulation API answered HTTP {s}")),
    }
}

/// Error de transporte con un mensaje fijo: el texto de ureq puede incluir
/// contenido remoto (p. ej. una cabecera `Location`), así que no se reenvía
/// a la interfaz (revisión de Sol, I4).
fn transport_error(e: &ureq::Error) -> SyncError {
    match e {
        ureq::Error::Timeout(_) => {
            SyncError::new("network", "The simulation API did not answer in time")
        }
        ureq::Error::BodyExceedsLimit(_) => too_much(),
        _ => SyncError::new("network", "Cannot reach the simulation API"),
    }
}

/// Cliente bloqueante: llamarlo fuera del hilo de la interfaz.
pub struct SimClient {
    base_url: String,
    token: String,
    agent: ureq::Agent,
}

impl SimClient {
    pub fn new(base_url: &str, token: &str) -> Result<Self, SyncError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(SyncError::new(
                "missing_token",
                "Set MERKEN_SIM_TOKEN or pass the simulation token",
            ));
        }
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .http_status_as_error(false)
            // La API no redirige; seguir una redirección podría llevar el token a otro sitio.
            .max_redirects(0)
            .build();
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            token: token.to_owned(),
            agent: config.into(),
        })
    }

    /// Hace un GET y devuelve el JSON y los bytes leídos.
    fn get(&self, path: &str, query: &[(&str, String)]) -> Result<(Value, usize), SyncError> {
        let mut req = self
            .agent
            .get(format!("{}{path}", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token));
        for (k, v) in query {
            req = req.query(*k, v);
        }
        let mut resp = req.call().map_err(|e| transport_error(&e))?;
        let status = resp.status().as_u16();
        if status != 200 {
            return Err(status_error(status));
        }
        let text = resp
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_string()
            .map_err(|e| match e {
                ureq::Error::BodyExceedsLimit(_) => too_much(),
                _ => bad("The simulation API did not return text"),
            })?;
        let json = serde_json::from_str::<Value>(&text)
            .map_err(|_| bad("The simulation API did not return JSON"))?;
        Ok((json, text.len()))
    }

    /// `GET /v1/astronauts`.
    pub fn list_astronauts(&self) -> Result<Vec<SimAstronaut>, SyncError> {
        parse_astronauts(&self.get("/v1/astronauts", &[])?.0)
    }

    /// Todos los registros de un astronauta desde `from` (RFC 3339), paginando.
    /// Si se superan [`MAX_RECORDS`] o [`MAX_TOTAL_BYTES`] devuelve
    /// `too_much_data`: nunca una descarga a medias.
    pub fn records_since(&self, astronaut_id: &str, from: &str) -> Result<Vec<Value>, SyncError> {
        if !valid_astronaut_id(astronaut_id) {
            return Err(SyncError::new("invalid_value", "Invalid astronaut_id"));
        }
        let path = format!("/v1/astronauts/{astronaut_id}/records");
        let mut records = Vec::new();
        let mut bytes = 0_usize;
        let mut cursor = 0_u64;
        loop {
            let (body, size) = self.get(
                &path,
                &[
                    ("from", from.to_owned()),
                    ("limit", PAGE_LIMIT.to_string()),
                    ("cursor", cursor.to_string()),
                ],
            )?;
            bytes = bytes.saturating_add(size);
            let (page, next) = parse_records_page(&body, cursor)?;
            if bytes > MAX_TOTAL_BYTES || records.len().saturating_add(page.len()) > MAX_RECORDS {
                return Err(too_much());
            }
            records.extend(page);
            match next {
                Some(n) => cursor = n,
                None => return Ok(records),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
    use super::*;
    use serde_json::json;

    #[test]
    fn ids_are_checked_before_building_the_path() {
        assert!(valid_astronaut_id("a9c7929e-f810-5b0f-ae07-7aeb92e4252c"));
        for bad in [
            "",
            "a",
            "---",
            "../health",
            "A9C7929E-F810-5B0F-AE07-7AEB92E4252C",
            "a9c7929ef8105b0fae077aeb92e4252c",
            "id?x=1",
        ] {
            assert!(!valid_astronaut_id(bad), "{bad}");
        }
    }

    #[test]
    fn astronauts_are_parsed_and_bad_entries_rejected() {
        let ok = json!({"simulation": true, "astronauts": [
            {"astronaut_id": "a9c7929e-f810-5b0f-ae07-7aeb92e4252c", "display_name": "Alex Morgan"}]});
        let list = parse_astronauts(&ok).unwrap();
        assert_eq!(list[0].display_name, "Alex Morgan");
        let bad_id = json!({"astronauts": [{"astronaut_id": "../x", "display_name": "X"}]});
        assert_eq!(parse_astronauts(&bad_id).unwrap_err().code, "bad_response");
        assert!(parse_astronauts(&json!({})).is_err());
    }

    #[test]
    fn pages_follow_the_offset_cursor_and_stop() {
        let mid = json!({"records": [{"a": 1}, {"a": 2}], "next_cursor": 2, "has_more": true});
        assert_eq!(
            parse_records_page(&mid, 0).unwrap(),
            (vec![json!({"a": 1}), json!({"a": 2})], Some(2))
        );
        let last = json!({"records": [{"a": 3}], "next_cursor": null, "has_more": false});
        assert_eq!(
            parse_records_page(&last, 2).unwrap(),
            (vec![json!({"a": 3})], None)
        );
        let bad_pages = [
            // Sin has_more, o de otro tipo: no se toma como "ya no hay más".
            json!({"records": [{"a": 1}], "next_cursor": 1}),
            json!({"records": [{"a": 1}], "next_cursor": 1, "has_more": "false"}),
            json!({"records": [{"a": 1}], "next_cursor": 1, "has_more": null}),
            // Página vacía que dice que hay más.
            json!({"records": [], "next_cursor": 2, "has_more": true}),
            // El cursor salta registros o no avanza.
            json!({"records": [{"a": 1}], "next_cursor": 5, "has_more": true}),
            json!({"records": [{"a": 1}], "next_cursor": 0, "has_more": true}),
            json!({"records": [{"a": 1}], "has_more": true}),
            json!({"next_cursor": 1, "has_more": false}),
        ];
        for page in bad_pages {
            assert_eq!(
                parse_records_page(&page, 0).unwrap_err().code,
                "bad_response",
                "{page}"
            );
        }
        let huge = json!({"records": vec![json!({}); 101], "has_more": false});
        assert!(parse_records_page(&huge, 0).is_err());
    }

    /// Servidor HTTP de una sola respuesta para probar el cliente sin red.
    fn one_shot_server(response: String) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut buf = [0_u8; 4096];
                let _ = s.read(&mut buf);
                let _ = s.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn redirects_are_not_followed_and_errors_do_not_echo_remote_text() {
        let marker = "SECRET-MARKER-123";
        let base = one_shot_server(format!(
            "HTTP/1.1 302 Found
Location: http://[{marker}
Content-Length: 0

"
        ));
        let err = SimClient::new(&base, marker)
            .unwrap()
            .list_astronauts()
            .unwrap_err();
        assert_eq!(err.code, "http_error");
        assert!(!err.message.contains(marker));
        let base = one_shot_server(format!(
            "HTTP/1.1 200 OK
Content-Length: 0
{marker}

"
        ));
        let err = SimClient::new(&base, "t")
            .unwrap()
            .list_astronauts()
            .unwrap_err();
        assert!(!err.message.contains(marker), "{err}");
    }

    #[test]
    fn oversized_responses_are_rejected() {
        let body = "x".repeat(MAX_RESPONSE_BYTES as usize + 1);
        let base = one_shot_server(format!(
            "HTTP/1.1 200 OK
Content-Length: {}

{body}",
            body.len()
        ));
        let err = SimClient::new(&base, "t")
            .unwrap()
            .list_astronauts()
            .unwrap_err();
        assert_eq!(err.code, "too_much_data");
    }

    #[test]
    fn http_status_maps_to_codes_and_token_is_required() {
        assert_eq!(status_error(401).code, "unauthorized");
        assert_eq!(status_error(404).code, "not_found");
        assert_eq!(status_error(500).code, "http_error");
        assert_eq!(
            SimClient::new(DEFAULT_BASE_URL, "  ").err().unwrap().code,
            "missing_token"
        );
    }
}
