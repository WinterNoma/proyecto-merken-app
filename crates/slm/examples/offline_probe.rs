//! Reproducible local native inference; supports private JSONL inputs without logging names.
#[cfg(feature = "native")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{path::PathBuf, sync::atomic::AtomicBool};
    let mut args = std::env::args().skip(1);
    let directory = PathBuf::from(args.next().ok_or("Expected model directory")?);
    let inputs = std::fs::read_to_string(args.next().ok_or("Expected input JSONL")?)?;
    merken_slm::runtime::verify(&directory)?;
    for line in inputs.lines().filter(|s| !s.trim().is_empty()) {
        let input: merken_slm::contract::Input = serde_json::from_str(line)?;
        let (out, raw, elapsed) =
            merken_slm::runtime::generate(&directory, &input, &AtomicBool::new(false))?;
        println!(
            "{}",
            serde_json::json!({"accepted":true,"model_output":out,"raw":raw,"elapsed_ms":elapsed,"source":input.source,"readings":input.readings,"clinical_validation":false})
        );
    }
    Ok(())
}
#[cfg(not(feature = "native"))]
fn main() {
    eprintln!("Enable the native feature to run offline inference");
}
