use crypto_box::{aead::Aead, PublicKey, SalsaBox, SecretKey};
use crypto_secretbox::{KeyInit, XSalsa20Poly1305};
use ed25519_dalek::{Signature, VerifyingKey};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

const KEY_LEN: usize = 32;

/// Both NaCl box and secretbox use a 24-byte nonce. The native peer seals the symmetric key
/// with an all-zero nonce of exactly this length, so the length is enforced here once instead
/// of at every JS call site, where a wrong constant only shows up as a handshake timeout.
const NONCE_LEN: usize = 24;

const SIGNATURE_LEN: usize = 64;

// generic-array's from_slice panics on a length mismatch, and a wasm panic takes the whole
// module down, so every length is checked before a slice is handed to it.
fn nonce_of(bytes: &[u8]) -> Option<&crypto_box::Nonce> {
    (bytes.len() == NONCE_LEN).then(|| crypto_box::Nonce::from_slice(bytes))
}

fn secretbox_of(key: &[u8]) -> Option<XSalsa20Poly1305> {
    (key.len() == KEY_LEN).then(|| XSalsa20Poly1305::new(crypto_secretbox::Key::from_slice(key)))
}

fn box_of(their_pk: &[u8], our_sk: &[u8]) -> Option<SalsaBox> {
    let pk = PublicKey::from_slice(their_pk).ok()?;
    let sk = SecretKey::from_slice(our_sk).ok()?;
    Some(SalsaBox::new(&pk, &sk))
}

/// Verifies an Ed25519 `signature || message` blob, the layout the peer uses for SignedId.
/// Returns the message on success.
#[wasm_bindgen(js_name = signOpen)]
pub fn sign_open(signed: &[u8], pk: &[u8]) -> Option<Vec<u8>> {
    let (sig, msg) = signed.split_at_checked(SIGNATURE_LEN)?;
    let key = VerifyingKey::from_bytes(pk.try_into().ok()?).ok()?;
    key.verify_strict(msg, &Signature::from_slice(sig).ok()?)
        .ok()?;
    Some(msg.to_vec())
}

/// Generates an X25519 keypair and returns `public_key || secret_key`, 32 bytes each.
#[wasm_bindgen(js_name = boxKeypair)]
pub fn box_keypair() -> Vec<u8> {
    let sk = SecretKey::generate(&mut OsRng);
    let mut out = Vec::with_capacity(KEY_LEN * 2);
    out.extend_from_slice(sk.public_key().as_bytes());
    out.extend_from_slice(&sk.to_bytes());
    out
}

/// NaCl crypto_box seal, used to hand the symmetric key to the peer.
#[wasm_bindgen(js_name = boxSeal)]
pub fn box_seal(msg: &[u8], nonce: &[u8], their_pk: &[u8], our_sk: &[u8]) -> Option<Vec<u8>> {
    box_of(their_pk, our_sk)?.encrypt(nonce_of(nonce)?, msg).ok()
}

/// NaCl secretbox seal, used for every message after the symmetric key is agreed.
#[wasm_bindgen(js_name = secretboxSeal)]
pub fn secretbox_seal(msg: &[u8], nonce: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    secretbox_of(key)?.encrypt(nonce_of(nonce)?, msg).ok()
}

/// NaCl secretbox open. `None` means authentication failed, so the caller must not treat
/// the sequence number as consumed.
#[wasm_bindgen(js_name = secretboxOpen)]
pub fn secretbox_open(cipher: &[u8], nonce: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    secretbox_of(key)?.decrypt(nonce_of(nonce)?, cipher).ok()
}

/// The login handshake hashes `sha256(password || salt)` and then `sha256(hash || challenge)`,
/// so both rounds take and return raw bytes.
#[wasm_bindgen]
pub fn sha256(data: &[u8]) -> Vec<u8> {
    Sha256::digest(data).to_vec()
}

/// Login password hash: `sha256(sha256(password || salt) || challenge)`. The two-step digest
/// is what the peer compares against its own derivation, so both sides must agree byte for
/// byte. Keeping the concatenation in wasm avoids three intermediate Uint8Arrays on the JS
/// side.
#[wasm_bindgen(js_name = hashPassword)]
pub fn hash_password(password: &str, salt: &str, challenge: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt.as_bytes());
    let first = hasher.finalize();
    let mut hasher = Sha256::new();
    hasher.update(&first);
    hasher.update(challenge.as_bytes());
    hasher.finalize().to_vec()
}

/// Builds the 24-byte nonce the framing protocol uses: the sequence number in little-endian
/// in the first 8 bytes, the remaining 16 zero. Mirroring the layout here keeps the JS side
/// from having to allocate a DataView and two setUint32 calls per frame.
fn seq_nonce(seq: u64) -> crypto_box::Nonce {
    let mut nonce = crypto_box::Nonce::default();
    nonce[..8].copy_from_slice(&seq.to_le_bytes());
    nonce
}

/// Secretbox seal driven by a sequence number instead of an explicit nonce. The JS framing
/// loop increments a counter and passes it here; the nonce is the counter in little-endian
/// padded to 24 bytes, matching hbb_common's Encrypt.
#[wasm_bindgen(js_name = secretboxSealSeq)]
pub fn secretbox_seal_seq(msg: &[u8], seq: f64, key: &[u8]) -> Option<Vec<u8>> {
    let nonce = seq_nonce(seq as u64);
    secretbox_of(key)?.encrypt(&nonce, msg).ok()
}

/// Secretbox open driven by a sequence number. `None` means authentication failed, so the
/// caller must not advance the counter.
#[wasm_bindgen(js_name = secretboxOpenSeq)]
pub fn secretbox_open_seq(cipher: &[u8], seq: f64, key: &[u8]) -> Option<Vec<u8>> {
    let nonce = seq_nonce(seq as u64);
    secretbox_of(key)?.decrypt(&nonce, cipher).ok()
}
