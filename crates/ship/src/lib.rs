//! Shipping an Alloy app: signed update manifests, self-update, crash reports.
//!
//! * [`update`] checks a signed `latest.json`, downloads the artifact for this
//!   OS and CPU, verifies size and SHA-256 (both covered by the manifest's
//!   Ed25519 signature) and swaps it in or hands it to the OS installer.
//! * [`crash`] installs a panic hook that writes a JSON report to disk; the app
//!   decides if and when to upload (see [`crash::upload_pending`]).
//! * [`keys`] and [`manifest`] are shared with `alloy bundle`, which signs the
//!   manifest at build time, so the format has exactly one implementation.
//!
//! Nothing here talks to the network unless the app calls it.

pub mod crash;
pub mod keys;
pub mod manifest;
pub mod net;
pub mod update;
pub mod version;

pub use net::Fetch;
#[cfg(feature = "http")]
pub use net::UreqFetch;

/// Everything that can go wrong.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A key or signature was not valid hex of the right length.
    #[error("invalid key or signature: {0}")]
    BadKey(String),
    /// The manifest signature did not verify against the configured key.
    #[error("update manifest signature is invalid")]
    BadSignature,
    /// The manifest was not valid JSON of the expected shape.
    #[error("update manifest is malformed: {0}")]
    BadManifest(String),
    /// A downloaded file did not match the size or hash in the manifest.
    #[error("downloaded file does not match the signed manifest: {0}")]
    Mismatch(String),
    /// The network request failed.
    #[error("network: {0}")]
    Network(String),
    /// A filesystem operation failed.
    #[error("{what}: {source}")]
    Io {
        /// What was being attempted.
        what: String,
        /// Underlying error.
        source: std::io::Error,
    },
    /// The manifest has no artifact for this platform.
    #[error("no update artifact for {0}")]
    NoArtifact(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> Error {
    let what = what.into();
    move |source| Error::Io { what, source }
}
