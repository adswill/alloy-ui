# Shipping

Everything here goes through `alloy bundle` and environment variables. Nothing secret lives in `Cargo.toml`.

What has really been run, and where, is in the table at the end. Read it before you rely on a platform.

## Metadata

```toml
[package.metadata.alloy]
identifier = "com.example.hello"
publisher = "Example Inc."
update-url = "https://downloads.example.com/hello"           # where you host releases
msix-publisher = "CN=Example Inc., O=Example Inc., C=US"     # must equal your certificate subject
mac-entitlements = "mac.entitlements"                        # optional
flatpak-runtime-version = "24.08"                            # optional
flatpak-finish-args = ["--share=network"]                    # optional, replaces the derived list
```

## macOS: sign and notarise

```sh
export ALLOY_SIGN_IDENTITY="Developer ID Application: Example Inc. (ABCDE12345)"
export ALLOY_NOTARY_PROFILE=alloy        # made once with: xcrun notarytool store-credentials alloy
alloy bundle --format app,dmg
```

Without `ALLOY_SIGN_IDENTITY` the `.app` gets an ad hoc signature, enough to run on your own machine. With it, `alloy` signs with the hardened runtime and a secure timestamp (and your entitlements file if set), zips the app with `ditto`, submits it with `notarytool --wait`, and staples the ticket. The `.dmg` is signed too.

Other ways to log in to the notary service: `ALLOY_NOTARY_APPLE_ID` + `ALLOY_NOTARY_TEAM_ID` + `ALLOY_NOTARY_PASSWORD` (an app-specific password), or an App Store Connect API key with `ALLOY_NOTARY_KEY` (the `.p8` path) + `ALLOY_NOTARY_KEY_ID` + `ALLOY_NOTARY_ISSUER`. Notarising with an ad hoc identity is refused with an error that says so.

## Windows: MSI, MSIX and signing

`msi` needs WiX 4+. `msix` needs nothing: the package is written in Rust, so it builds on any host. An unsigned MSIX will not install, so sign it:

```sh
set ALLOY_WIN_CERT=C:\keys\example.pfx
set ALLOY_WIN_CERT_PASSWORD=...
alloy bundle --format exe,msi,msix
```

Every `.exe`, `.msi`, `-setup.exe` and `.msix` is signed with `signtool` (or `osslsigncode`), timestamped from `ALLOY_WIN_TIMESTAMP_URL` (default DigiCert). A certificate in the Windows store works with `ALLOY_WIN_CERT_SHA1=<thumbprint>`. For Azure Trusted Signing, an HSM or any other service, set `ALLOY_WIN_SIGN_COMMAND` to a command containing `{file}`, for example `ALLOY_WIN_SIGN_COMMAND="azuresigntool sign ... {file}"`.

`msix-publisher` must match the certificate's subject exactly or Windows rejects the signature. The three tile logos are all your largest PNG icon; add your own sizes when you publish to the Store.

## Linux: AppImage and Flatpak

`appimage` calls `appimagetool`. `flatpak` always writes `<name>-flatpak/` (JSON manifest, `.desktop`, AppStream metainfo, icons, your binary) and, if `flatpak-builder` is installed, builds `<name>.flatpak`:

```sh
flatpak install flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08
alloy bundle --format flatpak
flatpak install --user target/alloy-bundle/*/hello-*.flatpak
```

The sandbox permissions come from your capability file: `network` adds `--share=network`, `fs_write` adds `--filesystem=home`, `fs_read` adds `--filesystem=home:ro`, `notifications` adds the notification service. File dialogs go through the desktop portal and need no static access. The binary is built outside the sandbox and must run on the Flatpak runtime's glibc, so build it on an older distribution such as Ubuntu 22.04.

## Auto-update

An update is a signed `latest.json` next to your installers. The client downloads it, checks the Ed25519 signature before parsing anything, checks the manifest is for your app and newer than the running version (no downgrades), and checks the downloaded file against the size and SHA-256 inside the signed manifest. A stolen download host can serve the files but cannot make a client accept a changed one.

```sh
alloy keygen --out alloy-update.key     # once; prints the public key, never commit the key file
ALLOY_UPDATE_KEY=@alloy-update.key alloy bundle
```

With `update-url` set, each bundle run writes `<name>-<os>-<arch>.update.json` next to the installers. When a release is built on several machines, collect those files in one folder and run `ALLOY_UPDATE_KEY=@alloy-update.key alloy release --dir dist --notes-file CHANGES.txt`. That writes `dist/latest.json` and `dist/latest.json.sig`. Upload both with the installers to `update-url`.

Add `alloy-ui-ship` to your app and check on start or from a menu item:

```rust,no_run
use alloy_ui_ship::update::{apply, cleanup_old, Config, Updater};
use alloy_ui_ship::UreqFetch;

fn check_for_updates() -> alloy_ui_ship::Result<()> {
    cleanup_old(); // removes the .old file a Windows update leaves behind
    let cfg = Config::new(
        "com.example.hello",
        env!("CARGO_PKG_VERSION"),
        "https://downloads.example.com/hello/latest.json",
        "7780b22eb7666158ddf7d424642842040a3bf3308c3b1cbe5dfd4e2f68c8045a", // from `alloy keygen`
    );
    let updater = Updater::new(cfg, UreqFetch::default());
    if let Some(update) = updater.check()? {
        // Ask the user first; show update.version and update.notes.
        let file = updater.download(&update, &std::env::temp_dir())?;
        apply(&file, &update.artifact)?;
    }
    Ok(())
}
# check_for_updates().ok();
```

What `apply` does depends on how the app was installed:

| install | update |
|---|---|
| AppImage | the file named by `$APPIMAGE` is replaced in place; restart to use it |
| macOS | the `.dmg` is opened in Finder |
| Windows | the `.msi` is run with `msiexec` (its upgrade code replaces the old version) |
| `.deb`, `.rpm`, Flatpak | not offered: `check` returns `None`, the package manager updates these |

Run `check` on a background thread (`alloy_ui::resource` fits) and never block the UI on it. The default HTTP client is `UreqFetch` (rustls); to use your own, implement the two-method `Fetch` trait, or build with `default-features = false`.

## Crash reports

```rust,no_run
use alloy_ui_ship::crash;

fn main() {
    let _crashes = crash::install(
        crash::Config::new("com.example.hello", env!("CARGO_PKG_VERSION"))
            .scrub(|s| s.replace("/home/", "/home/<user>/")),
    );
    // On the next start, after the user agrees:
    // crash::upload_pending(&crash::default_dir("com.example.hello"),
    //     "https://crash.example.com/report", &alloy_ui_ship::UreqFetch::default());
}
```

A panic on any thread writes a JSON report (message, location, backtrace, version, OS, CPU, time) to `<state dir>/<app id>/crashes`, keeps the newest ten, and then runs the previous panic hook, so the normal message still reaches stderr. It works with `panic = "abort"` too, because the hook runs before the abort. Nothing is ever sent unless your code calls `upload_pending`; ask the user first, and use `scrub` because panic messages can contain their data. Native faults such as a driver segfault are not caught. For readable backtraces use `debug = "line-tables-only"` and `strip = "debuginfo"` in the release profile instead of `strip = true`.

## What has been run

| thing | status |
|---|---|
| signed manifest: sign, verify, tamper, wrong key, wrong app, downgrade, replay of a changed file | unit-tested; the Ed25519 implementation matches RFC 8032 test vector 1, and `openssl` verified a manifest signed by `alloy` |
| update check, download, hash check and file replacement | tested over a real local HTTP server on Linux, and end to end in a real process on **Linux, macOS and Windows** runners (`update-e2e`, CI run on `main`, 2026-11-21) |
| crash hook and upload | tested (panic on a named thread, scrub, retention, upload over HTTP) on Linux; the real-process crash report also ran on macOS and Windows runners (`update-e2e`) |
| MSIX | built on Linux and checked by an independent script; **built with a throwaway self-signed certificate, installed with `Add-AppxPackage`, registered and removed on a Windows runner** (`windows-installers`). The certificate is not trusted outside that runner, so this proves the package format, not a store or user install |
| Flatpak | manifest, metainfo and layout tested; **built with `flatpak-builder`, installed for the user, `/app/bin/counter` found inside the sandbox and uninstalled on an Ubuntu runner** (`linux-packages`). A headless frame from inside the sandbox is a soft check, reported not required |
| AppImage | AppDir layout tested; **built with `appimagetool`, extracted and run, it rendered a frame, on an Ubuntu runner** (`linux-packages`) |
| macOS bundle (`.app` and `.dmg`), ad hoc signature | **built on a macOS runner: the ad hoc signature verifies, the plist lints, the `.dmg` mounts and the app inside it renders a frame** (`macos-bundle`) |
| macOS signing with a Developer ID and notarisation | command lines unit-tested; **never run against Apple's tools or service** (the `macos-notarize` job is skipped until Apple credentials are added as secrets) |
| Windows signing | command lines for `signtool`, `osslsigncode` and custom commands unit-tested; **never run** |
| MSI and NSIS installers | `.wxs` generation tested; **WiX 5 and NSIS built them on a Windows runner, which silently installed and uninstalled both** (`windows-installers`) |
