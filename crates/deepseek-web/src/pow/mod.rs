//! DeepSeekHashV1 proof-of-work solver.

mod hash;
mod keccak_f;
mod solve;
mod sponge;

pub use hash::{
    ALGORITHM, deepseek_hash, deepseek_hash_with_prefix, pow_prefix,
};
pub use solve::solve_challenge;

pub use crate::api::pow_challenge::PowChallenge;

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    use super::*;

    const EXPECTED_42: &str =
        "d1052c4a04fb634e3ac66d36bfeaa583d769839823812090d679b23de6048d6d";

    #[test]
    fn known_prefix_nonce_hash() {
        let prefix = "abc_1234567890_";
        assert_eq!(deepseek_hash_with_prefix(prefix, 42), EXPECTED_42);
        assert_eq!(deepseek_hash(&format!("{prefix}42")), EXPECTED_42);
    }

    #[test]
    fn solve_finds_matching_nonce() {
        let challenge = PowChallenge {
            algorithm: ALGORITHM.into(),
            challenge: EXPECTED_42.into(),
            salt: "abc".into(),
            signature: "sig".into(),
            difficulty: 1000,
            expire_at: 1_234_567_890,
            expire_after: 0,
            target_path: "/api/v0/chat/completion".into(),
        };
        let encoded = solve_challenge(&challenge).expect("solution");
        let decoded = STANDARD.decode(encoded).expect("base64");
        let json: serde_json::Value =
            serde_json::from_slice(&decoded).expect("json");
        assert_eq!(json["answer"], 42);
    }

    #[test]
    fn differs_from_nist_sha3_256() {
        let input = "abc_1234567890_42";
        let ours = deepseek_hash(input);
        let nist = {
            use sha3::{Digest, Sha3_256};
            hex::encode(Sha3_256::digest(input.as_bytes()))
        };
        assert_ne!(ours, nist);
        assert_eq!(
            nist,
            "69a035906b32ac3478d76144d55b29c88ae4a569781ddec86166049c8bb159b2"
        );
    }
}
