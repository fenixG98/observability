#!/usr/bin/env bash
# Usage: bin/test.sh [--back|--front|--e2e]   (no argument = everything)
set -euo pipefail
cd "$(dirname "$0")/.."
mode="${1:-all}"
case "$mode" in
  all|--back)
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    ;;
  --front)
    echo "There is no web frontend: the terminal UI is tested with --back (TestBackend snapshots)."
    ;;
  --e2e)
    cargo build --release
    ./target/release/guaitop --probe
    ;;
  *)
    echo "Usage: bin/test.sh [--back|--front|--e2e]" >&2
    exit 2
    ;;
esac
