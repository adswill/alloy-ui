use alloy_ui::prelude::*;
struct S { name: String }
fn main() {
    let s = &S { name: String::new() };
    let _ = at!(s.name.clone());
}
