#!/usr/bin/env bash
# Installs guaitop into ~/.cargo/bin.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo install --path . --locked --force
BIN="$(command -v guaitop || echo "$HOME/.cargo/bin/guaitop")"
echo "Installed at $BIN"
if [ "$(uname -s)" = "Linux" ]; then
  echo "Intel GPU: reading its usage requires granting the binary a capability (lost on reinstall):"
  echo "  sudo setcap cap_perfmon=ep $BIN"
fi
