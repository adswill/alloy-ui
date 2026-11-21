use alloy_ui::prelude::*;
#[derive(Clone, Default, Record)]
struct P {
    name: String,
    #[form(range = (1.0, 2.0, 3.0))]
    age: f32,
}
fn main() {}
