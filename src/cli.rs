use {
    clap::{Parser, Subcommand},
    std::path::PathBuf,
};

#[derive(Debug, Parser)]
pub struct Cli {
    /// The path to the banking trace event files.
    #[clap(short, long)]
    path: PathBuf,
    /// Mode to run the trace-tool in.
    #[command(subcommand)]
    mode: TraceToolMode,
}

#[derive(Debug, Subcommand)]
pub enum TraceToolMode {}
