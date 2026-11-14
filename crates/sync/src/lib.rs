//! # merken-sync
//!
//! Cliente de la API de sincronización (RF-20 a RF-22): subida idempotente
//! por lotes y descarga de análisis por cursor. El móvil **nunca** habla
//! directamente con MySQL; las credenciales solo las tiene la API.
