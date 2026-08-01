#!/usr/bin/env bash
set -euo pipefail

echo "=== Format (cargo fmt) ==="
cargo fmt --all
cargo fmt --all -- --check

echo ""
echo "=== Build (warnings as errors) ==="
RUSTFLAGS="-D warnings" cargo build --verbose

echo ""
echo "=== Clippy (warnings as errors) ==="
cargo clippy --all-targets -- -D warnings

echo ""
echo "=== Tests ==="
cargo test --verbose

echo ""
echo "=== All CI checks passed ==="