# pulsys-voice

Offline English TTS with the sherpa-onnx Kokoro v1.0 int8 package. Default builds do not require the native library; enable the tts feature to load it. Only this package layout is supported:

~~~text
kokoro-int8-multi-lang-v1_0/
  model.int8.onnx
  voices.bin
  tokens.txt
  lexicon-us-en.txt
  pulsys-us-en.txt
~~~

Copy `lexicon/pulsys-us-en.txt` from this crate into the model directory. Both lexicons are required; a comma in either lexicon path causes a load error because sherpa-onnx uses commas to separate lexicons.

The default voice is af_heart. Voice IDs follow the English v1.0 table in the sherpa-onnx documentation; confirm the mapping by listening to the actual package before production use.

Text is limited to 600 input characters. The normalizer expands integers from 0 to 999,999, negatives, decimal digits, grouped thousands, 24-hour times, percentages and a closed set of health units and abbreviations. For example, 37.2 °C becomes “thirty seven point two degrees Celsius”, and mg/dL becomes “milligrams a deciliter”. It does not infer units or expand arbitrary medical abbreviations. Malformed numbers return text_invalid. The expansions themselves must occur in the lexicon.

Each lexicon is loaded with a 16 MiB limit. Before any audio is created, normalized words are checked against the union of the US English and Pulsys lexicons; both are passed to sherpa-onnx. Unknown uppercase acronyms of at most five ASCII letters are spelled out only when all their individual letters occur in the lexicon; all other unknown tokens return text_unsupported. TtsEngine::unsupported_words reports distinct missing tokens in first-occurrence order so callers can check authorized text in advance. The check uses whitespace and common punctuation as boundaries; verify its behavior against the actual sherpa-onnx tokenizer and package lexicon when validating the native integration.

synthesize_streaming validates the entire input first, then calls the callback with each completed sentence or bounded part. Returning false stops further generation and returns cancelled; it cannot interrupt the native call already in progress. The callback is synchronous: applications can queue each chunk for playback while the caller continues synthesis. The return value combines all chunks. The 180-character target is a heuristic, not a phoneme limit; an individual long word is kept intact.

Measurements on a 12-core PC with v1.0 int8 and af_heart: first chunk 1.8–2.0 s; RTF 1.43 with two threads and 1.29 with four. Play completed sentences as they arrive. On mobile, playback will have pauses between sentences while RTF remains above 1; measure on the target device. Kitten TTS, also supported by sherpa-onnx, is a pending alternative to evaluate for faster but less natural speech.

Set SHERPA_ONNX_LIB_DIR to the native libraries directory. On Windows make the runtime DLLs available to the executable; on Android package the matching .so files in jniLibs/<abi>.

~~~powershell
$env:SHERPA_ONNX_LIB_DIR = "C:\path\to\sherpa\lib"
cargo run -p pulsys-voice --features tts --example say -- <model-dir> out.wav af_heart 1.0 2 "Hello astronaut."
cargo run -p pulsys-voice --features tts --example say -- --check <model-dir> "Heart rate: 95 bpm."
~~~

The positional voice, speed and thread count are optional, in that order. The example prints time to first chunk, audio duration, total synthesis time and RTF. --check loads the model but generates no audio.

Licenses: Kokoro Apache-2.0, sherpa-onnx Apache-2.0, ONNX Runtime MIT. Check the notices included with the specific binaries and model package for attribution and distribution requirements.

To check normalization vocabulary against the installed lexicons, set `PULSYS_KOKORO_DIR` to the model directory and run `cargo test -p pulsys-voice installed_lexicons_cover_normalizer_vocabulary -- --ignored`.

## Offline speech recognition (STT)

Enable the `stt` feature to transcribe locally with sherpa-onnx. Whisper is the
project default; base and small packages use `base-*` or `small-*` filenames,
with matching `<name>-encoder.int8.onnx`, `<name>-decoder.int8.onnx` and
`<name>-tokens.txt`. An unquantized `-encoder.onnx` / `-decoder.onnx` pair is
also supported. A complete int8 pair takes precedence. Whisper base fails on
Spanish; small improves Spanish recognition.

Canary is a secondary option using `encoder.int8.onnx`, `decoder.int8.onnx`
and `tokens.txt`. It transcribes Spanish well but writes numbers as words,
while the core only understands digits. Canary's `auto` language defaults to
Spanish because Canary does not detect the language. Use one complete model
per directory: ambiguous directories are rejected.

The engine accepts mono normalized f32 audio from 8,000 to 48,000 Hz, between
0.1 and 30 seconds, and returns trimmed text without saving or postprocessing
it. The WAV example truncates recordings over 30 seconds:

```text
cargo run -p pulsys-voice --features stt --example transcribe -- <model_dir> <mono.wav> [es|en|auto]
```

Stable errors include `model_missing`, `load_failed`, `unavailable`,
`audio_invalid`, and `transcription_failed`. Changing the requested language
recreates the recognizer.
