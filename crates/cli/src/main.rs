//! `alloy` command-line entry point.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = alloy_ui_cli::run(&args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
