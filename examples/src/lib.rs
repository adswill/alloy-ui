//! Example applications for Alloy, written against the concise `alloy_ui::prelude` API.
//!
//! Each module has `app()` (the state and view), a window `SIZE` and a `run()` used by its
//! binary. Every binary also takes `--headless [file.png] [--click Text]...` to render one
//! frame without a window, which is how the golden tests and the benchmark harness drive them.
#![warn(missing_docs)]

pub mod counter;
pub mod dashboard;
pub mod gallery;
pub mod settings;
pub mod text_editor;
pub mod todo;
