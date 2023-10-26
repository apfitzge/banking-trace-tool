use {
    crate::{
        account_usage::account_usage, cli::Cli, slot_ranges::slot_ranges,
        update_alt_store::update_alt_store,
    },
    clap::Parser,
    cli::TraceToolMode,
    setup::get_event_file_paths,
    std::process::exit,
};

mod account_usage;
mod cli;
mod process;
mod setup;
mod slot_ranges;
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
        TraceToolMode::SlotRanges => slot_ranges(&event_file_paths),
        TraceToolMode::UpdateAltStore(slot_range) => {
            update_alt_store(&event_file_paths, slot_range)
        }
    };

    if let Err(err) = result {
        eprintln!("Error: {err}");
        exit(1);
    }
}
