// src/api/lava.rs

use hmac::{Hmac, Mac};
use sha2::Sha256;

/// HMAC-SHA256 в hex.
/// Используется как заглушка под подпись Lava, пока не уточним точный алгоритм.
pub fn sign_hmac_sha256_hex(secret: &str, data: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    let result = mac.finalize().into_bytes();
    hex::encode(result)
}
