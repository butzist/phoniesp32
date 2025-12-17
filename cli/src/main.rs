use clap::{Parser, Subcommand};
mod commands;
use commands::transcode::TranscodeCommand;

#[derive(Parser)]
#[command(name = "pecli")]
#[command(about = "PhoniESP32 CLI tool")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Transcode(TranscodeCommand),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Transcode(cmd) => cmd.execute().await?,
    }

    Ok(())
}
