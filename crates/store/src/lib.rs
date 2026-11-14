//! # merken-store
//!
//! Persistencia local (SQLite) y bandeja de salida para la sincronización
//! (RF-19). El esquema inicial está en `docs/decisiones/002-base-de-datos.md`.

/// Versión de SQLite incluida en la app. Sirve para comprobar que
/// `rusqlite` con `bundled` compila y enlaza en cada plataforma.
pub fn version_sqlite() -> Result<String, rusqlite::Error> {
    let conn = rusqlite::Connection::open_in_memory()?;
    conn.query_row("SELECT sqlite_version()", [], |fila| fila.get(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_incluido_responde() -> Result<(), rusqlite::Error> {
        let version = version_sqlite()?;
        assert!(version.starts_with('3'), "versión inesperada: {version}");
        Ok(())
    }
}
