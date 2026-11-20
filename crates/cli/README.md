# alloy-ui-cli

The `alloy` tool: builds an Alloy app in release mode and packages it.

```
alloy bundle                      # host defaults: app,dmg | deb,appimage | exe,nsis
alloy bundle --format deb,appimage --target x86_64-unknown-linux-gnu
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
| `appimage` | AppDir by own code, squashfs by `appimagetool` | Linux | `appimagetool` or `ALLOY_APPIMAGETOOL` |
| `exe` | copy | any | none |
| `nsis` | generated script, per-user install, Start menu entry, uninstaller | any | `makensis` or `ALLOY_MAKENSIS` |

Limits: the Windows `.exe` icon is not embedded (the installer and shortcut use
the `.ico`); macOS notarisation is not done; `.icns` is built only from PNGs of
exactly 16/32/64/128/256/512/1024 px.

## Verification status

- Verified on Linux: unit tests, and an end-to-end test that builds a real app
  crate, bundles a `.deb`, checks it with `dpkg-deb -I/-c/-x` and runs the
  extracted binary.
- CI-only (workflow `alloy-cli.yml`): macOS `.app` (codesign verify, plutil lint,
  runs the binary) and `.dmg`; Windows `.exe` run and NSIS compile; AppImage
  build and run on Ubuntu.
