//! The smallest CRUD app: one struct, one call.
use alloy_ui::prelude::*;

#[derive(Record, Clone, Default)]
struct Book {
    title: String,
    author: String,
    #[form(range = (1.0, 5.0))]
    rating: u8,
    read: bool,
}

fn main() {
    crud_app("Books", || {
        Records::<Book>::of([
            vals!["Dune", "Frank Herbert", 5, true],
            vals!["Piranesi", "Susanna Clarke", 4, false],
        ])
    })
    .size(900.0, 560.0)
    .run()
}
