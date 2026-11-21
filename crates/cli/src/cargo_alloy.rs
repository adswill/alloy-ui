//! `cargo-alloy`: the same tool as `alloy`, so `cargo alloy test` works.
fn main() {
    // cargo runs `cargo-alloy alloy <args>`; drop the repeated subcommand name.
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "alloy") {
        args.remove(0);
    }
    if let Err(e) = alloy_ui_cli::run(&args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
