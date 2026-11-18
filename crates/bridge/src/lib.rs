pub mod api;
mod dto;
mod durable;
mod worker;

// FRB expands the public API before the generated module exists.
#[cfg(not(frb_expand))]
#[allow(unsafe_code)] // FRB emits the native exports and pointer handling in this module.
#[allow(clippy::unwrap_used, clippy::panic)] // FRB emits infallible buffer writes and codec dispatch.
#[allow(clippy::excessive_nesting)] // FRB 2.11.1 emits nested codec dispatch.
#[rustfmt::skip] // flutter_rust_bridge_codegen owns this file; CI regenerates it and diffs it.
mod frb_generated;
