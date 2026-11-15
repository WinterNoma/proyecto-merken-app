//! Lectura acotada de cabeceras GGUF para validar adaptadores LoRA.
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

const MAX_HEADER: u64 = 64 * 1024 * 1024;
const MAX_STRING: u64 = 1024 * 1024;
const MAX_ITEMS: u64 = 1_000_000;
const MAX_TENSORS: u64 = 100_000;
const MAX_ADAPTER_BYTES: u64 = 256 * 1024 * 1024;
type Result<T> = std::result::Result<T, ()>;

struct Reader<R> {
    input: R,
    left: u64,
}
impl<R: Read> Reader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> Result<()> {
        self.left = self
            .left
            .checked_sub(u64::try_from(bytes.len()).map_err(|_| ())?)
            .ok_or(())?;
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
        if bytes.contains(&0) {
            return Err(());
        }
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
    fn value(&mut self, ty: u32) -> Result<Option<String>> {
        match ty {
            0 | 1 | 7 => self.skip(1)?,
            2 | 3 => self.skip(2)?,
            4..=6 => self.skip(4)?,
            10..=12 => self.skip(8)?,
            8 => return self.string().map(Some),
            9 => {
                let elem = self.u32()?;
                let count = self.u64()?;
                if count > MAX_ITEMS || !matches!(elem, 0..=8 | 10..=12) {
                    return Err(());
                }
                for _ in 0..count {
                    let _ = self.value(elem)?;
                }
            }
            _ => return Err(()),
        }
        Ok(None)
    }
}
#[derive(Clone)]
struct Tensor {
    dims: Vec<u64>,
    ty: u32,
    offset: u64,
}
struct Header {
    tensors: HashMap<String, Tensor>,
    order: Vec<String>,
    architecture: Option<String>,
    general: Option<String>,
    adapter: Option<String>,
    blocks: Option<u32>,
    alignment: u64,
    data_start: u64,
    file_size: u64,
}
fn align(n: u64, alignment: u64) -> Result<u64> {
    Ok(n.checked_add(alignment.checked_sub(1).ok_or(())?)
        .ok_or(())?
        & !(alignment - 1))
}
fn header(path: &Path) -> Result<Header> {
    let file = File::open(path).map_err(|_| ())?;
    let file_size = file.metadata().map_err(|_| ())?.len();
    let mut r = Reader {
        input: BufReader::new(file),
        left: MAX_HEADER,
    };
    let mut magic = [0; 4];
    r.read(&mut magic)?;
    if &magic != b"GGUF" || !matches!(r.u32()?, 2 | 3) {
        return Err(());
    }
    let tensor_count = r.u64()?;
    let keys = r.u64()?;
    if tensor_count > MAX_TENSORS || keys > MAX_ITEMS {
        return Err(());
    }
    let (mut architecture, mut general, mut adapter, mut blocks) = (None, None, None, None);
    let mut alignment = 32_u64;
    let mut seen_keys = HashSet::new();
    for _ in 0..keys {
        let key = r.string()?;
        if key.is_empty() || !seen_keys.insert(key.clone()) {
            return Err(());
        }
        let ty = r.u32()?;
        match key.as_str() {
            "general.architecture" | "general.type" | "adapter.type" => {
                if ty != 8 {
                    return Err(());
                }
                let value = r.value(ty)?;
                match key.as_str() {
                    "general.architecture" => architecture = value,
                    "general.type" => general = value,
                    _ => adapter = value,
                }
            }
            "general.alignment" | "qwen3.block_count" => {
                if ty != 4 {
                    return Err(());
                }
                let value = r.u32()?;
                if key == "general.alignment" {
                    alignment = u64::from(value);
                } else {
                    blocks = Some(value);
                }
            }
            "adapter.lora.alpha" => {
                if ty != 6 {
                    return Err(());
                }
                let _ = r.value(ty)?;
            }
            "adapter.alora.invocation_tokens" => {
                if ty != 9 || r.u32()? != 4 {
                    return Err(());
                }
                let count = r.u64()?;
                if count > MAX_ITEMS {
                    return Err(());
                }
                r.skip(count.checked_mul(4).ok_or(())?)?;
            }
            _ => {
                let _ = r.value(ty)?;
            }
        }
    }
    if !alignment.is_power_of_two() {
        return Err(());
    }
    let mut tensors = HashMap::new();
    let mut order = Vec::new();
    for _ in 0..tensor_count {
        let name = r.string()?;
        if name.is_empty() || name.len() >= 64 || tensors.contains_key(&name) {
            return Err(());
        }
        let count = r.u32()?;
        if !(1..=4).contains(&count) {
            return Err(());
        }
        let mut dims = Vec::new();
        for _ in 0..count {
            let dim = r.u64()?;
            if dim == 0 || dim > i64::MAX as u64 {
                return Err(());
            }
            dims.push(dim);
        }
        let ty = r.u32()?;
        let offset = r.u64()?;
        order.push(name.clone());
        tensors.insert(name, Tensor { dims, ty, offset });
    }
    let data_start = align(MAX_HEADER.checked_sub(r.left).ok_or(())?, alignment)?;
    Ok(Header {
        tensors,
        order,
        architecture,
        general,
        adapter,
        blocks,
        alignment,
        data_start,
        file_size,
    })
}
fn allowed_target(name: &str, blocks: u32) -> bool {
    let Some(rest) = name.strip_prefix("blk.") else {
        return false;
    };
    let Some((number, suffix)) = rest.split_once('.') else {
        return false;
    };
    if number.is_empty()
        || !number.bytes().all(|b| b.is_ascii_digit())
        || (number.len() > 1 && number.starts_with('0'))
    {
        return false;
    }
    let Ok(index) = number.parse::<u32>() else {
        return false;
    };
    index < blocks
        && matches!(
            suffix,
            "attn_q.weight"
                | "attn_k.weight"
                | "attn_v.weight"
                | "attn_output.weight"
                | "ffn_gate.weight"
                | "ffn_up.weight"
                | "ffn_down.weight"
        )
}
fn size(t: &Tensor) -> Result<u64> {
    let width = match t.ty {
        0 => 4_u64,
        1 => 2,
        _ => return Err(()),
    };
    let elements = t
        .dims
        .iter()
        .try_fold(1_u64, |n, dim| n.checked_mul(*dim).ok_or(()))?;
    if elements > i64::MAX as u64 {
        return Err(());
    }
    elements.checked_mul(width).ok_or(())
}
pub(crate) fn validate_adapter(base: &Path, adapter: &Path) -> Result<()> {
    let base = header(base)?;
    let adapter = header(adapter)?;
    if base.architecture.as_deref() != Some("qwen3")
        || adapter.architecture != base.architecture
        || adapter.general.as_deref() != Some("adapter")
        || adapter.adapter.as_deref() != Some("lora")
    {
        return Err(());
    }
    let blocks = base.blocks.filter(|n| *n > 0).ok_or(())?;
    let mut total = 0_u64;
    let mut next_offset = 0_u64;
    let mut a = HashMap::new();
    let mut b = HashMap::new();
    for name in &adapter.order {
        let tensor = adapter.tensors.get(name).ok_or(())?;
        let bytes = size(tensor)?;
        if tensor.offset != next_offset {
            return Err(());
        }
        let end = adapter
            .data_start
            .checked_add(tensor.offset)
            .and_then(|v| v.checked_add(bytes))
            .ok_or(())?;
        if end > adapter.file_size {
            return Err(());
        }
        total = total.checked_add(bytes).ok_or(())?;
        if total > MAX_ADAPTER_BYTES || total > adapter.file_size {
            return Err(());
        }
        next_offset = next_offset
            .checked_add(align(bytes, adapter.alignment)?)
            .ok_or(())?;
        let (stem, map) = if let Some(stem) = name.strip_suffix(".lora_a") {
            (stem, &mut a)
        } else if let Some(stem) = name.strip_suffix(".lora_b") {
            (stem, &mut b)
        } else {
            return Err(());
        };
        if !allowed_target(stem, blocks) || map.insert(stem, tensor).is_some() {
            return Err(());
        }
    }
    if a.is_empty() || a.len() != b.len() {
        return Err(());
    }
    for (stem, ta) in a {
        let tb = b.get(stem).ok_or(())?;
        let target = base.tensors.get(stem).ok_or(())?;
        if target.dims.len() != 2 || ta.dims.len() != 2 || tb.dims.len() != 2 {
            return Err(());
        }
        let input = target.dims.first().ok_or(())?;
        let output = target.dims.get(1).ok_or(())?;
        let rank = ta.dims.get(1).ok_or(())?;
        if !(1..=256).contains(rank)
            || ta.dims.first() != Some(input)
            || tb.dims.first() != Some(rank)
            || tb.dims.get(1) != Some(output)
        {
            return Err(());
        }
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
    fn kv_string(key: &str, value: &str) -> (String, u32, Vec<u8>) {
        let mut v = Vec::new();
        string(&mut v, value);
        (key.into(), 8, v)
    }
    fn kv_u32(key: &str, value: u32) -> (String, u32, Vec<u8>) {
        (key.into(), 4, value.to_le_bytes().to_vec())
    }
    #[derive(Clone)]
    struct Spec {
        name: String,
        dims: Vec<u64>,
        ty: u32,
        offset: u64,
    }
    fn make(kvs: &[(String, u32, Vec<u8>)], tensors: &[Spec], data_len: usize) -> Vec<u8> {
        let mut out = b"GGUF".to_vec();
        out.extend_from_slice(&3_u32.to_le_bytes());
        out.extend_from_slice(&(tensors.len() as u64).to_le_bytes());
        out.extend_from_slice(&(kvs.len() as u64).to_le_bytes());
        for (key, ty, val) in kvs {
            string(&mut out, key);
            out.extend_from_slice(&ty.to_le_bytes());
            out.extend_from_slice(val);
        }
        for t in tensors {
            string(&mut out, &t.name);
            out.extend_from_slice(&(t.dims.len() as u32).to_le_bytes());
            for d in &t.dims {
                out.extend_from_slice(&d.to_le_bytes());
            }
            out.extend_from_slice(&t.ty.to_le_bytes());
            out.extend_from_slice(&t.offset.to_le_bytes());
        }
        out.resize((out.len() + 31) & !31, 0);
        out.resize(out.len() + data_len, 0);
        out
    }
    fn base() -> Vec<u8> {
        make(
            &[
                kv_string("general.architecture", "qwen3"),
                kv_u32("qwen3.block_count", 1),
            ],
            &[Spec {
                name: "blk.0.attn_q.weight".into(),
                dims: vec![4, 8],
                ty: 0,
                offset: 0,
            }],
            0,
        )
    }
    fn metadata() -> Vec<(String, u32, Vec<u8>)> {
        vec![
            kv_string("general.architecture", "qwen3"),
            kv_string("general.type", "adapter"),
            kv_string("adapter.type", "lora"),
        ]
    }
    fn tensors() -> Vec<Spec> {
        vec![
            Spec {
                name: "blk.0.attn_q.weight.lora_a".into(),
                dims: vec![4, 2],
                ty: 1,
                offset: 0,
            },
            Spec {
                name: "blk.0.attn_q.weight.lora_b".into(),
                dims: vec![2, 8],
                ty: 1,
                offset: 32,
            },
        ]
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
    fn valid_and_invalid_adapter_cases() {
        let b = base();
        let kv = metadata();
        let ts = tensors();
        let wrong_base = make(
            &[
                kv_string("general.architecture", "llama"),
                kv_u32("qwen3.block_count", 1),
            ],
            &[],
            0,
        );
        assert!(!check(&wrong_base, &make(&kv, &ts, 64)));
        assert!(check(&b, &make(&kv, &ts, 64)));
        let mut bad = kv.clone();
        bad[0] = kv_string("general.architecture", "llama");
        assert!(!check(&b, &make(&bad, &ts, 64)));
        bad[0] = kv_u32("general.architecture", 1);
        assert!(!check(&b, &make(&bad, &ts, 64)));
        bad = kv.clone();
        bad.push(kv_u32("adapter.lora.alpha", 1));
        assert!(!check(&b, &make(&bad, &ts, 64)));
        bad = kv.clone();
        bad.push(("adapter.lora.alpha".into(), 6, 1_f32.to_le_bytes().to_vec()));
        assert!(check(&b, &make(&bad, &ts, 64)));
        bad = kv.clone();
        bad.push(kv_string("description", "bad\0value"));
        assert!(!check(&b, &make(&bad, &ts, 64)));
        bad = kv.clone();
        bad.push((
            "adapter.alora.invocation_tokens".into(),
            9,
            [
                9_u32.to_le_bytes().as_slice(),
                0_u64.to_le_bytes().as_slice(),
            ]
            .concat(),
        ));
        assert!(!check(&b, &make(&bad, &ts, 64)));
        let mut t = ts.clone();
        for x in &mut t {
            x.name = x.name.replace("attn_q", "attn_norm");
        }
        assert!(!check(&b, &make(&kv, &t, 64)));
        t = ts.clone();
        for x in &mut t {
            x.name = x.name.replace("blk.0", "blk.1");
        }
        assert!(!check(&b, &make(&kv, &t, 64)));
        t = ts.clone();
        t[0].name.push('\0');
        assert!(!check(&b, &make(&kv, &t, 64)));
        for ty in [2, 28] {
            t = ts.clone();
            t[0].ty = ty;
            assert!(!check(&b, &make(&kv, &t, 64)));
        }
        t = ts.clone();
        t[0].dims[1] = u64::MAX;
        assert!(!check(&b, &make(&kv, &t, 0)));
        t = ts.clone();
        t[1].offset = 4096;
        assert!(!check(&b, &make(&kv, &t, 64)));
        t = ts.clone();
        t[1].offset = 0;
        assert!(!check(&b, &make(&kv, &t, 64)));
        t = ts.clone();
        t[1].dims[1] = 7;
        assert!(!check(&b, &make(&kv, &t, 64)));
        assert!(!check(&b, &make(&kv, &[], 0)));
        assert!(!check(&b, &make(&kv, &ts[..1], 64)));
        assert!(!check(&b, &make(&kv, &ts, 10)));
    }
}
