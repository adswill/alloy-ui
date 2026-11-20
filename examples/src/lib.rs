//! Example applications for Alloy: counter, todo, text editor, settings, dashboard.
//!
//! Each example is an [`alloy_ui_core::Application`]. There is no windowed
//! shell yet, so the binaries run each app headlessly: they drive a scripted
//! sequence of input events through the real [`alloy_ui_core::runtime::Runtime`],
//! rasterise the result with the CPU renderer and write a PNG.
#![warn(missing_docs)]

pub mod apps;
pub mod custom;
pub mod run;
