//! DeepSeek custom Keccak hash helpers.

use super::sponge::Sponge;

pub const ALGORITHM: &str = "DeepSeekHashV1";
const PAD_BYTE: u8 = 6;

/// Hash `input` with DeepSeek's custom Keccak sponge (not NIST SHA3-256).
#[must_use]
pub fn deepseek_hash(input: &str) -> String {
    hex::encode(hash_bytes(input.as_bytes()))
}

fn hash_bytes(input: &[u8]) -> [u8; 32] {
    let mut sponge = Sponge::new();
    sponge.absorb(input);
    sponge.squeeze(PAD_BYTE)
}

/// Hash after absorbing `prefix`, then `nonce` (matches JS
/// `copy().update(nonce)`).
#[must_use]
pub fn deepseek_hash_with_prefix(prefix: &str, nonce: u64) -> String {
    let mut sponge = Sponge::new();
    sponge.absorb(prefix.as_bytes());
    let mut trial = sponge.copy();
    trial.absorb(nonce.to_string().as_bytes());
    hex::encode(trial.squeeze(PAD_BYTE))
}

/// Build PoW prefix: `{salt}_{expire_at}_`.
#[must_use]
pub fn pow_prefix(salt: &str, expire_at: i64) -> String {
    format!("{salt}_{expire_at}_")
}

/// Find the first nonce in `0..difficulty` whose hash equals `challenge`.
pub fn find_nonce(
    challenge: &str,
    prefix: &str,
    difficulty: u64,
) -> Option<u64> {
    if difficulty == 0 {
        return None;
    }
    let mut sponge = Sponge::new();
    sponge.absorb(prefix.as_bytes());

    for nonce in 0..difficulty {
        let mut trial = sponge.copy();
        trial.absorb(nonce.to_string().as_bytes());
        if hex::encode(trial.squeeze(PAD_BYTE)) == challenge {
            return Some(nonce);
        }
    }
    None
}
