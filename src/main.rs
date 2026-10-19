mod auth;
mod cairo;
mod config;
mod content;
mod db;
mod exercise;
mod gemini;
mod http;
mod review;
mod settings;

use anyhow::Context;
use clap::{Parser, Subcommand};
use config::Config;

#[derive(Parser)]
#[command(name = "eng", about = "English dictation Mini App backend")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the HTTP server
    Serve,
    /// Content tooling
    Content {
        #[command(subcommand)]
        command: ContentCommand,
    },
}

#[derive(Subcommand)]
enum ContentCommand {
    /// Generate sentences for one situation, or all of them
    Generate {
        #[arg(long, conflicts_with = "all")]
        situation: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Generate MP3 for sentences and words that have none
    Audio {
        #[arg(long)]
        missing: bool,
        /// Clean start and end artifacts from existing files instead of generating
        #[arg(long)]
        repair: bool,
    },
    /// Write one sample MP3 per candidate voice
    Voices {
        #[arg(long)]
        text: String,
        #[arg(long, default_value = "data/voice-samples")]
        out: std::path::PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "eng=info,tower_http=info".into()),
        )
        .init();

    match Cli::parse().command {
        Command::Serve => serve().await,
        Command::Content { command } => content_command(command).await,
    }
}

async fn serve() -> anyhow::Result<()> {
    let config = Config::from_env().context("invalid configuration")?;
    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("cannot bind {}", config.bind_addr))?;
    tracing::info!(addr = %config.bind_addr, gemini_keys = config.gemini_keys.len(), "listening");
    let state = http::AppState {
        pool: pool.clone(),
        settings: std::sync::Arc::new(settings::Settings::load(&pool).await?),
        config: std::sync::Arc::new(config),
    };
    axum::serve(listener, http::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;
    pool.close().await;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

/// Voice sampling only needs the Gemini keys, so it skips the full Config.
fn gemini_keys_from_env() -> Vec<String> {
    Config::gemini_keys_from(std::env::vars())
}

async fn content_command(command: ContentCommand) -> anyhow::Result<()> {
    if let ContentCommand::Voices { text, out } = &command {
        let gemini = gemini::Gemini::new(gemini_keys_from_env())?;
        return content::voices::write_samples(&gemini, text, out).await;
    }
    if let ContentCommand::Audio { repair: true, .. } = &command {
        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "data".into());
        return content::audio::repair(std::path::Path::new(&data_dir)).await;
    }
    let config = Config::from_env().context("invalid configuration")?;
    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;
    let gemini = std::sync::Arc::new(gemini::Gemini::new(config.gemini_keys.clone())?);
    match command {
        ContentCommand::Generate { situation, all } => {
            anyhow::ensure!(
                all || situation.is_some(),
                "pass --situation <slug> or --all"
            );
            content::generate::run(&pool, gemini, situation.map(|s| vec![s])).await
        }
        ContentCommand::Audio { .. } => content::audio::run(&pool, gemini, &config.data_dir).await,
        ContentCommand::Voices { .. } => unreachable!("handled above"),
    }
}
