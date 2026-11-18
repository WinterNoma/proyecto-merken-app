# Manual offline recommendations

The additive Recommendations panel uses native Rust/llama.cpp with the evaluated
Qwen3 1.7B Q4_K_M base and V7 F16 adapter at scale one. No inference server, PC,
Nova endpoint or cloud fallback is used by the app. The existing design is retained.

The trusted workflow-dispatch APK bundles both GGUF files. Native Android code
copies them from APK assets into private app storage on the first request, checks
SHA-256, and does not load a partial model. PR check APKs omit model files and are
explicitly labeled as compile checks. Space is needed for the APK and installed
model files. No personal records or secrets are embedded in build artifacts.

Use the local profile, or expand Synchronize backend data and sign in to the
existing HTTPS ground-station origin. Credentials are not persisted. The app
saves the raw backend snapshot and existing alert context privately. It can then
generate while disconnected. Received/capture time is never used as measurement
time. The current backend replay supports Garmin-linked records and the four
fields shared with V7: pulse, sleep, HRV, Body Battery. Unknown-provenance records
are disabled rather than silently called simulated. The local profile supports
canonical core indicators and keeps manual/Garmin origins separate.

Backend snapshots retain conservative server flags but do not claim an independent
onboard evaluation of another crew member. Profile status comes from pulsys-core.
Urgent actions bypass the language model; review flags remain in the packet.
Exercise readiness is unknown until a trusted reviewed mission-plan integration
exists. Mission exercise equipment is shown only as contextual information.
All output is experimental and not clinically validated.

The Rust contract is checked against thirty frozen synthetic policy vectors.
Raw model output must exactly match its permitted card, provenance and caveat;
no template repairs a failed model response. Original readings are attached by
native code. Inference runs on a blocking worker with cancellation and a deadline,
leaving core alerts independent. Physical Android RAM/latency/thermal/airplane-mode
validation still must be performed on the target handset.
