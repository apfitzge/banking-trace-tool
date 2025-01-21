use {
    crate::{
        account_usage::account_usage, cli::Cli, graphia_input::graphia_input,
        packet_count::packet_count, slot_ranges::slot_ranges, time_range::time_range,
        update_alt_store::update_alt_store,
    },
    chrono::{DateTime, Utc},
    clap::Parser,
    cli::TraceToolMode,
    setup::get_event_file_paths,
    std::process::exit,
};

mod account_usage;
mod cli;
mod dump;
mod graphia_input;
mod packet_count;
mod process;
mod setup;
mod slot_ranges;
mod time_range;
mod update_alt_store;

fn main() {
    let Cli { path, mode } = Cli::parse();

    if !path.is_dir() {
        eprintln!("{} is not a directory", path.display());
        exit(1);
    }

    let event_file_paths = get_event_file_paths(path);
    let result = match mode {
        TraceToolMode::AccountUsage(slot_range) => account_usage(&event_file_paths, slot_range),
        TraceToolMode::Dump {
            accounts,
            ips,
            skip_alt_resolution,
            start_timestamp,
            end_timestamp,
        } => dump::dump(
            &event_file_paths,
            accounts.map(|accounts| accounts.into_iter().collect()),
            ips.map(|ips| ips.into_iter().collect()),
            skip_alt_resolution,
            start_timestamp.map(cli_parse_timestamp),
            end_timestamp.map(cli_parse_timestamp),
        ),
        TraceToolMode::GraphiaInput { slot, output } => {
            graphia_input(&event_file_paths, slot, output)
        }
        TraceToolMode::PacketCount {
            start_timestamp,
            end_timestamp,
            ip_count,
        } => packet_count(
            &event_file_paths,
            start_timestamp.map(cli_parse_timestamp),
            end_timestamp.map(cli_parse_timestamp),
            ip_count.unwrap_or(5),
        ),
        TraceToolMode::SlotRanges => slot_ranges(&event_file_paths),
        TraceToolMode::TimeRange => time_range(&event_file_paths),
        TraceToolMode::UpdateAltStore(slot_range) => {
            update_alt_store(&event_file_paths, slot_range)
        }
    };

    if let Err(err) = result {
        eprintln!("Error: {err}");
        exit(1);
    }
}

fn cli_parse_timestamp(s: String) -> DateTime<Utc> {
    s.parse().expect("Failed to parse timestamp")
}
