#!/usr/bin/env bash
# Runs obstui built in release mode.
set -euo pipefail
cd "$(dirname "$0")/../.."
exec cargo run --release -- "$@"
