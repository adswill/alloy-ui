//! Error type for the CLI.

use std::path::PathBuf;

/// Everything that can go wrong while building or bundling.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Bad command line.
    #[error("{0}")]
    Usage(String),
    /// `cargo metadata` failed or returned something unexpected.
    #[error("cargo metadata: {0}")]
    Metadata(String),
    /// The app's `[package.metadata.alloy]` table (or an icon) is invalid.
    #[error("{0}")]
    Config(String),
    /// The capability config file failed validation.
    #[error("capability config `{path}`: {source}")]
    Capabilities {
        /// The file that was read.
        path: PathBuf,
        /// Why validation failed.
        source: alloy_ui_ipc::CapabilityError,
    },
    /// A filesystem operation failed.
    #[error("{what}: {source}")]
    Io {
        /// What was being attempted.
        what: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// An external tool needed for a format is not installed.
    #[error("`{tool}` not found: {hint}")]
    ToolMissing {
        /// Tool name.
        tool: &'static str,
        /// How to get it.
        hint: &'static str,
    },
    /// An external tool exited unsuccessfully.
    #[error("`{cmd}` failed: {status}")]
    CommandFailed {
        /// Command line, for the message.
        cmd: String,
        /// Exit status text.
        status: String,
    },
    /// A format cannot be produced on this host.
    #[error("format `{format}` can only be built on {host}")]
    WrongHost {
        /// Format name.
        format: &'static str,
        /// Required host OS.
        host: &'static str,
    },
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Build a `map_err` closure that attaches context to an I/O error.
pub fn io<'a>(what: impl Into<String> + 'a) -> impl FnOnce(std::io::Error) -> Error + 'a {
    move |source| Error::Io {
        what: what.into(),
        source,
    }
}
