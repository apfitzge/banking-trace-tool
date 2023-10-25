use {
    crate::{cli::Cli, slot_ranges::slot_ranges},
    clap::Parser,
    cli::TraceToolMode,
    setup::get_event_file_paths,
    std::process::exit,
};

mod cli;
mod process;
mod setup;
mod slot_ranges;

fn main() {
    let Cli { path, mode } = Cli::parse();

    if !path.is_dir() {
        eprintln!("{} is not a directory", path.display());
        exit(1);
    }

    let event_file_paths = get_event_file_paths(path);
    let result = match mode {
        TraceToolMode::SlotRanges => slot_ranges(&event_file_paths),
    };

    if let Err(err) = result {
        eprintln!("Error: {err}");
        exit(1);
    }
}
