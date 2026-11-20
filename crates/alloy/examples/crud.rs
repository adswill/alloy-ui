//! The smallest CRUD app: one struct, one call.
use alloy_ui::{prelude::*, vals};
use serde::{Deserialize, Serialize};

#[derive(Record, Clone, Default, Serialize, Deserialize)]
struct Book {
    #[form(required)]
    title: String,
    author: String,
    #[form(range = (1.0, 5.0))]
    rating: u8,
    read: bool,
}

fn main() {
    let books = Records::<Book>::of([
        vals!["Dune", "Frank Herbert", 5, true],
        vals!["Piranesi", "Susanna Clarke", 4, false],
    ]);
    crud("Books", books).run_sized("Books", 900.0, 560.0)
}
