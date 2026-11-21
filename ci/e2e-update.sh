#!/usr/bin/env bash
# End-to-end auto-update with real processes, on whatever OS this runs on.
# Builds the demo as 1.0.0 and 2.0.0 (and a tampered 3.0.0), signs releases,
# serves them with a plain HTTP server, runs the 1.0.0 binary's updater, then
# checks the running file really became 2.0.0, a tampered download is refused,
# an unsigned/other-key manifest is refused, and a crash writes a report.
set -euo pipefail
cd "$(dirname "$0")/.."
EXE=""; case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) EXE=".exe";; esac
W=$(mktemp -d); trap 'kill ${PID:-0} 2>/dev/null || true' EXIT
build() { DEMO_VERSION=$1 cargo build -q --release -p alloy-ui-ship --example selfupdate_demo; cp "target/release/examples/selfupdate_demo$EXE" "$2"; }
mkdir -p "$W/srv" "$W/app"
build 2.0.0 "$W/srv/demo-2.0.0$EXE"
build 3.0.0 "$W/srv/demo-3.0.0$EXE"
build 1.0.0 "$W/app/demo$EXE"
APP="$W/app/demo$EXE"
[ "$("$APP" version)" = 1.0.0 ] || { echo "::error::wrong baseline version"; exit 1; }
[ "$("$W/srv/demo-2.0.0$EXE" version)" = 2.0.0 ] || { echo "::error::wrong 2.0.0 build"; exit 1; }

read -r PRIV PUB < <("$APP" keygen)
read -r OTHER_PRIV _ < <("$APP" keygen)
PORT=$((20000 + RANDOM % 20000))
BASE="http://127.0.0.1:$PORT"
(cd "$W/srv" && exec python3 -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1) & PID=$!
for _ in $(seq 50); do curl -fs "$BASE/" >/dev/null && break; sleep 0.2; done

fail() { echo "::error::$*"; exit 1; }
cp "$W/srv/demo-3.0.0$EXE" "$W/orig-3.0.0$EXE"

# 1. Manifest signed with a different key: refused, file untouched.
"$APP" release "$W/srv" 2.0.0 "$BASE" "$W/srv/demo-2.0.0$EXE" "$OTHER_PRIV"
set +e; "$APP" update "$BASE/latest.json" "$PUB"; rc=$?; set -e
[ $rc = 1 ] || fail "wrong-key manifest: expected exit 1, got $rc"
[ "$("$APP" version)" = 1.0.0 ] || fail "wrong-key manifest changed the program"

# 2. Correctly signed manifest whose binary was swapped after signing: refused.
"$APP" release "$W/srv" 3.0.0 "$BASE" "$W/srv/demo-3.0.0$EXE" "$PRIV"
printf 'tampered' >> "$W/srv/demo-3.0.0$EXE"
set +e; "$APP" update "$BASE/latest.json" "$PUB"; rc=$?; set -e
[ $rc = 1 ] || fail "tampered download: expected exit 1, got $rc"
[ "$("$APP" version)" = 1.0.0 ] || fail "tampered download changed the program"

# 3. The real update: 1.0.0 downloads, verifies and replaces itself with 2.0.0.
"$APP" release "$W/srv" 2.0.0 "$BASE" "$W/srv/demo-2.0.0$EXE" "$PRIV"
"$APP" update "$BASE/latest.json" "$PUB"
[ "$("$APP" version)" = 2.0.0 ] || fail "after update the program still reports $("$APP" version)"
echo "OK: $APP is now $("$APP" version)"

# 4. Already current: no downgrade, no change.
set +e; "$APP" update "$BASE/latest.json" "$PUB"; rc=$?; set -e
[ $rc = 3 ] || fail "up-to-date check: expected exit 3, got $rc"
ls "$W/app" | grep -q '\.old$' && echo "note: leftover .old removed on next start" || true

# 5. Crash reporting: a panic leaves a report that the next run can read.
set +e; "$APP" crash "$W/crashes" 2>/dev/null; rc=$?; set -e
[ $rc != 0 ] || fail "crash demo did not crash"
OUT=$("$APP" crashes "$W/crashes")
echo "$OUT"
echo "$OUT" | grep -q '1 report' || fail "no crash report written"
echo "$OUT" | grep -q 'demo crash on purpose' || fail "report lacks the panic message"
{
  echo "### Auto-update and crash report, end to end ($(uname -s))"; echo
  echo "- wrong-key manifest refused, tampered download refused, 1.0.0 -> 2.0.0 replaced in place, no downgrade, crash report written and read back"
} | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
