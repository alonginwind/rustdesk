use base64::{engine::general_purpose::STANDARD, Engine};
use wasm_bindgen::prelude::*;

// Schema-free protobuf writers, mirroring the schema-free readers in codec.rs. Keeping both
// directions in wasm makes the module the single source of truth for the wire format: the JS
// bridge only composes fields, so int64 no longer needs the BigInt detour and a 64 KiB payload
// no longer gets spread element by element into a temporary array.

fn write_varint(out: &mut Vec<u8>, mut n: u64) {
    while n > 0x7f {
        out.push(((n as u8) & 0x7f) | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
}

fn write_tag(out: &mut Vec<u8>, field: u32, wire: u8) {
    write_varint(out, ((field as u64) << 3) | wire as u64);
}

// Wire type 2 is shared by string, bytes and embedded messages, so all three build the same
// length-delimited record; the tag plus a five-byte varint bound the header.
fn delimited(field: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 6);
    write_tag(&mut out, field, 2);
    write_varint(&mut out, payload.len() as u64);
    out.extend_from_slice(payload);
    out
}

#[wasm_bindgen(js_name = pbUint32)]
pub fn pb_uint32(field: u32, n: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(6);
    write_tag(&mut out, field, 0);
    write_varint(&mut out, n as u64);
    out
}

/// int64 arrives from JS as a number, which is exact up to 2^53 and covers every timestamp and
/// file size the bridge encodes.
#[wasm_bindgen(js_name = pbInt64)]
pub fn pb_int64(field: u32, n: f64) -> Vec<u8> {
    let mut out = Vec::with_capacity(11);
    write_tag(&mut out, field, 0);
    write_varint(&mut out, n as u64);
    out
}

#[wasm_bindgen(js_name = pbBool)]
pub fn pb_bool(field: u32, v: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(2);
    write_tag(&mut out, field, 0);
    out.push(u8::from(v));
    out
}

#[wasm_bindgen(js_name = pbSint32)]
pub fn pb_sint32(field: u32, n: i32) -> Vec<u8> {
    let mut out = Vec::with_capacity(6);
    write_tag(&mut out, field, 0);
    write_varint(&mut out, (((n as u32) << 1) ^ ((n >> 31) as u32)) as u64);
    out
}

#[wasm_bindgen(js_name = pbString)]
pub fn pb_string(field: u32, s: &str) -> Vec<u8> {
    delimited(field, s.as_bytes())
}

#[wasm_bindgen(js_name = pbBytes)]
pub fn pb_bytes(field: u32, bytes: &[u8]) -> Vec<u8> {
    delimited(field, bytes)
}

#[wasm_bindgen(js_name = pbEmbed)]
pub fn pb_embed(field: u32, inner: &[u8]) -> Vec<u8> {
    delimited(field, inner)
}

// Hex and base64 encoding helpers. These run on the hot path for file transfers and terminal
// data, where the JS byte-loop was the one part of the download path that pinned the main
// thread; the wasm side avoids the intermediate string allocation entirely.

#[wasm_bindgen(js_name = bytesToHex)]
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

#[wasm_bindgen(js_name = hexToBytes)]
pub fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()?;
        out.push(byte);
    }
    Some(out)
}

#[wasm_bindgen(js_name = toBase64)]
pub fn to_base64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

#[wasm_bindgen(js_name = fromBase64)]
pub fn from_base64(s: &str) -> Option<Vec<u8>> {
    STANDARD.decode(s).ok()
}
