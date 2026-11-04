#!/usr/bin/env bash
# Runs observability in development mode.
set -euo pipefail
cd "$(dirname "$0")/../.."
exec cargo run -- "$@"
