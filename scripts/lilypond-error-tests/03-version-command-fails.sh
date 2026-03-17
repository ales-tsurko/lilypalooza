#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

echo "Building lily-view..."
cargo build

FAKE_BIN_DIR="$(mktemp -d)"
trap 'rm -rf "$FAKE_BIN_DIR"' EXIT

cat > "$FAKE_BIN_DIR/lilypond" <<'EOF'
#!/bin/sh
echo "broken lilypond install" >&2
exit 1
EOF
chmod +x "$FAKE_BIN_DIR/lilypond"

echo "Running lily-view with failing lilypond --version..."
PATH="$FAKE_BIN_DIR:$PATH" command -v lilypond
PATH="$FAKE_BIN_DIR:$PATH" lilypond --version || true
PATH="$FAKE_BIN_DIR:$PATH" ./target/debug/lily-view
