//! Lectura acotada de cabeceras GGUF para validar adaptadores LoRA.
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

const MAX_HEADER: u64 = 64 * 1024 * 1024;
const MAX_STRING: u64 = 1024 * 1024;
const MAX_ITEMS: u64 = 1_000_000;
const MAX_TENSORS: u64 = 100_000;
const MAX_DEPTH: u8 = 4;

type Result<T> = std::result::Result<T, ()>;

struct Reader<R> {
    input: R,
    left: u64,
}
impl<R: Read> Reader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> Result<()> {
        let n = u64::try_from(bytes.len()).map_err(|_| ())?;
        self.left = self.left.checked_sub(n).ok_or(())?;
        self.input.read_exact(bytes).map_err(|_| ())
    }
    fn u32(&mut self) -> Result<u32> {
        let mut b = [0; 4];
        self.read(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }
    fn u64(&mut self) -> Result<u64> {
        let mut b = [0; 8];
        self.read(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }
    fn string(&mut self) -> Result<String> {
        let len = self.u64()?;
        if len > MAX_STRING {
            return Err(());
        }
        let mut bytes = vec![0; usize::try_from(len).map_err(|_| ())?];
        self.read(&mut bytes)?;
        String::from_utf8(bytes).map_err(|_| ())
    }
    fn skip(&mut self, mut n: u64) -> Result<()> {
        let mut buf = [0; 4096];
        while n > 0 {
            let take = usize::try_from(n.min(4096)).map_err(|_| ())?;
            self.read(buf.get_mut(..take).ok_or(())?)?;
            n -= u64::try_from(take).map_err(|_| ())?;
        }
        Ok(())
    }
    fn value(&mut self, ty: u32, depth: u8) -> Result<Option<String>> {
        match ty {
            0 | 1 | 7 => self.skip(1)?,
            2 | 3 => self.skip(2)?,
            4..=6 => self.skip(4)?,
            10..=12 => self.skip(8)?,
            8 => return self.string().map(Some),
            9 => {
                if depth >= MAX_DEPTH {
                    return Err(());
                }
                let elem = self.u32()?;
                let count = self.u64()?;
                if count > MAX_ITEMS {
                    return Err(());
                }
                for _ in 0..count {
                    let _ = self.value(elem, depth + 1)?;
                }
            }
            _ => return Err(()),
        }
        Ok(None)
    }
}
struct Header {
    tensors: HashSet<String>,
    general: Option<String>,
    adapter: Option<String>,
}
fn header(path: &Path) -> Result<Header> {
    let mut r = Reader {
        input: BufReader::new(File::open(path).map_err(|_| ())?),
        left: MAX_HEADER,
    };
    let mut magic = [0; 4];
    r.read(&mut magic)?;
    if &magic != b"GGUF" || !matches!(r.u32()?, 2 | 3) {
        return Err(());
    }
    let tensors = r.u64()?;
    let keys = r.u64()?;
    if tensors > MAX_TENSORS || keys > MAX_ITEMS {
        return Err(());
    }
    let mut general = None;
    let mut adapter = None;
    let mut seen_keys = HashSet::new();
    for _ in 0..keys {
        let key = r.string()?;
        if !seen_keys.insert(key.clone()) {
            return Err(());
        }
        let ty = r.u32()?;
        let value = r.value(ty, 0)?;
        match key.as_str() {
            "general.type" => general = value,
            "adapter.type" => adapter = value,
            _ => {}
        }
    }
    let mut names = HashSet::new();
    for _ in 0..tensors {
        let name = r.string()?;
        if name.is_empty() || !names.insert(name) {
            return Err(());
        }
        let dims = r.u32()?;
        if !(1..=4).contains(&dims) {
            return Err(());
        }
        for _ in 0..dims {
            if r.u64()? == 0 {
                return Err(());
            }
        }
        r.u32()?; // ggml type
        r.u64()?; // offset, relativo al comienzo de los datos
    }
    Ok(Header {
        tensors: names,
        general,
        adapter,
    })
}

pub(crate) fn validate_adapter(base: &Path, adapter: &Path) -> Result<()> {
    let base = header(base)?;
    let adapter = header(adapter)?;
    if adapter.general.as_deref() != Some("adapter") || adapter.adapter.as_deref() != Some("lora") {
        return Err(());
    }
    let mut a = HashSet::new();
    let mut b = HashSet::new();
    for name in &adapter.tensors {
        if let Some(stem) = name.strip_suffix(".lora_a") {
            a.insert(stem);
        } else if let Some(stem) = name.strip_suffix(".lora_b") {
            b.insert(stem);
        }
    }
    if a.is_empty()
        || a != b
        || a.iter().any(|name| {
            name.is_empty()
                || name.contains(".lora_a")
                || name.contains(".lora_b")
                || !base.tensors.contains(*name)
        })
    {
        return Err(());
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn string(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(&(s.len() as u64).to_le_bytes());
        out.extend_from_slice(s.as_bytes());
    }
    fn make(kind: &str, subtype: &str, tensors: &[&str]) -> Vec<u8> {
        let mut out = b"GGUF".to_vec();
        out.extend_from_slice(&3_u32.to_le_bytes());
        out.extend_from_slice(&(tensors.len() as u64).to_le_bytes());
        out.extend_from_slice(&2_u64.to_le_bytes());
        for (key, value) in [("general.type", kind), ("adapter.type", subtype)] {
            string(&mut out, key);
            out.extend_from_slice(&8_u32.to_le_bytes());
            string(&mut out, value);
        }
        for name in tensors {
            string(&mut out, name);
            out.extend_from_slice(&1_u32.to_le_bytes());
            out.extend_from_slice(&1_u64.to_le_bytes());
            out.extend_from_slice(&0_u32.to_le_bytes());
            out.extend_from_slice(&0_u64.to_le_bytes());
        }
        out
    }
    fn check(base: &[u8], adapter: &[u8]) -> bool {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("merken-gguf-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let b = dir.join("base");
        let a = dir.join("adapter");
        std::fs::write(&b, base).unwrap();
        std::fs::write(&a, adapter).unwrap();
        let result = validate_adapter(&b, &a).is_ok();
        std::fs::remove_dir_all(dir).unwrap();
        result
    }
    #[test]
    fn adapter_requires_complete_applicable_pair() {
        let base = make("model", "", &["layer.weight"]);
        let good = make(
            "adapter",
            "lora",
            &["layer.weight.lora_a", "layer.weight.lora_b"],
        );
        assert!(check(&base, &good));
        assert!(!check(&base, &make("adapter", "lora", &[])));
        assert!(!check(
            &base,
            &make("adapter", "lora", &["layer.weight.lora_a"])
        ));
        assert!(!check(
            &base,
            &make(
                "model",
                "lora",
                &["layer.weight.lora_a", "layer.weight.lora_b"]
            )
        ));
        assert!(!check(
            &base,
            &make(
                "adapter",
                "other",
                &["layer.weight.lora_a", "layer.weight.lora_b"]
            )
        ));
        assert!(!check(
            &base,
            &make("adapter", "lora", &["missing.lora_a", "missing.lora_b"])
        ));
        assert!(!check(&base, &good[..good.len() - 1]));
        let mut absurd = good.clone();
        absurd[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(!check(&base, &absurd));
    }
}
