# alloy-ui-cli

The `alloy` tool: builds an Alloy app in release mode and packages it.

```
alloy new my-app                  # starter counter app (add --alloy-path <checkout> for a local Alloy)
alloy dev                         # run it; ALLOY_CAPABILITIES points at its capability file
alloy bundle                      # host defaults: app,dmg | deb,appimage | exe,nsis
alloy bundle --format deb,rpm --target x86_64-unknown-linux-gnu
alloy check                       # validate metadata, icons and capability config
```

## App metadata

In the app's `Cargo.toml`:

```toml
[package.metadata.alloy]
identifier   = "dev.example.counter"          # required, reverse-DNS
product-name = "Counter"                      # default: package name
publisher    = "Example Inc."
category     = "Utility"                      # freedesktop main category
icons        = ["assets/icon-256.png", "assets/icon-512.png"]   # png, icns, ico
capabilities = "alloy.capabilities.toml"      # the alloy-ui-ipc config
```

The capability file is parsed and validated with `alloy-ui-ipc` before anything
is packaged (a bad file fails the bundle), and copied verbatim into the bundle:

| Format | Location |
|---|---|
| `.app` | `Contents/Resources/alloy.capabilities.toml` |
| `.deb`, AppImage | `usr/share/<bin>/alloy.capabilities.toml` |
| `.exe`, NSIS | next to the executable |

## Formats and what they need

| Format | Built by | Host | External tool |
|---|---|---|---|
| `app` | own code (Info.plist, icns from PNG); ad-hoc `codesign` on macOS, `ALLOY_SIGN_IDENTITY` to override | any (signs only on macOS) | none |
| `dmg` | `hdiutil` | macOS | hdiutil |
| `deb` | own `ar`+`tar.gz` writer, reproducible (`SOURCE_DATE_EPOCH`) | any | none |
| `rpm` | own header/cpio writer with SHA-256 header and payload digests, reproducible; `rpm-requires` sets dependencies (Fedora names by default) | any | none |
| `appimage` | AppDir by own code, squashfs by `appimagetool` | Linux | `appimagetool` or `ALLOY_APPIMAGETOOL` |
| `exe` | copy, with the icon embedded by own PE `.rsrc` writer (`pe.rs`) | any | none |
| `nsis` | generated script, per-user install, Start menu entry, uninstaller | any | `makensis` or `ALLOY_MAKENSIS` |
| `msi` | generated WiX v4 source, per-user, stable upgrade code | any | `wix` 4+ (`dotnet tool install --global wix`) or `ALLOY_WIX` |

Limits: icon embedding needs a PE with no existing resources or overlay (otherwise
it warns and keeps the original binary); macOS notarisation is not done; RPM/MSI
are unsigned; `.icns` is built only from PNGs of
exactly 16/32/64/128/256/512/1024 px.

## Verification status

- Verified on Linux: unit tests, and an end-to-end test that builds a real app
  crate, bundles a `.deb` (checked with `dpkg-deb -I/-c/-x`) and an `.rpm`
  (checked with `rpm -qp`, `rpm -K` digests, `rpm2cpio` extraction), runs the
  extracted binaries, and compiles an NSIS installer with real `makensis`.
  `alloy new` output was `cargo check`ed against the workspace.
- CI-only (workflow `alloy-cli.yml`): macOS `.app` (codesign verify, plutil lint,
  runs the binary) and `.dmg`; Windows `.exe` run with the embedded icon, NSIS and MSI compile; AppImage
  build and run on Ubuntu.
