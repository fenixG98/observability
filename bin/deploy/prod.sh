#!/usr/bin/env bash
# Installs observability into ~/.cargo/bin.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo install --path . --locked --force
BIN="$(command -v observability || echo "$HOME/.cargo/bin/observability")"
echo "Installed at $BIN"
if [ "$(uname -s)" = "Linux" ]; then
  echo "Intel GPU: reading its usage requires granting the binary a capability (lost on reinstall):"
  echo "  sudo setcap cap_perfmon=ep $BIN"
fi
