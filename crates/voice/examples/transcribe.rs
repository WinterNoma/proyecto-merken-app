use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::time::Instant;

use pulsys_voice::{SttConfig, SttEngine, SttLanguage, SttPaths};

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

// Wave does not expose channels, so inspect the RIFF fmt chunk first.
fn require_mono(path: &str) -> Result<(), Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut header = [0_u8; 12];
    file.read_exact(&mut header)?;
    if header.get(..4) != Some(b"RIFF".as_slice()) || header.get(8..12) != Some(b"WAVE".as_slice())
    {
        return Err(invalid("expected a RIFF/WAVE file").into());
    }
    loop {
        let mut chunk = [0_u8; 8];
        file.read_exact(&mut chunk)
            .map_err(|_| invalid("WAV fmt chunk missing"))?;
        let size_bytes: [u8; 4] = chunk
            .get(4..8)
            .ok_or_else(|| invalid("invalid WAV chunk"))?
            .try_into()?;
        let size = u32::from_le_bytes(size_bytes);
        if chunk.get(..4) == Some(b"fmt ".as_slice()) {
            if size < 16 {
                return Err(invalid("invalid WAV fmt chunk").into());
            }
            let mut format = [0_u8; 4];
            file.read_exact(&mut format)?;
            let channels = u16::from_le_bytes(
                format
                    .get(2..4)
                    .ok_or_else(|| invalid("invalid WAV fmt chunk"))?
                    .try_into()?,
            );
            if channels != 1 {
                return Err(invalid("WAV must be mono").into());
            }
            return Ok(());
        }
        file.seek(SeekFrom::Current(i64::from(size) + i64::from(size % 2)))?;
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .ok_or_else(|| invalid("usage: transcribe <dir_modelo> <wav> [es|en|auto]"))?;
    let wav = args
        .next()
        .ok_or_else(|| invalid("usage: transcribe <dir_modelo> <wav> [es|en|auto]"))?;
    let language = args
        .next()
        .as_deref()
        .unwrap_or("auto")
        .parse::<SttLanguage>()?;
    if args.next().is_some() {
        return Err(invalid("too many arguments").into());
    }
    if wav.contains('\0') {
        return Err(invalid("invalid WAV path").into());
    }
    require_mono(&wav)?;
    let wave = sherpa_onnx::Wave::read(&wav).ok_or_else(|| invalid("could not read mono WAV"))?;
    let rate = wave.sample_rate();
    let max_samples = usize::try_from(rate)
        .ok()
        .and_then(|r| r.checked_mul(30))
        .ok_or_else(|| invalid("invalid sample rate"))?;
    let mut samples = wave.samples();
    if samples.len() > max_samples {
        eprintln!("Aviso: audio recortado a 30 segundos");
        samples = samples
            .get(..max_samples)
            .ok_or_else(|| invalid("invalid audio length"))?;
    }
    let audio_s = samples.len() as f64 / f64::from(rate);
    let start = Instant::now();
    let paths = SttPaths::from_dir(dir)?;
    let mut engine = SttEngine::load(
        paths,
        SttConfig {
            language,
            ..SttConfig::default()
        },
    )?;
    let load_ms = start.elapsed().as_millis();
    let start = Instant::now();
    let text = engine.transcribe(samples, rate, None)?;
    let transcribe_ms = start.elapsed().as_millis();
    println!("{text}\nload_ms={load_ms} transcribe_ms={transcribe_ms} audio_s={audio_s:.3}");
    Ok(())
}
