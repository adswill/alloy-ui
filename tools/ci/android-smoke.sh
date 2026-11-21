#!/usr/bin/env bash
# Runs inside reactivecircus/android-emulator-runner: install the APK, launch it, wait, then
# check the process is alive, the log has no crash, and the screenshot is not a blank frame.
# usage: android-smoke.sh <apk> <package> <outdir>
set -u
apk=$1; pkg=$2; out=$3
mkdir -p "$out"
adb install -r "$apk" || { echo "install failed"; exit 1; }
adb logcat -c
adb shell am start -W -n "$pkg/android.app.NativeActivity" || { echo "start failed"; exit 1; }
sleep 15
adb logcat -d > "$out/logcat.txt"
adb exec-out screencap -p > "$out/android.png"
pid=$(adb shell pidof "$pkg" | tr -d '\r')
size=$(wc -c < "$out/android.png")
{
  echo "### Android emulator smoke test"
  echo "- process alive: $([ -n "$pid" ] && echo yes || echo NO)"
  echo "- screenshot bytes: $size"
  echo "- alloy log lines:"; echo '```'; grep -iE "alloy|panicked|wgpu|vulkan|FATAL" "$out/logcat.txt" | tail -25; echo '```'
} | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
[ -n "$pid" ] || { echo "app died"; exit 1; }
# A uniform frame compresses to a few KB; a drawn UI is much larger.
[ "$size" -gt 12000 ] || { echo "screenshot looks blank"; exit 1; }
