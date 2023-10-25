use {
    clap::{Parser, Subcommand},
    std::path::PathBuf,
};

#[derive(Debug, Parser)]
pub struct Cli {
    /// The path to the banking trace event file directory.
    #[clap(short, long)]
    pub path: PathBuf,
    /// Mode to run the trace-tool in.
    #[command(subcommand)]
    pub mode: TraceToolMode,
}

#[derive(Debug, Subcommand)]
pub enum TraceToolMode {
    /// Get the ranges of slots for data in directory.
    SlotRanges,
}
