use js_sys::Date;
use std::collections::HashMap;
use std::ptr::addr_of_mut;
use wasm_bindgen::prelude::*;

// CRC-32/ISO-HDLC (reflected, poly 0xEDB88320), the checksum a zip store entry carries. The
// digest runs over every byte of a folder download, so on a large transfer the JS byte loop was
// the one part of the download path that pinned the main thread; the folded table is built once
// at compile time and the update is a tight wasm loop instead.

const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0usize;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

/// Folds one more chunk into a running CRC. Pass the previous value back in, starting from
/// `0xFFFFFFFF`, and finalise with `crc32Finish`.
#[wasm_bindgen(js_name = crc32Update)]
pub fn crc32_update(crc: u32, data: &[u8]) -> u32 {
    let mut c = crc;
    for &b in data {
        c = CRC_TABLE[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    c
}

#[wasm_bindgen(js_name = crc32Finish)]
pub fn crc32_finish(crc: u32) -> u32 {
    crc ^ 0xffff_ffff
}

// Store-mode zip builder. A folder download accumulates FileTransferBlock chunks for several
// files in parallel; once the transfer is done the JS side assembles them into a zip. The
// original JS implementation pushed bytes one at a time into small arrays and then spread-
// merged hundreds of Uint8Arrays, which on a large folder pinned the main thread for seconds.
// The wasm builder writes directly into a growing Vec and keeps a single central-directory
// buffer, so the final blob is one contiguous copy.

struct ZipEntry {
    name: Vec<u8>,
    crc: u32,
    size: u32,
    dos_time: u16,
    dos_date: u16,
    is_dir: bool,
    data: Vec<u8>,
}

struct ZipBuilder {
    entries: Vec<ZipEntry>,
    offset: u32,
}

static mut BUILDERS: Option<HashMap<u32, ZipBuilder>> = None;
static mut NEXT_BUILDER_ID: u32 = 1;

// SAFETY: wasm32-unknown-unknown is single-threaded; these are only touched from the main
// JS thread through wasm_bindgen exports.
fn builders_mut() -> &'static mut HashMap<u32, ZipBuilder> {
    unsafe { &mut *addr_of_mut!(BUILDERS) }
        .get_or_insert_with(HashMap::new)
}

fn dos_time(unix_sec: f64) -> (u16, u16) {
    let millis = JsValue::from_f64(unix_sec * 1000.0);
    let d = Date::new(&millis);
    let year = d.get_full_year();
    if year < 1980 {
        return (0, 0x21);
    }
    let time = ((d.get_hours() << 11) | (d.get_minutes() << 5) | (d.get_seconds() >> 1)) as u16;
    let date = (((year - 1980) << 9) | ((d.get_month() + 1) << 5) | d.get_date()) as u16;
    (time, date)
}

fn push_u16(out: &mut Vec<u8>, v: u16) {
    out.push(v as u8);
    out.push((v >> 8) as u8);
}

fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.push(v as u8);
    out.push((v >> 8) as u8);
    out.push((v >> 16) as u8);
    out.push((v >> 24) as u8);
}

#[wasm_bindgen(js_name = zipCreate)]
pub fn zip_create() -> u32 {
    let builders = builders_mut();
    let id = unsafe { &mut *addr_of_mut!(NEXT_BUILDER_ID) };
    let this_id = *id;
    *id += 1;
    builders.insert(
        this_id,
        ZipBuilder {
            entries: Vec::new(),
            offset: 0,
        },
    );
    this_id
}

#[wasm_bindgen(js_name = zipAddEntry)]
pub fn zip_add_entry(
    id: u32,
    name: &str,
    crc: u32,
    size: u32,
    mtime: f64,
    is_dir: bool,
    chunks: &JsValue,
) -> bool {
    let builders = unsafe { &mut *addr_of_mut!(BUILDERS) };
    let Some(ref mut map) = *builders else {
        return false;
    };
    let Some(builder) = map.get_mut(&id) else {
        return false;
    };
    let (dos_time, dos_date) = dos_time(mtime);
    let name_bytes = name.as_bytes().to_vec();
    let mut data = Vec::new();
    if let Some(arr) = chunks.dyn_ref::<js_sys::Array>() {
        for i in 0..arr.length() {
            if let Some(chunk) = arr.get(i).dyn_ref::<js_sys::Uint8Array>() {
                data.extend_from_slice(&chunk.to_vec());
            }
        }
    }
    builder.offset += 30 + name_bytes.len() as u32 + size;
    builder.entries.push(ZipEntry {
        name: name_bytes,
        crc,
        size,
        dos_time,
        dos_date,
        is_dir,
        data,
    });
    true
}

#[wasm_bindgen(js_name = zipFinish)]
pub fn zip_finish(id: u32) -> Option<Vec<u8>> {
    let builders = unsafe { &mut *addr_of_mut!(BUILDERS) };
    let map = builders.as_mut()?;
    let builder = map.remove(&id)?;
    let mut parts: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    let mut offset: u32 = 0;
    for entry in &builder.entries {
        let mut lh = Vec::with_capacity(30 + entry.name.len());
        push_u32(&mut lh, 0x04034b50);
        push_u16(&mut lh, 20);
        push_u16(&mut lh, 0x0800);
        push_u16(&mut lh, 0);
        push_u16(&mut lh, entry.dos_time);
        push_u16(&mut lh, entry.dos_date);
        push_u32(&mut lh, entry.crc);
        push_u32(&mut lh, entry.size);
        push_u32(&mut lh, entry.size);
        push_u16(&mut lh, entry.name.len() as u16);
        push_u16(&mut lh, 0);
        parts.extend_from_slice(&lh);
        parts.extend_from_slice(&entry.name);
        parts.extend_from_slice(&entry.data);
        let mut cd = Vec::with_capacity(46 + entry.name.len());
        push_u32(&mut cd, 0x02014b50);
        push_u16(&mut cd, 20);
        push_u16(&mut cd, 20);
        push_u16(&mut cd, 0x0800);
        push_u16(&mut cd, 0);
        push_u16(&mut cd, entry.dos_time);
        push_u16(&mut cd, entry.dos_date);
        push_u32(&mut cd, entry.crc);
        push_u32(&mut cd, entry.size);
        push_u32(&mut cd, entry.size);
        push_u16(&mut cd, entry.name.len() as u16);
        push_u16(&mut cd, 0);
        push_u16(&mut cd, 0);
        push_u16(&mut cd, 0);
        push_u16(&mut cd, 0);
        push_u32(&mut cd, if entry.is_dir { 0x10 } else { 0 });
        push_u32(&mut cd, offset);
        central.extend_from_slice(&cd);
        central.extend_from_slice(&entry.name);
        offset += lh.len() as u32 + entry.size + entry.name.len() as u32;
    }
    parts.extend_from_slice(&central);
    let cd_size = central.len() as u32;
    let entry_count = builder.entries.len() as u16;
    let mut eocd = Vec::with_capacity(22);
    push_u32(&mut eocd, 0x06054b50);
    push_u16(&mut eocd, 0);
    push_u16(&mut eocd, 0);
    push_u16(&mut eocd, entry_count);
    push_u16(&mut eocd, entry_count);
    push_u32(&mut eocd, cd_size);
    push_u32(&mut eocd, offset);
    push_u16(&mut eocd, 0);
    parts.extend_from_slice(&eocd);
    Some(parts)
}
