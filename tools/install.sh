#!/bin/bash
# Build rustpaint and install it for this user, into ~/.cargo/bin. Rerun to upgrade.
# Works from any folder: tools/install.sh, ../tools/install.sh, and so on.
# Needs Rust (https://rustup.rs).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
command -v cargo >/dev/null || { echo "cargo not found; install Rust from https://rustup.rs first" >&2; exit 1; }
cargo install --locked --path "$ROOT/crates/app"
