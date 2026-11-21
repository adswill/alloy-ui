# Bundling

`alloy bundle` builds a release binary and packages it for the platform you are on.

```sh
alloy bundle                       # the default formats for this OS
alloy bundle --format deb,rpm      # pick formats
alloy bundle --target x86_64-pc-windows-msvc --format exe
```

| format | what you get | needs |
|---|---|---|
| `app` | macOS `.app`, signed (ad hoc, or Developer ID and notarised, see [shipping](14-shipping.md)) | macOS |
| `dmg` | macOS disk image | macOS, `hdiutil` |
| `deb` | Debian/Ubuntu package, built in pure Rust | nothing extra |
| `rpm` | Fedora/RHEL/openSUSE package, built in pure Rust | nothing extra |
| `appimage` | Linux AppImage | `appimagetool` (or `ALLOY_APPIMAGETOOL`) |
| `exe` | Windows portable executable, icon embedded | nothing extra |
| `nsis` | Windows installer | `makensis` |
| `msi` | Windows MSI installer | WiX 4+ (`wix`, or `ALLOY_WIX`) |
| `msix` | Windows MSIX package, written in Rust, signed when `ALLOY_WIN_CERT` is set | nothing extra |
| `flatpak` | Flatpak bundle, plus the `flatpak-builder` source directory | `flatpak-builder` |

Other options: `--out <dir>` (default `target/alloy-bundle`), `--binary <path>` to package a prebuilt binary, `--skip-build`, `-p <package>` and `--bin <name>` in a workspace.

## Metadata

Your `Cargo.toml` carries the app's identity:

```toml
[package.metadata.alloy]
identifier = "com.example.hello"
product-name = "Hello"
icons = ["assets/icon-256.png", "assets/icon-512.png"]
capabilities = "alloy.capabilities.toml"
```

`alloy check` validates this table, the icons and the capability file. See [capabilities](11-capabilities-and-ipc.md).

## Small by default

Use the size-optimised release profile that `alloy new` writes:

```toml
[profile.release]
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

Measured on Linux, the counter binary is 7.5 MB and its `.deb` and `.rpm` are 3.2 MB (see [BENCHMARKS.md](../../BENCHMARKS.md) for the method). Idle CPU is 0% because nothing is redrawn until something changes. Bundling has been run on Linux; the macOS and Windows formats compile and are exercised by CI, which had not completed when this was written. Signing, notarisation, signed updates and crash reports are in [Shipping](14-shipping.md).
