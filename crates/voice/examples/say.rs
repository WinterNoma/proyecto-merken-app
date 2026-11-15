use merken_voice::{KokoroPaths, TtsConfig, TtsEngine, Voice};
use std::error::Error;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--check") {
        if args.len() != 3 {
            return Err("Usage: say --check <model-dir> \"text\"".into());
        }
        let model = args.get(1).ok_or("Missing model directory")?;
        let text = args.get(2).ok_or("Missing text")?;
        let engine = TtsEngine::load(KokoroPaths::from_dir(model)?, TtsConfig::default())?;
        let missing = engine.unsupported_words(text)?;
        if missing.is_empty() {
            println!("No unsupported words");
        } else {
            for word in missing {
                println!("{word}");
            }
        }
        return Ok(());
    }
    if !(3..=6).contains(&args.len()) {
        return Err("Usage: say <model-dir> <out.wav> [voice] [speed] [threads] \"text\"".into());
    }
    let model = args.first().ok_or("Missing model directory")?;
    let output = args.get(1).ok_or("Missing output path")?;
    let text = args.last().ok_or("Missing text")?;
    let mut config = TtsConfig::default();
    if args.len() >= 4 {
        config.voice = args.get(2).ok_or("Missing voice")?.parse::<Voice>()?;
    }
    if args.len() >= 5 {
        config.speed = args
            .get(3)
            .ok_or("Missing speed")?
            .parse()
            .map_err(|_| "Invalid speed")?;
    }
    if args.len() == 6 {
        config.threads = args
            .get(4)
            .ok_or("Missing threads")?
            .parse()
            .map_err(|_| "Invalid threads")?;
    }
    let mut engine = TtsEngine::load(KokoroPaths::from_dir(model)?, config)?;
    let start = Instant::now();
    let mut first = None;
    let pcm = engine.synthesize_streaming(text, |_| {
        if first.is_none() {
            first = Some(start.elapsed().as_secs_f64());
        }
        true
    })?;
    let elapsed = start.elapsed().as_secs_f64();
    let duration = pcm.samples.len() as f64 / f64::from(pcm.sample_rate);
    std::fs::write(output, pcm.to_wav_bytes()?)?;
    println!(
        "First chunk: {:.2}s; audio: {duration:.2}s; synthesis: {elapsed:.2}s; RTF: {:.3}",
        first.ok_or("No audio")?,
        elapsed / duration
    );
    Ok(())
}
