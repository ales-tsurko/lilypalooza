#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

echo "Building Lilypalooza..."
cargo build

EMPTY_PATH_DIR="$(mktemp -d)"
trap 'rm -rf "$EMPTY_PATH_DIR"' EXIT

echo "Running Lilypalooza with no lilypond in PATH..."
PATH="$EMPTY_PATH_DIR" ./target/debug/lilypalooza
