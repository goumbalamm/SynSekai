mod app;
mod engine;
mod spoofer;
mod terminal;
mod tui;
mod types;
mod ui;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "synsekai", about = "Interactive TUI torrent client")]
struct Cli {
    /// Directory to save downloaded files
    #[arg(long, default_value = "~/Downloads")]
    output_dir: PathBuf,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("librqbit=warn".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    // Expand ~ in path
    let output_dir = if let Some(rest) = cli.output_dir.to_str().and_then(|s| s.strip_prefix("~/"))
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(rest)
    } else {
        cli.output_dir
    };

    std::fs::create_dir_all(&output_dir)?;

    let engine = engine::TorrentEngine::new(output_dir).await?;
    terminal::run(engine).await
}
