//! Error type for renderer construction and image I/O.

use thiserror::Error;

/// Errors produced by the renderer crate.
#[derive(Debug, Error)]
pub enum RenderError {
    /// No suitable GPU (or software) adapter was found.
    #[error("no suitable graphics adapter found: {0}")]
    NoAdapter(String),
    /// The logical device could not be created.
    #[error("failed to create graphics device: {0}")]
    Device(String),
    /// The window surface could not be created or configured.
    #[error("window surface error: {0}")]
    Surface(String),
    /// PNG encode/decode failure.
    #[error("image codec error: {0}")]
    Codec(String),
    /// Filesystem failure.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}
