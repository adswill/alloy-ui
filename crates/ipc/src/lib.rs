//! Typed background commands and the capability/permissions config. Implemented in the ipc workstream.
#![warn(missing_docs)]

/// A capability an app can be granted in its config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Read and write the clipboard.
    Clipboard,
    /// Open native file dialogs.
    Dialogs,
    /// Read the filesystem.
    FsRead,
    /// Write the filesystem.
    FsWrite,
    /// Make network requests.
    Network,
}
