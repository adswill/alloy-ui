#!/usr/bin/env bash
# Binary size and cold start of one release binary, appended to the job summary.
# usage: footprint.sh <binary> [runs]       Needs a display (xvfb-run on Linux).
set -u
bin=$1; runs=${2:-10}
bytes=$(wc -c < "$bin" | tr -d ' ')
vals=()
for _ in $(seq "$runs"); do
  ALLOY_PERF=1 "$bin" > /tmp/fp.out 2>&1 &
  pid=$!
  for _ in $(seq 100); do grep -q first_paint_ms /tmp/fp.out 2>/dev/null && break; sleep 0.1; done
  v=$(grep -o 'first_paint_ms=[0-9.]*' /tmp/fp.out | cut -d= -f2)
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
  [ -n "$v" ] && vals+=("$v")
done
{
  echo "### $(uname -s) footprint ($(basename "$bin"))"
  echo "- binary: $bytes bytes ($(awk "BEGIN{printf \"%.2f\", $bytes/1000000}") MB, $(awk "BEGIN{printf \"%.2f\", $bytes/1048576}") MiB)"
  if [ ${#vals[@]} -gt 0 ]; then
    sorted=$(printf '%s\n' "${vals[@]}" | sort -n)
    med=$(echo "$sorted" | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}')
    echo "- first paint over ${#vals[@]} runs: median ${med} ms, min $(echo "$sorted" | head -1) ms, max $(echo "$sorted" | tail -1) ms"
  else
    echo "- first paint: no window opened (no usable display or GPU on this runner)"
    tail -5 /tmp/fp.out
  fi
  echo "- phases of the last run:"; echo '```'; grep alloy-perf /tmp/fp.out | head -30; echo '```'
} | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
