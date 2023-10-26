use {
    clap::{Args, Parser, Subcommand},
    solana_sdk::clock::Slot,
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
    /// Get account usage statistics for a given slot range.
    AccountUsage(SlotRange),
    /// Get the ranges of slots for data in directory.
    SlotRanges,
    /// Update Address-Lookup-Table store for tables used in a given slot-range.
    UpdateAltStore(SlotRange),
}

#[derive(Debug, Args)]
pub struct SlotRange {
    /// The starting slot of the range, inclusive.
    pub start_slot: Slot,
    /// The ending slot of the range, inclusive.
    pub end_slot: Slot,
}
