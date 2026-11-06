#!/usr/bin/env bash
# Installs guaitop into ~/.cargo/bin and makes sure that directory is on PATH.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo install --path . --locked --force

CARGO_BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
BIN="$CARGO_BIN/guaitop"
echo "Installed at $BIN"

case ":$PATH:" in
  *":$CARGO_BIN:"*) ;;
  *)
    case "$(basename "${SHELL:-}")" in
      zsh)  RC="$HOME/.zshrc" ;;
      bash) if [ "$(uname -s)" = "Darwin" ]; then RC="$HOME/.bash_profile"; else RC="$HOME/.bashrc"; fi ;;
      *)    RC="$HOME/.profile" ;;
    esac
    LINE="export PATH=\"$CARGO_BIN:\$PATH\""
    if ! grep -qsF "$LINE" "$RC"; then
      printf '\n# Added by guaitop installer\n%s\n' "$LINE" >> "$RC"
      echo "Added $CARGO_BIN to PATH in $RC"
    fi
    echo "Open a new terminal or run: source $RC"
    ;;
esac

if [ "$(uname -s)" = "Linux" ]; then
  echo "Intel GPU: reading its usage requires granting the binary a capability (lost on reinstall):"
  echo "  sudo setcap cap_perfmon=ep $BIN"
fi
