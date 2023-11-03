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
    /// Write graphia json input file for a given slot.
    GraphiaInput {
        /// The slot to write the graphia input file for.
        slot: Slot,
        /// The filepath to write the graphia input file to.
        #[clap(default_value = "graphia_input.json")]
        output: PathBuf,
    },
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
