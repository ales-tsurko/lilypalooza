#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

echo "Building lily-view..."
cargo build

FAKE_BIN_DIR="$(mktemp -d)"
trap 'rm -rf "$FAKE_BIN_DIR"' EXIT

cat > "$FAKE_BIN_DIR/lilypond" <<'EOF'
#!/usr/bin/env bash
echo "GNU LilyPond 2.20.0"
exit 0
EOF
chmod +x "$FAKE_BIN_DIR/lilypond"

echo "Running lily-view with fake old lilypond..."
PATH="$FAKE_BIN_DIR:$PATH" ./target/debug/lily-view
