#!/usr/bin/env bash
# Windowed benchmark for Linux under Xvfb (needs Xvfb, x11-utils).
# usage: windowed.sh <release-dir> <example>...
# Prints per example: time until an X window exists, idle CPU over 5 s, resident memory.
# Rendering is software (llvmpipe), so this is NOT a GPU number.
set -u
dir=$1; shift
Xvfb :77 -screen 0 1280x800x24 -nolisten tcp >/dev/null 2>&1 &
xp=$!; sleep 1
export DISPLAY=:77
echo "| Example | Window mapped | Idle CPU (5 s) | Resident memory |"
echo "|---|---|---|---|"
for ex in "$@"; do
  before=$(xwininfo -root -children | grep -c '^ *0x')
  t0=$(date +%s%N)
  "$dir/$ex" >/dev/null 2>&1 &
  ap=$!
  for _ in $(seq 1 500); do
    now=$(xwininfo -root -children | grep -c '^ *0x')
    [ "$now" -gt "$before" ] && break
    sleep 0.005
  done
  t1=$(date +%s%N)
  sleep 1.5
  a=$(awk '{print $14+$15}' /proc/$ap/stat); sleep 5; b=$(awk '{print $14+$15}' /proc/$ap/stat)
  hz=$(getconf CLK_TCK)
  cpu=$(awk -v d=$((b-a)) -v hz=$hz 'BEGIN{printf "%.2f", d*100/(hz*5)}')
  rss=$(awk '/VmRSS/{printf "%.0f", $2/1024}' /proc/$ap/status)
  echo "| $ex | $(( (t1-t0)/1000000 )) ms | ${cpu}% | ${rss} MB |"
  kill $ap 2>/dev/null; wait $ap 2>/dev/null
done
kill $xp 2>/dev/null
