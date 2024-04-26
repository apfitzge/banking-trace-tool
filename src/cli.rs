use {
    clap::{Args, Parser, Subcommand},
    solana_sdk::{clock::Slot, pubkey::Pubkey},
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
    /// Dump all the non-vote events in the directory.
    Dump {
        /// Limit dumping to these accounts, if specified.
        #[clap(short, long)]
        accounts: Option<Vec<Pubkey>>,
        /// Skip ALT resolution.
        #[clap(short, long)]
        skip_alt_resolution: bool,
        /// Timestamp to start dumping from.
        /// Format: "YYYY-MM-DDTHH:HH:SS.xxxxxxxxZ".
        /// Example: "2024-02-02T20:01:30.436991968Z".
        #[clap(long)]
        start_timestamp: Option<String>,
        /// Timestamp to stop dumping at.
        /// Format: "YYYY-MM-DDTHH:HH:SS.xxxxxxxxZ".
        /// Example: "2024-02-02T20:01:30.436991968Z".
        #[clap(long)]
        end_timestamp: Option<String>,
    },
    /// Write graphia json input file for a given slot.
    GraphiaInput {
        /// The slot to write the graphia input file for.
        slot: Slot,
        /// The filepath to write the graphia input file to.
        #[clap(default_value = "graphia_input.json")]
        output: PathBuf,
    },
    /// Get summary of packet counts.
    PacketCount {
        /// Timestamp to start summary from.
        /// Format: "YYYY-MM-DDTHH:HH:SS.xxxxxxxxZ".
        /// Example: "2024-02-02T20:01:30.436991968Z".
        #[clap(long)]
        start_timestamp: Option<String>,
        /// Timestamp to stop summary at.
        /// Format: "YYYY-MM-DDTHH:HH:SS.xxxxxxxxZ".
        /// Example: "2024-02-02T20:01:30.436991968Z".
        #[clap(long)]
        end_timestamp: Option<String>,
    },
    /// Get the ranges of slots for data in directory.
    SlotRanges,
    /// Get the time ranges of data in the directory.
    TimeRange,
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
