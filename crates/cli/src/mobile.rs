//! `alloy build|bundle --target android|ios`: cross-compile the app's
//! `cdylib`/`staticlib` and lay out the native project that wraps it.
//!
//! * **Android**: a Gradle project whose only activity is the system's
//!   `NativeActivity`; it loads `lib<name>.so` from `jniLibs/`. No Java or
//!   Kotlin is written, so the scaffold is a manifest, two Gradle files and
//!   the library.
//! * **iOS**: an XcodeGen `project.yml`, a six-line `main.m` that calls
//!   `alloy_ios_main` and the static library. XcodeGen (or a hand-made Xcode
//!   project) turns that into an `.app`.
//!
//! Scaffolding is pure Rust and runs on any host, so it is unit-tested here.
//! Compiling needs the target's toolchain: the Android NDK (found through
//! `ANDROID_NDK_HOME`) or Xcode on a Mac.

use crate::bundle::write_file;
use crate::error::{io, Error, Result};
use crate::icon::IconSet;
use crate::project::Project;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Mobile OS family of a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    /// Android (`*-linux-android*`).
    Android,
    /// iOS and its simulator (`*-apple-ios*`).
    Ios,
}

/// Map a `--target` value to a Rust triple: `android` and `ios` are shorthands.
pub fn expand_target(t: &str) -> String {
    match t {
        "android" => "aarch64-linux-android".into(),
        "ios" => "aarch64-apple-ios".into(),
        "ios-sim" => "aarch64-apple-ios-sim".into(),
        other => other.into(),
    }
}

/// Which mobile OS `triple` is for, if any.
pub fn os_of(triple: &str) -> Option<Os> {
    if triple.contains("android") {
        Some(Os::Android)
    } else if triple.contains("apple-ios") {
        Some(Os::Ios)
    } else {
        None
    }
}

/// Android ABI directory for `triple`.
pub fn android_abi(triple: &str) -> Result<&'static str> {
    Ok(match triple {
        "aarch64-linux-android" => "arm64-v8a",
        "armv7-linux-androideabi" => "armeabi-v7a",
        "x86_64-linux-android" => "x86_64",
        "i686-linux-android" => "x86",
        other => {
            return Err(Error::Usage(format!(
                "unknown Android target `{other}`; use aarch64-linux-android, \
                 armv7-linux-androideabi, x86_64-linux-android or i686-linux-android"
            )))
        }
    })
}

/// Java package for an Android application id: `-` is not legal, and a
/// segment may not start with a digit.
pub fn android_package(identifier: &str) -> String {
    identifier
        .split('.')
        .map(|seg| {
            let seg = seg.replace('-', "_");
            if seg.starts_with(|c: char| c.is_ascii_digit()) {
                format!("a{seg}")
            } else {
                seg
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---- Android --------------------------------------------------------------

/// `AndroidManifest.xml` for a `NativeActivity` app.
pub fn android_manifest(p: &Project, has_icon: bool) -> String {
    let orientation = match p.mobile.orientation.as_str() {
        "portrait" => "portrait",
        "landscape" => "landscape",
        _ => "unspecified",
    };
    let lib = p.lib_name.clone().unwrap_or_else(|| p.bin_name.clone());
    let icon = if has_icon {
        "\n        android:icon=\"@mipmap/ic_launcher\""
    } else {
        ""
    };
    // adjustResize shrinks the window above the soft keyboard, so the content
    // rect (and `safe_area()`) already excludes it. configChanges keeps the
    // activity alive across rotation: Alloy handles the resize itself.
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <!-- Alloy draws through Vulkan or GLES. -->
    <uses-feature android:glEsVersion="0x00030000" android:required="true" />
    <application
        android:label="{label}"
        android:hasCode="false"{icon}
        android:allowBackup="true">
        <activity
            android:name="android.app.NativeActivity"
            android:exported="true"
            android:label="{label}"
            android:screenOrientation="{orientation}"
            android:windowSoftInputMode="adjustResize"
            android:configChanges="orientation|keyboardHidden|keyboard|screenSize|smallestScreenSize|screenLayout|uiMode|density">
            <meta-data android:name="android.app.lib_name" android:value="{lib}" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
"#,
        label = esc(&p.product_name),
        lib = esc(lib.trim_start_matches("lib")),
    )
}

/// `app/build.gradle`.
pub fn android_app_gradle(p: &Project) -> String {
    format!(
        r#"plugins {{
    id 'com.android.application'
}}

android {{
    namespace '{pkg}'
    compileSdk {target}
    defaultConfig {{
        applicationId '{pkg}'
        minSdk {min}
        targetSdk {target}
        versionCode 1
        versionName '{version}'
    }}
    buildTypes {{
        release {{
            // Sign with your own keystore for the store; debug builds are signed automatically.
            minifyEnabled false
        }}
    }}
}}
"#,
        pkg = android_package(&p.identifier),
        target = p.mobile.android_target_sdk,
        min = p.mobile.android_min_sdk,
        version = p.version,
    )
}

const ANDROID_ROOT_GRADLE: &str =
    "plugins {\n    id 'com.android.application' version '8.7.3' apply false\n}\n";

fn android_settings(p: &Project) -> String {
    format!(
        "pluginManagement {{\n    repositories {{ google(); mavenCentral(); gradlePluginPortal() }}\n}}\n\
         dependencyResolutionManagement {{\n    repositories {{ google(); mavenCentral() }}\n}}\n\
         rootProject.name = '{}'\ninclude ':app'\n",
        p.package_name.replace('\'', "")
    )
}

fn android_readme(p: &Project, abis: &[&str]) -> String {
    format!(
        "# {name} for Android\n\n\
         Generated by `alloy bundle --target android`. Contains: {abis}.\n\n\
         Build and install (needs the Android SDK, ANDROID_HOME, and Gradle 8.9+):\n\n\
         ```sh\ngradle assembleDebug\nadb install -r app/build/outputs/apk/debug/app-debug.apk\n```\n\n\
         The activity is the system's NativeActivity; it loads `lib{lib}.so` from \
         `app/src/main/jniLibs/<abi>/`. Rebuild the library with \
         `alloy bundle --target android` after code changes.\n",
        name = p.product_name,
        abis = abis.join(", "),
        lib = p.lib_name.clone().unwrap_or_else(|| p.bin_name.clone()),
    )
}

/// Write the Gradle project into `dir`. `libs` maps an ABI directory name to
/// the compiled library for it. Returns `dir`.
pub fn scaffold_android(
    p: &Project,
    icons: &IconSet,
    libs: &[(&str, PathBuf)],
    dir: &Path,
) -> Result<PathBuf> {
    let main = dir.join("app/src/main");
    let has_icon = icons.largest_png().is_some();
    write_file(&dir.join("settings.gradle"), android_settings(p).as_bytes())?;
    write_file(&dir.join("build.gradle"), ANDROID_ROOT_GRADLE.as_bytes())?;
    write_file(
        &dir.join("gradle.properties"),
        b"org.gradle.jvmargs=-Xmx2g\nandroid.useAndroidX=true\n",
    )?;
    write_file(
        &dir.join("app/build.gradle"),
        android_app_gradle(p).as_bytes(),
    )?;
    write_file(
        &main.join("AndroidManifest.xml"),
        android_manifest(p, has_icon).as_bytes(),
    )?;
    if let Some(icon) = icons.largest_png() {
        // One density; Android scales it. Ship per-density art for the store.
        write_file(&main.join("res/mipmap-xxxhdpi/ic_launcher.png"), &icon.data)?;
    }
    let mut abis = Vec::new();
    for (abi, lib) in libs {
        let name = lib
            .file_name()
            .ok_or_else(|| Error::Config(format!("bad library path {}", lib.display())))?;
        let to = main.join("jniLibs").join(abi).join(name);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .map_err(io(format!("creating {}", parent.display())))?;
        }
        std::fs::copy(lib, &to).map_err(io(format!("copying {}", lib.display())))?;
        abis.push(*abi);
    }
    write_file(&dir.join("README.md"), android_readme(p, &abis).as_bytes())?;
    Ok(dir.to_path_buf())
}

// ---- iOS ------------------------------------------------------------------

/// `main.m`: UIKit's entry point lives in `winit`, started by the Rust library.
pub const IOS_MAIN_M: &str = "// Generated by `alloy bundle --target ios`.\n\
extern void alloy_ios_main(void);\n\n\
int main(int argc, char *argv[]) {\n    @autoreleasepool {\n        alloy_ios_main();\n    }\n    return 0;\n}\n";

fn ios_orientations(o: &str) -> Vec<&'static str> {
    match o {
        "portrait" => vec!["UIInterfaceOrientationPortrait"],
        "landscape" => vec![
            "UIInterfaceOrientationLandscapeLeft",
            "UIInterfaceOrientationLandscapeRight",
        ],
        _ => vec![
            "UIInterfaceOrientationPortrait",
            "UIInterfaceOrientationLandscapeLeft",
            "UIInterfaceOrientationLandscapeRight",
        ],
    }
}

/// XcodeGen spec (`project.yml`) for the app.
pub fn ios_project_yml(p: &Project, lib_file: &str, has_icon: bool) -> String {
    let target: String = p
        .product_name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let target = if target.is_empty() {
        "App".to_string()
    } else {
        target
    };
    let lib = lib_file
        .trim_start_matches("lib")
        .trim_end_matches(".a")
        .to_string();
    let orient: String = ios_orientations(&p.mobile.orientation)
        .iter()
        .map(|o| format!("          - {o}\n"))
        .collect();
    let icon_setting = if has_icon {
        "        ASSETCATALOG_COMPILER_APPICON_NAME: AppIcon\n"
    } else {
        ""
    };
    let sources = if has_icon {
        "    sources:\n      - Sources\n      - Assets.xcassets\n"
    } else {
        "    sources:\n      - Sources\n"
    };
    format!(
        r#"# Generated by `alloy bundle --target ios`. Run `xcodegen generate` next to this file.
name: {target}
options:
  deploymentTarget:
    iOS: "{min}"
targets:
  {target}:
    type: application
    platform: iOS
{sources}    settings:
      base:
        PRODUCT_BUNDLE_IDENTIFIER: {id}
        PRODUCT_NAME: "{name}"
        MARKETING_VERSION: "{version}"
        CURRENT_PROJECT_VERSION: "1"
        LIBRARY_SEARCH_PATHS: "$(PROJECT_DIR)/Libs"
        OTHER_LDFLAGS: ["-l{lib}", "-framework", "UIKit", "-framework", "Foundation", "-framework", "CoreGraphics", "-framework", "QuartzCore", "-framework", "Metal", "-framework", "CoreText", "-framework", "CoreFoundation", "-lc++"]
        ENABLE_BITCODE: "NO"
{icon_setting}    info:
      path: Sources/Info.plist
      properties:
        CFBundleDisplayName: "{name}"
        UILaunchScreen: {{}}
        UIRequiresFullScreen: true
        UISupportedInterfaceOrientations:
{orient}"#,
        min = p.mobile.ios_deployment_target,
        id = p.identifier,
        name = p.product_name.replace('"', ""),
        version = p.version,
    )
}

/// Write the XcodeGen project into `dir`. `lib` is the compiled static library.
pub fn scaffold_ios(p: &Project, icons: &IconSet, lib: &Path, dir: &Path) -> Result<PathBuf> {
    let file = lib
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| Error::Config(format!("bad library path {}", lib.display())))?;
    let has_icon = icons.pngs.iter().any(|i| i.size == 1024);
    write_file(
        &dir.join("project.yml"),
        ios_project_yml(p, file, has_icon).as_bytes(),
    )?;
    write_file(&dir.join("Sources/main.m"), IOS_MAIN_M.as_bytes())?;
    if let Some(icon) = icons.pngs.iter().find(|i| i.size == 1024) {
        write_file(
            &dir.join("Assets.xcassets/AppIcon.appiconset/icon-1024.png"),
            &icon.data,
        )?;
        write_file(
            &dir.join("Assets.xcassets/AppIcon.appiconset/Contents.json"),
            br#"{"images":[{"filename":"icon-1024.png","idiom":"universal","platform":"ios","size":"1024x1024"}],"info":{"author":"alloy","version":1}}"#,
        )?;
    }
    let to = dir.join("Libs").join(file);
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(io(format!("creating {}", parent.display())))?;
    }
    std::fs::copy(lib, &to).map_err(io(format!("copying {}", lib.display())))?;
    write_file(
        &dir.join("README.md"),
        format!(
            "# {name} for iOS\n\nGenerated by `alloy bundle --target ios`.\n\n\
             On a Mac with Xcode and [XcodeGen](https://github.com/yonaskolb/XcodeGen):\n\n\
             ```sh\nxcodegen generate\nopen *.xcodeproj   # pick a team under Signing, then Run\n```\n\n\
             `Libs/` holds the static library for the target it was built for \
             (device `aarch64-apple-ios`, simulator `aarch64-apple-ios-sim`); rebuild with \
             `alloy bundle --target ios` or `--target ios-sim` after code changes.\n",
            name = p.product_name
        )
        .as_bytes(),
    )?;
    Ok(dir.to_path_buf())
}

// ---- building -------------------------------------------------------------

/// Locate the Android NDK's clang for `triple`, from `ANDROID_NDK_HOME`,
/// `ANDROID_NDK_ROOT` or the newest `$ANDROID_HOME/ndk/*`.
pub fn ndk_clang(triple: &str, api: u32) -> Option<PathBuf> {
    let root = ["ANDROID_NDK_HOME", "ANDROID_NDK_ROOT"]
        .iter()
        .find_map(|k| std::env::var_os(k).map(PathBuf::from))
        .or_else(|| {
            let sdk = std::env::var_os("ANDROID_HOME")
                .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))?;
            let mut v: Vec<_> = std::fs::read_dir(PathBuf::from(sdk).join("ndk"))
                .ok()?
                .flatten()
                .map(|e| e.path())
                .collect();
            v.sort();
            v.pop()
        })?;
    let host = if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else if cfg!(windows) {
        "windows-x86_64"
    } else {
        "linux-x86_64"
    };
    // The clang wrapper name uses the Android triple, which spells armv7 differently.
    let t = if triple == "armv7-linux-androideabi" {
        "armv7a-linux-androideabi"
    } else {
        triple
    };
    let bin = root.join("toolchains/llvm/prebuilt").join(host).join("bin");
    let clang = bin.join(format!(
        "{t}{api}-clang{}",
        if cfg!(windows) { ".cmd" } else { "" }
    ));
    clang.is_file().then_some(clang)
}

/// `CARGO_TARGET_<TRIPLE>_LINKER` style variable name.
fn env_key(prefix: &str, triple: &str, upper: bool) -> String {
    let t = triple.replace('-', "_");
    if upper {
        format!("{prefix}_{}", t.to_uppercase())
    } else {
        format!("{prefix}_{t}")
    }
}

/// Where cargo puts the release library for `triple`.
pub fn lib_path(p: &Project, os: Os, triple: &str) -> Result<PathBuf> {
    let lib = p.lib_name.clone().ok_or_else(|| {
        Error::Config(format!(
            "package `{}` has no cdylib/staticlib target",
            p.package_name
        ))
    })?;
    let ext = if os == Os::Android { "so" } else { "a" };
    Ok(p.target_dir
        .join(triple)
        .join("release")
        .join(format!("lib{lib}.{ext}")))
}

/// Build the app's library for `triple` and return its path.
pub fn build_lib(p: &Project, os: Os, triple: &str, extra: &[String]) -> Result<PathBuf> {
    let lib = p.lib_name.clone().ok_or_else(|| {
        Error::Config(format!(
            "package `{}` has no cdylib/staticlib target; mobile builds need \
             `[lib] crate-type = [\"cdylib\", \"staticlib\", \"rlib\"]` (see mobile/counter)",
            p.package_name
        ))
    })?;
    let mut cmd = Command::new(crate::project::cargo());
    // One crate type per target: Android loads a cdylib, iOS links a staticlib.
    // (Building both would try to link a dylib for iOS, which needs Xcode.)
    let crate_type = if os == Os::Android {
        "cdylib"
    } else {
        "staticlib"
    };
    cmd.args(["rustc", "--release", "--lib", "--crate-type", crate_type])
        .arg("--manifest-path")
        .arg(&p.manifest_path)
        .args(["-p", &p.package_name, "--target", triple])
        .args(extra);
    if os == Os::Android {
        match ndk_clang(triple, p.mobile.android_min_sdk) {
            Some(clang) => {
                eprintln!("   using NDK clang {}", clang.display());
                cmd.env(env_key("CARGO_TARGET", triple, true) + "_LINKER", &clang);
                cmd.env(env_key("CC", triple, false), &clang);
            }
            None => eprintln!(
                "   warning: no Android NDK found (set ANDROID_NDK_HOME); \
                 relying on your cargo linker configuration for {triple}"
            ),
        }
    }
    eprintln!("   building {lib} (release, {triple})");
    let status = cmd.status().map_err(io("running cargo build"))?;
    if !status.success() {
        return Err(Error::CommandFailed {
            cmd: "cargo rustc --release --lib".into(),
            status: status.to_string(),
        });
    }
    let path = lib_path(p, os, triple)?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(Error::Config(format!(
            "build succeeded but {} does not exist",
            path.display()
        )))
    }
}

/// Build (unless `lib` is given) and scaffold the native project for `triple`
/// under `out_dir`. Returns the project directory.
pub fn bundle(
    p: &Project,
    icons: &IconSet,
    triple: &str,
    out_dir: &Path,
    prebuilt: Option<PathBuf>,
    extra: &[String],
) -> Result<PathBuf> {
    let os =
        os_of(triple).ok_or_else(|| Error::Usage(format!("`{triple}` is not a mobile target")))?;
    let lib = match prebuilt {
        Some(l) => l,
        None => build_lib(p, os, triple, extra)?,
    };
    if !lib.is_file() {
        return Err(Error::Config(format!(
            "library {} not found",
            lib.display()
        )));
    }
    match os {
        Os::Android => {
            let abi = android_abi(triple)?;
            scaffold_android(p, icons, &[(abi, lib)], &out_dir.join("android"))
        }
        Os::Ios => scaffold_ios(p, icons, &lib, &out_dir.join("ios")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Selector;
    use serde_json::json;

    fn proj(extra: serde_json::Value) -> Project {
        let mut alloy =
            json!({"identifier": "dev.alloy-ui.counter", "product-name": "Alloy Counter"});
        if let (Some(a), Some(e)) = (alloy.as_object_mut(), extra.as_object()) {
            for (k, v) in e {
                a.insert(k.clone(), v.clone());
            }
        }
        let m = json!({"target_directory": "/t", "packages": [{
            "name": "alloy-ui-mobile-counter", "version": "0.2.0", "manifest_path": "/a/Cargo.toml",
            "targets": [
                {"name": "alloy_counter", "kind": ["cdylib", "staticlib", "rlib"]},
                {"name": "mobile-counter", "kind": ["bin"]}],
            "metadata": {"alloy": alloy}}]});
        Project::from_metadata(&m, &Selector::default()).unwrap()
    }

    #[test]
    fn target_shorthands() {
        assert_eq!(expand_target("android"), "aarch64-linux-android");
        assert_eq!(expand_target("ios"), "aarch64-apple-ios");
        assert_eq!(
            expand_target("x86_64-unknown-linux-gnu"),
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(os_of("aarch64-linux-android"), Some(Os::Android));
        assert_eq!(os_of("aarch64-apple-ios-sim"), Some(Os::Ios));
        assert_eq!(os_of("aarch64-apple-darwin"), None);
        assert_eq!(android_abi("x86_64-linux-android").unwrap(), "x86_64");
        assert!(android_abi("riscv64-linux-android").is_err());
    }

    #[test]
    fn android_package_is_legal() {
        assert_eq!(
            android_package("dev.alloy-ui.counter"),
            "dev.alloy_ui.counter"
        );
        assert_eq!(android_package("com.1x.app"), "com.a1x.app");
    }

    #[test]
    fn metadata_defaults_and_validation() {
        let p = proj(json!({}));
        assert_eq!(p.lib_name.as_deref(), Some("alloy_counter"));
        assert_eq!(p.mobile.android_min_sdk, 26);
        assert_eq!(p.mobile.orientation, "any");
        let p = proj(json!({"mobile": {"orientation": "portrait", "android-min-sdk": 28}}));
        assert_eq!(
            (p.mobile.orientation.as_str(), p.mobile.android_min_sdk),
            ("portrait", 28)
        );
        let bad = json!({"target_directory": "/t", "packages": [{
            "name": "x", "version": "1.0.0", "manifest_path": "/a/Cargo.toml",
            "targets": [{"name": "x", "kind": ["bin"]}],
            "metadata": {"alloy": {"identifier": "a.b", "mobile": {"orientation": "sideways"}}}}]});
        assert!(Project::from_metadata(&bad, &Selector::default()).is_err());
    }

    #[test]
    fn manifest_has_native_activity_and_lib() {
        let p = proj(json!({"mobile": {"orientation": "portrait"}}));
        let x = android_manifest(&p, true);
        assert!(x.contains(r#"android:name="android.app.NativeActivity""#));
        assert!(x.contains(r#"android:name="android.app.lib_name" android:value="alloy_counter""#));
        assert!(x.contains(r#"android:screenOrientation="portrait""#));
        assert!(x.contains("adjustResize") && x.contains("hasCode=\"false\""));
        assert!(x.contains("@mipmap/ic_launcher"));
        assert!(!android_manifest(&p, false).contains("ic_launcher"));
        let g = android_app_gradle(&p);
        assert!(g.contains("applicationId 'dev.alloy_ui.counter'") && g.contains("minSdk 26"));
    }

    #[test]
    fn android_scaffold_layout() {
        let tmp = std::env::temp_dir().join(format!("alloy-mobile-a-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let so = tmp.join("liballoy_counter.so");
        std::fs::write(&so, b"\x7fELF").unwrap();
        let p = proj(json!({}));
        let dir = scaffold_android(
            &p,
            &IconSet::default(),
            &[("arm64-v8a", so)],
            &tmp.join("android"),
        )
        .unwrap();
        for f in [
            "settings.gradle",
            "build.gradle",
            "app/build.gradle",
            "app/src/main/AndroidManifest.xml",
            "app/src/main/jniLibs/arm64-v8a/liballoy_counter.so",
            "README.md",
        ] {
            assert!(dir.join(f).is_file(), "missing {f}");
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ios_scaffold_layout() {
        let tmp = std::env::temp_dir().join(format!("alloy-mobile-i-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let a = tmp.join("liballoy_counter.a");
        std::fs::write(&a, b"!<arch>\n").unwrap();
        let p =
            proj(json!({"mobile": {"orientation": "landscape", "ios-deployment-target": "16.0"}}));
        let y = ios_project_yml(&p, "liballoy_counter.a", false);
        assert!(y.contains("name: AlloyCounter"));
        // Indentation is the format: a flattened `settings:` would be silently wrong.
        assert!(y.contains("    sources:\n      - Sources\n    settings:\n      base:\n        PRODUCT_BUNDLE_IDENTIFIER"));
        assert!(y.contains("    info:\n      path: Sources/Info.plist\n      properties:\n"));
        assert!(y.contains("PRODUCT_BUNDLE_IDENTIFIER: dev.alloy-ui.counter"));
        assert!(y.contains("\"-lalloy_counter\""));
        assert!(y.contains("iOS: \"16.0\""));
        assert!(
            y.contains("UIInterfaceOrientationLandscapeLeft") && !y.contains("OrientationPortrait")
        );
        let dir = scaffold_ios(&p, &IconSet::default(), &a, &tmp.join("ios")).unwrap();
        for f in [
            "project.yml",
            "Sources/main.m",
            "Libs/liballoy_counter.a",
            "README.md",
        ] {
            assert!(dir.join(f).is_file(), "missing {f}");
        }
        assert!(std::fs::read_to_string(dir.join("Sources/main.m"))
            .unwrap()
            .contains("alloy_ios_main"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn env_keys() {
        assert_eq!(
            env_key("CARGO_TARGET", "aarch64-linux-android", true),
            "CARGO_TARGET_AARCH64_LINUX_ANDROID"
        );
        assert_eq!(
            env_key("CC", "aarch64-linux-android", false),
            "CC_aarch64_linux_android"
        );
    }
}
