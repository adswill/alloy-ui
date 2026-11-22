//! The counter example served to a browser: `cargo run -p alloy-ui-examples --bin web`,
//! then open http://127.0.0.1:8080. Pass an address to change it.
fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".into());
    alloy_ui::web::serve(
        alloy_ui_examples::counter::app(),
        &addr,
        alloy_ui_examples::counter::SIZE,
    )
    .expect("web server");
}
