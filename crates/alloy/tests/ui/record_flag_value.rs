use alloy_ui::prelude::*;
#[derive(Clone, Default, Record)]
struct P {
    name: String,
    #[form(email = true)]
    mail: String,
}
fn main() {}
