use clap::Parser;

fn main() {
    let cli = rime_cli::Cli::parse();
    if let Err(error) = rime_cli::run(cli) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
