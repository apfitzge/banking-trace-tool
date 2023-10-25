use clap::Parser;

mod cli;
mod process;

fn main() {
    let args = cli::Cli::parse();
    println!("Args: {args:?}");
}
