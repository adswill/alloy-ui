//! `alloy` command-line entry point.
fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `cargo alloy test` runs `cargo-alloy alloy test`: drop the repeated subcommand name.
    if args.first().is_some_and(|a| a == "alloy") {
        args.remove(0);
    }
    if let Err(e) = alloy_ui_cli::run(&args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
