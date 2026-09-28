#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --quiet
exec ./target/debug/reru "$@"
