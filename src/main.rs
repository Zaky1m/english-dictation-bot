mod config;
mod content;
mod db;
mod gemini;
mod http;

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
        Command::Content {
            command: ContentCommand::Voices { text, out },
        } => {
            let keys = gemini_keys_from_env();
            let gemini = gemini::Gemini::new(keys)?;
            content::voices::write_samples(&gemini, &text, &out).await
        }
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
    axum::serve(listener, http::router())
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
