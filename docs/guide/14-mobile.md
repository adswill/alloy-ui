# Mobile: Android and iOS

The same app code runs on phones and tablets. This chapter is a prototype: the code paths are written and cross-compiled, but nothing has run on a device yet. [What is verified](#what-is-verified) says exactly what was and was not tried.

## Make an app mobile-ready

Three small changes to a normal Alloy crate (`mobile/counter` is a working example):

```toml
[lib]
name = "my_app"
crate-type = ["cdylib", "staticlib", "rlib"]   # Android loads the cdylib, iOS links the staticlib

[package.metadata.alloy]
identifier = "com.example.myapp"

[package.metadata.alloy.mobile]
orientation = "portrait"         # portrait | landscape | any (default)
android-min-sdk = 26
android-target-sdk = 35
ios-deployment-target = "15.0"
```

```no_run
use alloy_ui::prelude::*;

pub fn run() {
    sketch(0i64, |n| {
        col![
            text(n.to_string()).display(),
            button("Add one").primary().fill_w().act(|n| *n += 1),
        ]
        .gap(12.0)
        .pad_safe(24.0)          // padding plus the notch and system bars
        .fill()
    })
    .run("My app");
}

// Android's `android_main` and iOS's `alloy_ios_main`; nothing on the desktop.
alloy_ui::mobile_entry!(run);
# fn main() { run() }
```

The same crate still builds and runs on the desktop (`cargo run`), which is the fast way to iterate on layout.

## Build

```sh
alloy bundle --target android     # aarch64-linux-android, writes a Gradle project
alloy bundle --target ios         # aarch64-apple-ios, writes an XcodeGen project
alloy bundle --target ios-sim     # simulator build (Apple silicon)
alloy build  --target android     # just the library
```

`--target` also takes a full triple (`x86_64-linux-android` for the emulator). The output lands in `target/alloy-bundle/<triple>/android` or `/ios`, each with a README listing the next command.

| | needs | produces |
|---|---|---|
| Android | the Android NDK (`ANDROID_NDK_HOME`, or `$ANDROID_HOME/ndk/*`); `rustup target add aarch64-linux-android`; for the APK, the Android SDK and Gradle | `liby.so` in `jniLibs/<abi>/`, `AndroidManifest.xml` (the system `NativeActivity`, no Java or Kotlin), Gradle files |
| iOS | `rustup target add aarch64-apple-ios`; for the app, a Mac with Xcode and [XcodeGen](https://github.com/yonaskolb/XcodeGen) | `libx.a`, `project.yml`, `main.m`, an asset catalog when a 1024 px icon is configured |

The library is built with `cargo rustc --crate-type cdylib` (Android) or `staticlib` (iOS), so the iOS build never tries to link a dynamic library. The staticlib can be built on Linux; only turning it into an `.app` needs Xcode.

## Touch

The shell turns the first finger into the mouse, so every widget works by touch with no extra code:

| Finger | Becomes |
|---|---|
| tap | left click at that point |
| drag (past 10 px) | scrolling; the content follows the finger, and a flick keeps going and slows down |
| hold (0.5 s) | right click, for context menus |
| second finger | cancels the click; the pair is reported as a pinch |

A drag never ends as a click: the pointer leaves before the button is released. Apps that want more listen for `PlatformEvent::Gesture` (`Tap`, `DoubleTap`, `LongPress`, `Pan`, `PanEnd` with the release velocity, `Pinch` with a scale and a centre) and the raw `PlatformEvent::Touch`. The recognizer is plain Rust with unit tests, so it also drives touch screens on laptops.

## Safe areas, rotation and the keyboard

* `safe_area()` returns the margins that system chrome may cover, in logical pixels (zero on desktop). `pad_safe(p)` adds them to a padding. The view is rebuilt when they change.
* On Android the margins come from the window's content rectangle; the generated manifest uses `adjustResize`, so that rectangle also excludes the soft keyboard and content stays above it. Display cutouts are only covered to the extent Android reports them there.
* On iOS they come from `UIView.safeAreaInsets`. Keyboard avoidance is **not** done yet on iOS (the keyboard overlaps the content).
* Rotation arrives as a resize; the manifest and `Info.plist` list the orientations you allowed.
* Phones get the `Spacious` spacing preset automatically, for larger touch targets.

Text fields show the soft keyboard through the same call as desktop IME (`ShellContext::set_ime_allowed`, also available as `set_soft_keyboard`). Typed characters and Backspace arrive as normal key events. Composition (predictive text, CJK input) is limited by what the underlying window library exposes on mobile; treat it as unfinished.

## Lifecycle

Android destroys the native window whenever the app goes to the background and gives a new one on return. The shell reports `PlatformEvent::Suspended` (drop GPU surfaces) and `PlatformEvent::Resumed` (rebuild); the facade does both for you, so an app only notices a short redraw. iOS windows survive backgrounding and nothing is sent. Settings saved with `.persist()` go to the app's private storage on Android.

## Not there yet

* Menus, tray icons, native file pickers and message boxes report an error on mobile (use in-window UI). The Android clipboard is in-process only; the iOS clipboard uses `UIPasteboard`.
* Screen readers (TalkBack, VoiceOver) are not connected: the tree is built, the bridges are not.
* Haptics, share sheets, permissions prompts, push notifications and store signing.

## What is verified

| Item | Status |
|---|---|
| Touch recognizer, pointer emulation, safe-area maths, scaffold generation, manifest and project text | unit-tested, run on Linux |
| Whole stack (`alloy-ui`, wgpu, text) type-checks and passes clippy `-D warnings` for `aarch64-linux-android` and `aarch64-apple-ios` | run on Linux |
| `aarch64-apple-ios` static library of the counter (`alloy bundle --target ios`) | built on Linux, 374 objects; never linked into an app |
| `aarch64-linux-android` shared library of the counter | **linked** with `rust-lld` against empty stub libraries, because the NDK could not be downloaded in the sandbox; exports `android_main`. It is not loadable on a device (the NDK's unwinder is missing), so it proves the code and the link, not a run |
| Android APK, `gradle assembleDebug`, emulator or phone | not run |
| Xcode project, iOS simulator or device | not run |
| Real touch input, safe areas, rotation, keyboard, suspend and resume | not run |

Next step for whoever has the tools: install the NDK and `alloy bundle --target android`, then `gradle assembleDebug` in the output folder and run it on an emulator; on a Mac, `alloy bundle --target ios-sim`, `xcodegen generate`, run in the simulator. Please report what breaks.
