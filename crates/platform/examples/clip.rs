//! `clip get` prints the clipboard; `clip set TEXT` sets it and stays alive for 3 s.
//! Used to cross-check the native clipboards against other clients.
use alloy_ui_platform::default_clipboard;

fn main() {
    let mut c = default_clipboard();
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("set") => {
            c.write_text(&args[2]).expect("write");
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
        _ => println!("{}", c.read_text().expect("read").unwrap_or_default()),
    }
}
