use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;

use super::hash::{self, ALGORITHM};
use crate::{Error, api::pow_challenge::PowChallenge};

pub fn solve_challenge(challenge: &PowChallenge) -> Result<String, Error> {
    if challenge.algorithm != ALGORITHM {
        return Err(Error::Other(format!(
            "unsupported PoW algorithm: {}",
            challenge.algorithm
        )));
    }
    let prefix = hash::pow_prefix(&challenge.salt, challenge.expire_at);
    let answer = hash::find_nonce(
        &challenge.challenge,
        &prefix,
        u64::from(challenge.difficulty),
    )
    .ok_or_else(|| Error::Other("PoW solver failed".into()))?;
    let payload = json!({
        "algorithm": challenge.algorithm,
        "challenge": challenge.challenge,
        "salt": challenge.salt,
        "answer": answer,
        "signature": challenge.signature,
        "target_path": challenge.target_path,
    });
    Ok(STANDARD.encode(payload.to_string()))
}
