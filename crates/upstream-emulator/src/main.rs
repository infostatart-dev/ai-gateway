use clap::Parser;
use upstream_emulator::{EmulatorConfig, bind_and_serve};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    address: String,
    #[arg(short, long, default_value_t = 5151)]
    port: u16,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    bind_and_serve(EmulatorConfig {
        address: args.address,
        port: args.port,
        ..EmulatorConfig::default()
    })
    .await
}
