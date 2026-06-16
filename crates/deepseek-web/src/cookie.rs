use rand::Rng;
use uuid::Uuid;

pub fn generate_fake_cookie() -> String {
    let ts = chrono::Utc::now().timestamp_millis();
    let hwid = random_hex(18);
    let hm = Uuid::new_v4();
    let frid = Uuid::new_v4();
    format!(
        "intercom-HWWAFSESTIME={ts}; HWWAFSESID={hwid}; Hm_lvt_{hm}={}; \
         _frid={frid}",
        ts / 1000
    )
}

fn random_hex(len: usize) -> String {
    let mut rng = rand::rng();
    (0..len)
        .map(|_| format!("{:x}", rng.random_range(0..16)))
        .collect()
}
