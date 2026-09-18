use ruzstd::StreamingDecoder;
use std::io::Read;
use wasm_bindgen::prelude::*;

/// Same ceiling as the native client's decompression limit, so a crafted frame cannot make
/// the tab allocate without bound.
const MAX_DECOMPRESSED: usize = 256 * 1024 * 1024;

const CHUNK: usize = 16 * 1024;

/// Decodes a zstd frame. Returns `None` if the input is not a valid frame or if the decoded
/// size would exceed `MAX_DECOMPRESSED`.
#[wasm_bindgen(js_name = zstdDecompress)]
pub fn zstd_decompress(data: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = StreamingDecoder::new(data).ok()?;
    let mut out = Vec::new();
    let mut buf = [0u8; CHUNK];
    loop {
        match decoder.read(&mut buf) {
            Ok(0) => return Some(out),
            Ok(n) => {
                if out.len() + n > MAX_DECOMPRESSED {
                    return None;
                }
                out.extend_from_slice(&buf[..n]);
            }
            Err(_) => return None,
        }
    }
}
