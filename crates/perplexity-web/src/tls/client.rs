use std::sync::OnceLock;

use wreq::Client;
use wreq_util::Emulation;

use crate::Error;

static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();

pub fn shared_client() -> Result<&'static Client, Error> {
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .emulation(Emulation::Firefox136)
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| Error::Tls(e.clone()))
}
