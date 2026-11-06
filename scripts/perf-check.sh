#!/usr/bin/env bash
# Rough measurement of the dashboard cost (Linux): average CPU and peak RSS over
# 20 s in a 130x40 pseudo-terminal. Budget: < 2 % CPU, < 50 MiB and < 15 context
# switches per second across all threads. The program only wakes up when there is a sample
# or a keypress (~1/s per thread); the rest are kernel waits while reading slow sensors (EC,
# NVMe SMART) every 5 s, which use no CPU. A 100 ms poll would add 10 more.
# The TUI runs inside a tmux session (a reliable background pty: it does not get EOF
# on stdin as it would with `script`). With `exec`, the pane PID is the binary's.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ "$(uname -s)" != "Linux" ]; then
  echo "Linux only (reads /proc). On macOS, measure it with Activity Monitor." >&2
  exit 2
fi
if ! command -v tmux >/dev/null 2>&1; then
  echo "tmux is required to run the TUI in a pseudo-terminal; install it and try again" >&2
  exit 2
fi

SESSION="guaitop-perf-$$"
PID=""
cleanup() {
  [ -n "$PID" ] && kill "$PID" 2>/dev/null || true
  tmux kill-session -t "$SESSION" 2>/dev/null || true
}
trap cleanup EXIT

cargo build --release -q
BIN="${CARGO_TARGET_DIR:-$PWD/target}/release/guaitop"
case "$BIN" in /*) ;; *) BIN="$PWD/$BIN" ;; esac
if [ ! -x "$BIN" ]; then
  echo "binary $BIN not found" >&2
  exit 1
fi

tmux new-session -d -s "$SESSION" -x 130 -y 40 "exec '$BIN'"
sleep 3

PID="$(tmux list-panes -t "$SESSION" -F '#{pane_pid}' 2>/dev/null | head -n1 || true)"
if [ -z "$PID" ] || [ ! -r "/proc/$PID/stat" ]; then
  PID=""
  echo "guaitop process not found (did it exit before measuring?)" >&2
  exit 1
fi
if [ "$(readlink -f "/proc/$PID/exe")" != "$(readlink -f "$BIN")" ]; then
  echo "PID $PID is not the guaitop binary (it is $(readlink -f "/proc/$PID/exe"))" >&2
  exit 1
fi

HZ="$(getconf CLK_TCK)"
T0="$(awk '{print $14+$15}' "/proc/$PID/stat")"
switches() { awk '/ctxt_switches/{s+=$2} END{print s}' /proc/"$PID"/task/*/status; }
W0="$(switches)"
sleep 20
if [ ! -r "/proc/$PID/stat" ]; then
  PID=""
  echo "the guaitop process exited during the measurement" >&2
  exit 1
fi
T1="$(awk '{print $14+$15}' "/proc/$PID/stat")"
W1="$(switches)"
WAKE="$(awk -v a="$W0" -v b="$W1" 'BEGIN{printf "%.1f", (b-a)/20}')"
RSS="$(awk '/VmHWM/{printf "%.1f", $2/1024}' "/proc/$PID/status")"
CPU="$(awk -v a="$T0" -v b="$T1" -v hz="$HZ" 'BEGIN{printf "%.2f", (b-a)/hz/20*100}')"

echo "CPU: ${CPU}%  RSS: ${RSS} MiB  despertades: ${WAKE}/s"
if awk -v c="$CPU" -v r="$RSS" -v w="$WAKE" 'BEGIN{exit !(c < 2.0 && r < 50.0 && w < 15.0)}'; then
  echo "OK (< 2% CPU, < 50 MiB, < 15 despertades/s)"
else
  echo "OVER BUDGET"
  exit 1
fi
