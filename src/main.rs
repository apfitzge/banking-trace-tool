use clap::Parser;
use setup::get_event_file_paths;

use crate::cli::Cli;

mod cli;
mod process;
mod setup;

#[allow(dead_code)]
fn main() {
    let Cli { path, mode } = Cli::parse();
    let _event_file_paths = get_event_file_paths(path);

    match mode {}
}
