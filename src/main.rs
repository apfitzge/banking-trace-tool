use {
    crate::{cli::Cli, slot_ranges::slot_ranges, update_alt_store::update_alt_store},
    clap::Parser,
    cli::TraceToolMode,
    setup::get_event_file_paths,
    std::process::exit,
};

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
        TraceToolMode::SlotRanges => slot_ranges(&event_file_paths),
        TraceToolMode::UpdateAltStore {
            start_slot,
            end_slot,
        } => update_alt_store(&event_file_paths, start_slot, end_slot),
    };

    if let Err(err) = result {
        eprintln!("Error: {err}");
        exit(1);
    }
}
