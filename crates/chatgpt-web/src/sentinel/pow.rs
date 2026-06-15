use base64::{Engine, engine::general_purpose::STANDARD};
use sha3::{Digest, Sha3_512};

pub struct PowOptions {
    pub config: Vec<serde_json::Value>,
    pub seed: String,
    pub target: String,
    pub prefix: String,
    pub max_iter: u32,
}

pub fn solve_pow(opts: PowOptions) -> String {
    let mut cfg = opts.config.clone();
    for i in 0..opts.max_iter {
        cfg[3] = serde_json::json!(i);
        let json = serde_json::to_string(&cfg).unwrap_or_default();
        let b64 = STANDARD.encode(json.as_bytes());
        let mut hasher = Sha3_512::new();
        hasher.update(format!("{}{}", opts.seed, b64).as_bytes());
        let hash: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if !opts.target.is_empty()
            && hash.len() >= opts.target.len()
            && &hash[..opts.target.len()] <= opts.target.as_str()
        {
            return format!("{}{}", opts.prefix, b64);
        }
    }
    let b64 = STANDARD
        .encode(serde_json::to_string(&cfg).unwrap_or_default().as_bytes());
    format!("{}{}", opts.prefix, b64)
}

pub fn build_prepare_token(config: Vec<serde_json::Value>) -> String {
    solve_pow(PowOptions {
        config,
        seed: String::new(),
        target: "0fffff".into(),
        prefix: "gAAAAAC".into(),
        max_iter: 100_000,
    })
}

pub fn solve_proof_of_work(
    seed: &str,
    difficulty: &str,
    config: Vec<serde_json::Value>,
) -> String {
    solve_pow(PowOptions {
        config,
        seed: seed.to_string(),
        target: difficulty.to_ascii_lowercase(),
        prefix: "gAAAAAB".into(),
        max_iter: 500_000,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_prefix() {
        let cfg = vec![serde_json::json!(1); 18];
        let token = build_prepare_token(cfg);
        assert!(token.starts_with("gAAAAAC"));
    }

    #[test]
    fn conversation_prefix() {
        let cfg = vec![serde_json::json!(1); 18];
        let token = solve_proof_of_work("", "ffff", cfg);
        assert!(token.starts_with("gAAAAAB"));
    }
}
