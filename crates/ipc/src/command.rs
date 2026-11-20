//! Typed, capability-gated named commands.

use crate::capability::{Capability, CapabilityError, CapabilitySet};
use crate::task::{TaskHandle, TaskRunner};
use alloy_ui_core::runtime::BoxFuture;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;

/// Errors from registering or invoking a command.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    /// No command with that name is registered.
    #[error("unknown command `{0}`")]
    Unknown(String),
    /// A required capability was not granted.
    #[error("command `{command}` denied: {source}")]
    Denied {
        /// The command name.
        command: String,
        /// The underlying capability failure.
        #[source]
        source: CapabilityError,
    },
    /// The arguments did not deserialise into the handler's argument type.
    #[error("bad arguments for `{command}`: {message}")]
    BadArgs {
        /// The command name.
        command: String,
        /// The serde error text.
        message: String,
    },
    /// The handler failed (message supplied by the handler) or its result could not be serialised.
    #[error("command failed: {0}")]
    Failed(String),
}

type Handler = Box<
    dyn Fn(Value) -> Result<BoxFuture<Result<Value, CommandError>>, CommandError> + Send + Sync,
>;

struct Entry {
    required: Vec<Capability>,
    handler: Handler,
}

/// Registry of named async commands with serde args and results.
#[derive(Default)]
pub struct CommandRegistry {
    commands: HashMap<String, Entry>,
}

impl CommandRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `name`, requiring all of `required` to be granted at invoke time.
    /// The handler returns `Result<R, String>`; the error string becomes [`CommandError::Failed`].
    /// Re-registering a name replaces the previous command.
    pub fn register<A, R, Fut, F>(&mut self, name: &str, required: &[Capability], handler: F)
    where
        A: DeserializeOwned + 'static,
        R: Serialize + 'static,
        Fut: Future<Output = Result<R, String>> + Send + 'static,
        F: Fn(A) -> Fut + Send + Sync + 'static,
    {
        let cmd = name.to_string();
        let handler: Handler = Box::new(move |args| {
            let a: A = serde_json::from_value(args).map_err(|e| CommandError::BadArgs {
                command: cmd.clone(),
                message: e.to_string(),
            })?;
            let fut = handler(a);
            Ok(Box::pin(async move {
                let r = fut.await.map_err(CommandError::Failed)?;
                serde_json::to_value(r).map_err(|e| CommandError::Failed(e.to_string()))
            }))
        });
        self.commands.insert(
            name.to_string(),
            Entry {
                required: required.to_vec(),
                handler,
            },
        );
    }

    /// Whether `name` is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    /// Sorted names of all registered commands.
    pub fn names(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.commands.keys().map(String::as_str).collect();
        v.sort_unstable();
        v
    }

    /// Check permissions, decode args and return the future for the command.
    pub fn invoke(
        &self,
        name: &str,
        args: Value,
        caps: &CapabilitySet,
    ) -> Result<BoxFuture<Result<Value, CommandError>>, CommandError> {
        let entry = self
            .commands
            .get(name)
            .ok_or_else(|| CommandError::Unknown(name.to_string()))?;
        for c in &entry.required {
            caps.check(c).map_err(|source| {
                log::warn!("command `{name}` denied: {source}");
                CommandError::Denied {
                    command: name.to_string(),
                    source,
                }
            })?;
        }
        (entry.handler)(args)
    }

    /// Like [`invoke`](Self::invoke) but runs the command on `runner`, mapping the
    /// result (including pre-flight errors) to a message with `to_msg`.
    pub fn invoke_on<M: Send + 'static>(
        &self,
        runner: &TaskRunner<M>,
        name: &str,
        args: Value,
        caps: &CapabilitySet,
        to_msg: impl FnOnce(Result<Value, CommandError>) -> M + Send + 'static,
    ) -> TaskHandle {
        match self.invoke(name, args, caps) {
            Ok(fut) => runner.spawn(Box::pin(async move { to_msg(fut.await) })),
            Err(e) => runner.spawn(Box::pin(async move { to_msg(Err(e)) })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;
    use std::time::Duration;

    #[derive(Deserialize)]
    struct Add {
        a: i32,
        b: i32,
    }

    fn block_on<T>(f: BoxFuture<T>) -> T
    where
        T: Send + 'static,
    {
        let r = TaskRunner::new(|| {});
        r.spawn(f);
        r.recv_timeout(Duration::from_secs(5)).unwrap()
    }

    fn reg() -> CommandRegistry {
        let mut r = CommandRegistry::new();
        r.register("add", &[], |a: Add| async move { Ok(a.a + a.b) });
        r.register("paste", &[Capability::Clipboard], |_: ()| async {
            Ok("clip".to_string())
        });
        r.register("fail", &[], |_: ()| async {
            Err::<(), _>("boom".to_string())
        });
        r
    }

    #[test]
    fn invoke_ok() {
        let fut = reg()
            .invoke("add", json!({"a":2,"b":3}), &CapabilitySet::none())
            .unwrap();
        assert_eq!(block_on(fut), Ok(json!(5)));
    }

    #[test]
    fn unknown_command() {
        let e = reg()
            .invoke("nope", json!(null), &CapabilitySet::none())
            .err()
            .unwrap();
        assert_eq!(e, CommandError::Unknown("nope".into()));
    }

    #[test]
    fn bad_args() {
        let e = reg()
            .invoke("add", json!({"a":"x"}), &CapabilitySet::none())
            .err()
            .unwrap();
        assert!(matches!(e, CommandError::BadArgs { .. }));
    }

    #[test]
    fn capability_gate() {
        let r = reg();
        let e = r
            .invoke("paste", json!(null), &CapabilitySet::none())
            .err()
            .unwrap();
        assert!(matches!(e, CommandError::Denied { .. }));
        let caps = CapabilitySet::from_toml(r#"capabilities=["clipboard"]"#).unwrap();
        let fut = r.invoke("paste", json!(null), &caps).unwrap();
        assert_eq!(block_on(fut), Ok(json!("clip")));
    }

    #[test]
    fn handler_error() {
        let fut = reg()
            .invoke("fail", json!(null), &CapabilitySet::none())
            .unwrap();
        assert_eq!(block_on(fut), Err(CommandError::Failed("boom".into())));
    }

    #[test]
    fn names_and_contains() {
        let r = reg();
        assert_eq!(r.names(), vec!["add", "fail", "paste"]);
        assert!(r.contains("add") && !r.contains("x"));
    }

    #[test]
    fn invoke_on_runner_round_trip() {
        let runner: TaskRunner<Result<Value, CommandError>> = TaskRunner::new(|| {});
        let r = reg();
        r.invoke_on(
            &runner,
            "add",
            json!({"a":1,"b":1}),
            &CapabilitySet::none(),
            |x| x,
        );
        assert_eq!(
            runner.recv_timeout(Duration::from_secs(5)),
            Some(Ok(json!(2)))
        );
        r.invoke_on(&runner, "nope", json!(null), &CapabilitySet::none(), |x| x);
        assert!(matches!(
            runner.recv_timeout(Duration::from_secs(5)),
            Some(Err(CommandError::Unknown(_)))
        ));
    }
}
