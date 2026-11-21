# Capabilities and IPC

Like Tauri, Alloy lets an app declare up front what it may touch. The declaration is a TOML file next to your `Cargo.toml`; everything not listed is denied.

```toml
# alloy.capabilities.toml
capabilities = ["clipboard", "fs_read", "network"]

[fs]
read = ["/home/me/docs"]

[network]
hosts = ["api.example.com", "*.cdn.example.org"]
```

The capabilities are `clipboard`, `dialogs`, `fs_read`, `fs_write`, `network`, `notifications` and `open_external`. `fs_*` and `network` must come with a scope list (an empty scope, a relative path or one containing `..` is rejected). `alloy dev` and `alloy check` validate the file and print what was granted; `alloy dev` also passes its path in `ALLOY_CAPABILITIES`.

## Checking a capability in code

```rust
use alloy_ui::ipc::{Capability, CapabilitySet};
use std::path::Path;

# // Absolute paths need a drive letter on Windows; this keeps the example runnable there.
# let home = if cfg!(windows) { "C:/home/me" } else { "/home/me" };
let caps = CapabilitySet::from_toml(&format!(
    r#"
    capabilities = ["fs_read", "network"]
    [fs]
    read = ["{home}/docs"]
    [network]
    hosts = ["api.example.com"]
    "#
))
.unwrap();

assert!(caps.has(Capability::FsRead));
assert!(caps.check_fs_read(Path::new(&format!("{home}/docs/a.txt"))).is_ok());
assert!(caps.check_fs_read(Path::new("/etc/passwd")).is_err());
assert!(caps.check_host("api.example.com").is_ok());
assert!(caps.check_host("evil.example.net").is_err());
assert!(caps.check(&Capability::Clipboard).is_err());
```

## Typed commands

A `CommandRegistry` holds named async commands with serde arguments and results, each declaring the capabilities it needs. Invoking one checks the capabilities first, then decodes the arguments.

```rust
use alloy_ui::ipc::{Capability, CapabilitySet, CommandError, CommandRegistry};
use serde_json::json;

let mut commands = CommandRegistry::new();
commands.register("read_len", &[Capability::FsRead], |path: String| async move {
    Ok::<_, String>(path.len())
});

# let home = if cfg!(windows) { "C:/home/me" } else { "/home/me" };
let granted = CapabilitySet::from_toml(&format!(
    "capabilities = [\"fs_read\"]\n[fs]\nread = [\"{home}\"]"
))
.unwrap();

let path = format!("{home}/a.txt");
let fut = commands.invoke("read_len", json!(path), &granted).unwrap();
assert_eq!(alloy_ui::block_on(fut).unwrap(), json!(path.len()));

// Without the grant, the call is refused before the handler runs.
let denied = commands.invoke("read_len", json!("x"), &CapabilitySet::none());
assert!(matches!(denied, Err(CommandError::Denied { .. })));
```

## Background tasks

`TaskRunner<M>` runs futures or blocking closures off the UI thread and wakes the window when a result is ready; `CancelToken` lets long work stop early. `CommandRegistry::invoke_on` runs a command on a runner and maps its result to your message type. In `sketch` and `App` apps you rarely need it directly: use [`resource`](06-async-resources.md) or `Cmd::perform`.

`alloy_ui::ipc` re-exports the whole `alloy-ui-ipc` crate.
