import { invoke } from "@tauri-apps/api/core";

// La interfaz solo dibuja: pide los datos ya evaluados al núcleo en Rust.
window.addEventListener("DOMContentLoaded", async () => {
  const el = document.querySelector("#core-status");
  if (!el) return;
  try {
    el.textContent = await invoke<string>("core_status");
  } catch (err) {
    el.textContent = `Core error: ${String(err)}`;
  }
});
