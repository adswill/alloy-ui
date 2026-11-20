//! Typed background commands and the capability/permissions config.
//!
//! * [`capability`]: the TOML permissions file, [`CapabilitySet`] and its checks.
//! * [`task`]: a thread-based [`TaskRunner`] that runs futures or blocking
//!   closures off the UI thread and wakes the platform shell when results arrive.
//! * [`command`]: a [`CommandRegistry`] of named, capability-gated, typed commands.
#![warn(missing_docs)]

pub mod capability;
pub mod command;
pub mod task;

pub use capability::{Capability, CapabilityError, CapabilitySet};
pub use command::{CommandError, CommandRegistry};
pub use task::{CancelToken, TaskHandle, TaskRunner};
