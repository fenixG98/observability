#!/usr/bin/env bash
# Rough measurement of the dashboard cost (Linux): average CPU and peak RSS over
# 20 s in a 130x40 pseudo-terminal. Budget: < 2 % CPU and < 50 MiB.
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

SESSION="observability-perf-$$"
PID=""
cleanup() {
  [ -n "$PID" ] && kill "$PID" 2>/dev/null || true
  tmux kill-session -t "$SESSION" 2>/dev/null || true
}
trap cleanup EXIT

cargo build --release -q
BIN="${CARGO_TARGET_DIR:-$PWD/target}/release/observability"
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
  echo "observability process not found (did it exit before measuring?)" >&2
  exit 1
fi
if [ "$(readlink -f "/proc/$PID/exe")" != "$(readlink -f "$BIN")" ]; then
  echo "PID $PID is not the observability binary (it is $(readlink -f "/proc/$PID/exe"))" >&2
  exit 1
fi

HZ="$(getconf CLK_TCK)"
T0="$(awk '{print $14+$15}' "/proc/$PID/stat")"
sleep 20
if [ ! -r "/proc/$PID/stat" ]; then
  PID=""
  echo "the observability process exited during the measurement" >&2
  exit 1
fi
T1="$(awk '{print $14+$15}' "/proc/$PID/stat")"
RSS="$(awk '/VmHWM/{printf "%.1f", $2/1024}' "/proc/$PID/status")"
CPU="$(awk -v a="$T0" -v b="$T1" -v hz="$HZ" 'BEGIN{printf "%.2f", (b-a)/hz/20*100}')"

echo "CPU: ${CPU}%  RSS: ${RSS} MiB"
if awk -v c="$CPU" -v r="$RSS" 'BEGIN{exit !(c < 2.0 && r < 50.0)}'; then
  echo "OK (< 2% CPU, < 50 MiB)"
else
  echo "OVER BUDGET"
  exit 1
fi
