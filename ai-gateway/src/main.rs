use std::{path::PathBuf, time::Duration};

use ai_gateway::{
    app::{App, startup_tasks},
    config::Config,
    discover::monitor::{
        health::provider::HealthMonitor, rate_limit::RateLimitMonitor,
    },
    error::{init::InitError, runtime::RuntimeError},
    metrics::system::SystemMetrics,
    middleware::rate_limit,
    store::db_listener::DatabaseListener,
    utils::meltdown::TaggedService,
};
use clap::{Parser, Subcommand};
use meltdown::Meltdown;
use opentelemetry_sdk::{
    logs::SdkLoggerProvider, metrics::SdkMeterProvider,
    trace::SdkTracerProvider,
};
use tracing::{debug, info};

// jemalloc is used on Unix for lower memory use; Windows release builds use the
// system allocator.
#[cfg(not(windows))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Path to the default config file.
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the AI gateway HTTP server (default).
    Serve,
    /// `ChatGPT` web session utilities.
    #[cfg(feature = "chatgpt-login")]
    Chatgpt {
        #[command(subcommand)]
        action: ChatgptAction,
    },
    /// Perplexity web session utilities.
    #[cfg(feature = "perplexity-login")]
    Perplexity {
        #[command(subcommand)]
        action: PerplexityAction,
    },
    /// `DeepSeek` web session utilities.
    #[cfg(feature = "deepseek-login")]
    Deepseek {
        #[command(subcommand)]
        action: DeepseekAction,
    },
}

#[derive(Debug, Subcommand)]
#[cfg(feature = "chatgpt-login")]
enum ChatgptAction {
    /// Open a browser to log in and save session cookies to the default path
    /// (`dev/session.json`; configure via secrets file).
    Login,
    /// Paste Cookie header from Firefox/Chrome `DevTools`.
    Import {
        /// Full cookie string, e.g. `Cookie:
        /// __Secure-next-auth.session-token=...; cf_clearance=...`
        #[arg(long)]
        cookie: String,
    },
}

#[derive(Debug, Subcommand)]
#[cfg(feature = "perplexity-login")]
enum PerplexityAction {
    /// Open perplexity.ai in browser and save cookies to
    /// `PERPLEXITY_BROWSER_CLI`.
    Login,
    /// Paste Cookie header from browser `DevTools` (logged-in account).
    Import {
        #[arg(long)]
        cookie: String,
    },
    /// Send one test query using the session file.
    Probe {
        #[arg(long, default_value = "Reply with exactly one word: OK")]
        query: String,
    },
}

#[derive(Debug, Subcommand)]
#[cfg(feature = "deepseek-login")]
enum DeepseekAction {
    /// Open chat.deepseek.com and save `localStorage.userToken` to
    /// `DEEPSEEK_BROWSER_CLI` path.
    Login,
    /// Paste `userToken` from `DevTools` → `Application` → `Local Storage`.
    Import {
        #[arg(long)]
        token: String,
    },
    /// Verify session: `users/current` (+ optional one completion).
    Probe {
        /// When set, sends one non-stream completion with this user message.
        #[arg(long)]
        query: Option<String>,
        /// Smoke-test strict `json_schema` through the live session.
        #[arg(long)]
        structured_output: bool,
        /// Binary-search largest single-prompt completion size.
        #[arg(long)]
        context_limit: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), RuntimeError> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let cli = Cli::parse();

    #[cfg(feature = "chatgpt-login")]
    if let Some(Command::Chatgpt { action }) = cli.command {
        let result = match action {
            ChatgptAction::Login => {
                ai_gateway::cli::chatgpt_login::run_login().await
            }
            ChatgptAction::Import { cookie } => {
                ai_gateway::cli::chatgpt_login::run_import(cookie).await
            }
        };
        if let Err(e) = result {
            eprintln!("chatgpt command failed: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    #[cfg(feature = "perplexity-login")]
    if let Some(Command::Perplexity { action }) = cli.command {
        let result = match action {
            PerplexityAction::Login => {
                ai_gateway::cli::perplexity_login::run_login().await
            }
            PerplexityAction::Import { cookie } => {
                ai_gateway::cli::perplexity_login::run_import(cookie).await
            }
            PerplexityAction::Probe { query } => {
                ai_gateway::cli::perplexity_login::run_probe(query).await
            }
        };
        if let Err(e) = result {
            eprintln!("perplexity command failed: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    #[cfg(feature = "deepseek-login")]
    if let Some(Command::Deepseek { action }) = cli.command {
        let result = match action {
            DeepseekAction::Login => {
                ai_gateway::cli::deepseek_login::run_login().await
            }
            DeepseekAction::Import { token } => {
                ai_gateway::cli::deepseek_login::run_import(token).await
            }
            DeepseekAction::Probe {
                query,
                structured_output,
                context_limit,
            } => {
                ai_gateway::cli::deepseek_login::run_probe(
                    query,
                    structured_output,
                    context_limit,
                )
                .await
            }
        };
        if let Err(e) = result {
            eprintln!("deepseek command failed: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let config = load_and_validate_config(cli.config, cli.verbose)?;
    let (logger_provider, tracer_provider, metrics_provider) =
        init_telemetry(&config)?;

    run_app(config).await?;

    shutdown_telemetry(logger_provider, &tracer_provider, metrics_provider);

    println!("shut down");

    Ok(())
}

fn load_and_validate_config(
    config_path: Option<PathBuf>,
    verbose: bool,
) -> Result<Config, RuntimeError> {
    let mut config = match Config::try_read(config_path) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("failed to read config: {error}");
            std::process::exit(1);
        }
    };

    if verbose {
        config.telemetry.level = "info,ai_gateway=trace".to_string();
    }

    config.validate().inspect_err(|e| {
        tracing::error!(error = %e, "configuration validation failed");
    })?;

    ai_gateway::emulated::apply_if_enabled(&mut config);

    if !config.has_autodefault_router()
        && config.credentials.has_for(
            &ai_gateway::types::provider::InferenceProvider::Named(
                "chatgpt-web".into(),
            ),
        )
        && !ai_gateway::config::chatgpt_web::session_file_available()
    {
        eprintln!(
            "chatgpt-web is configured in secrets but session file is \
             missing. Run: cargo run --features chatgpt-login -- chatgpt login"
        );
    }

    Ok(config)
}

fn init_telemetry(
    config: &Config,
) -> Result<
    (
        Option<SdkLoggerProvider>,
        SdkTracerProvider,
        Option<SdkMeterProvider>,
    ),
    InitError,
> {
    let (logger_provider, tracer_provider, metrics_provider) =
        telemetry::init_telemetry(&config.telemetry)?;

    debug!("telemetry initialized");
    let pretty_config = serde_yml::to_string(&config)
        .expect("config should always be serializable");
    tracing::debug!(config = pretty_config, "Creating app with config");

    #[cfg(debug_assertions)]
    tracing::warn!("running in debug mode");

    if config.has_autodefault_router() {
        tracing::info!(
            "Router 'autodefault' is configured, will be available at \
             /router/autodefault"
        );
    }

    Ok((logger_provider, tracer_provider, metrics_provider))
}

async fn run_app(config: Config) -> Result<(), RuntimeError> {
    const CLEANUP_INTERVAL: Duration = Duration::from_mins(5);
    let mut shutting_down = false;
    let app = App::new(config).await?;
    let config = app.state.config();
    let health_monitor = HealthMonitor::new(app.state.clone());
    let rate_limit_monitor = RateLimitMonitor::new(app.state.clone());
    let client_access_reloader = config
        .client_access
        .enabled
        .then(|| {
            config.client_access.file.as_ref().map(|path| {
                ai_gateway::client_access::reload::ClientAccessReloader::new(
                    app.state.clone(),
                    path.clone(),
                    config.client_access.reload_interval,
                )
            })
        })
        .flatten();

    let rate_limiting_cleanup_service =
        config.global.rate_limit.as_ref().map(|_| {
            rate_limit::cleanup::GarbageCollector::new(
                app.state.clone(),
                CLEANUP_INTERVAL,
            )
        });

    // Infostart control plane (router/org push) is deferred — see
    // docs/control-plane.md. Helicone Cloud sidecar websocket must not
    // block HTTP bind (0.5.3+).
    let tasks = startup_tasks::meltdown_service_names(config);
    let mut meltdown = Meltdown::new().register(TaggedService::new(
        "shutdown-signals",
        ai_gateway::utils::meltdown::wait_for_shutdown_signals,
    ));

    if config.deployment_target.is_cloud() {
        meltdown = meltdown.register(TaggedService::new(
            "database-listener",
            DatabaseListener::new(
                config.database.url.expose(),
                app.state.clone(),
            )
            .await?,
        ));
    }

    meltdown = meltdown
        .register(TaggedService::new("gateway", app))
        .register(TaggedService::new(
            "provider-health-monitor",
            health_monitor,
        ))
        .register(TaggedService::new(
            "provider-rate-limit-monitor",
            rate_limit_monitor,
        ))
        .register(TaggedService::new("system-metrics", SystemMetrics));

    if let Some(rate_limiting_cleanup_service) = rate_limiting_cleanup_service {
        meltdown = meltdown.register(TaggedService::new(
            "rate-limiting-cleanup",
            rate_limiting_cleanup_service,
        ));
    }

    if let Some(client_access_reloader) = client_access_reloader {
        meltdown = meltdown.register(TaggedService::new(
            "client-access-reloader",
            client_access_reloader,
        ));
    }

    info!(tasks = ?tasks, "starting services");

    while let Some((service, result)) = meltdown.next().await {
        match result {
            Ok(()) => info!(%service, "service stopped successfully"),
            Err(error) => tracing::error!(%service, %error, "service crashed"),
        }

        if !shutting_down {
            info!("propagating shutdown signal...");
            meltdown.trigger();
            shutting_down = true;
        }
    }
    Ok(())
}

fn shutdown_telemetry(
    logger_provider: Option<SdkLoggerProvider>,
    tracer_provider: &SdkTracerProvider,
    metrics_provider: Option<SdkMeterProvider>,
) {
    if let Some(logger_provider) = logger_provider
        && let Err(e) = logger_provider.shutdown()
    {
        println!("error shutting down logger provider: {e}");
    }
    if let Err(e) = tracer_provider.shutdown() {
        println!("error shutting down tracer provider: {e}");
    }
    if let Some(metrics_provider) = metrics_provider
        && let Err(e) = metrics_provider.shutdown()
    {
        println!("error shutting down metrics provider: {e}");
    }
}
