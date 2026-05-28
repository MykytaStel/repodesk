use clap::Parser;
use repodesk_cli::cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(error) = repodesk_cli::commands::dispatch(cli) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
