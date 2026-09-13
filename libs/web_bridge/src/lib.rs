mod codec;
mod compress;
mod crypto;

use wasm_bindgen::prelude::*;

/// Lets the JS side check that the module it loaded was built from this crate.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
