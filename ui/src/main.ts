import { invoke } from "@tauri-apps/api/core";

// La interfaz solo dibuja: pide los datos ya evaluados al núcleo en Rust.
window.addEventListener("DOMContentLoaded", async () => {
  const el = document.querySelector("#estado-nucleo");
  if (!el) return;
  try {
    el.textContent = await invoke<string>("estado_nucleo");
  } catch (err) {
    el.textContent = `Error del núcleo: ${String(err)}`;
  }
});
