use js_sys::{Array, Object, Reflect, Uint8Array};
use std::collections::HashSet;
use wasm_bindgen::prelude::*;

/// A varint is at most 10 bytes; a longer run means the input is corrupt, not huge.
const MAX_VARINT_BYTES: usize = 10;

fn read_varint(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut val: u64 = 0;
    let mut shift = 0u32;
    for _ in 0..MAX_VARINT_BYTES {
        let b = *data.get(*pos)?;
        *pos += 1;
        val |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some(val);
        }
        shift += 7;
    }
    None
}

/// Scans one level of protobuf wire format without a schema, which is what the JS bridge
/// does for `RendezvousMessage`, relay `Message` and every nested sub-message alike.
///
/// Returns a plain object keyed by field number: varints as numbers, and wire types 1/2/5
/// as `Uint8Array`. A repeated field keeps its first occurrence so existing call sites that
/// index a single value keep working. `None` means the input was truncated or corrupt, and
/// the caller falls back to the JS scanner.
///
/// Deliberately schema-free: pulling in rust-protobuf to get field names would compile the
/// 28k generated lines of message.proto into the module for roughly a megabyte of wasm, and
/// the bridge never needs the names.
#[wasm_bindgen(js_name = parseFields)]
pub fn parse_fields(data: &[u8]) -> Option<Object> {
    let out = Object::new();
    let mut seen = HashSet::new();
    let mut pos = 0usize;
    while pos < data.len() {
        let tag = read_varint(data, &mut pos)?;
        let field = tag >> 3;
        if field == 0 {
            return None;
        }
        let value = match tag & 7 {
            0 => JsValue::from_f64(read_varint(data, &mut pos)? as f64),
            1 => {
                let raw = data.get(pos..pos + 8)?;
                pos += 8;
                Uint8Array::from(raw).into()
            }
            2 => {
                let len = read_varint(data, &mut pos)? as usize;
                let end = pos.checked_add(len)?;
                let raw = data.get(pos..end)?;
                pos = end;
                Uint8Array::from(raw).into()
            }
            5 => {
                let raw = data.get(pos..pos + 4)?;
                pos += 4;
                Uint8Array::from(raw).into()
            }
            // Unknown wire type: stop, exactly like the JS scanner does.
            _ => break,
        };
        if !seen.insert(field) {
            continue;
        }
        let key = JsValue::from_f64(field as f64);
        if !Reflect::set(&out, &key, &value).unwrap_or(false) {
            return None;
        }
    }
    Some(out)
}

/// Collects every occurrence of one field number, in wire order, which is how a repeated
/// message, bytes or string field arrives. `parseFields` keeps only the first occurrence, so
/// a repeated field has to come through here instead.
#[wasm_bindgen(js_name = parseRepeated)]
pub fn parse_repeated(data: &[u8], field: u32) -> Option<Array> {
    let out = Array::new();
    let mut pos = 0usize;
    while pos < data.len() {
        let tag = read_varint(data, &mut pos)?;
        let number = tag >> 3;
        if number == 0 {
            return None;
        }
        match tag & 7 {
            // Not the field being collected, so its payload only has to be stepped over.
            0 => {
                read_varint(data, &mut pos)?;
            }
            1 => {
                if data.len() - pos < 8 {
                    return None;
                }
                pos += 8;
            }
            5 => {
                if data.len() - pos < 4 {
                    return None;
                }
                pos += 4;
            }
            2 => {
                let len = read_varint(data, &mut pos)? as usize;
                let end = pos.checked_add(len)?;
                let raw = data.get(pos..end)?;
                pos = end;
                if number == field as u64 {
                    out.push(&Uint8Array::from(raw));
                }
            }
            _ => break,
        }
    }
    Some(out)
}
