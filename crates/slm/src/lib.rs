//! # merken-slm
//!
//! Asistente con modelos pequeños en el dispositivo, detrás de un trait para
//! poder cambiar de motor (llama.cpp, candle, ONNX Runtime):
//! - **interpretar** texto → propuesta de registro con salida restringida por
//!   gramática (capa 2 de RF-14); el usuario la confirma;
//! - **explicar** una evaluación ya decidida por `merken-core` (RF-15).
//!
//! El SLM ayuda, no decide: nunca asigna estados ni recomienda tratamientos.
