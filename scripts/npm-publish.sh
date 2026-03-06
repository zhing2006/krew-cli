#!/usr/bin/env bash
# Publish all npm packages in the correct order (sub-packages first, then main).
# Usage: ./scripts/npm-publish.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
NPM_DIR="$(cd "$SCRIPT_DIR/../npm" && pwd)"

SUB_PACKAGES=(
  "krew-win32-x64"
  "krew-linux-x64"
  "krew-linux-arm64"
  "krew-darwin-x64"
  "krew-darwin-arm64"
)

echo "Publishing sub-packages..."
for pkg in "${SUB_PACKAGES[@]}"; do
  echo "  Publishing @zhing2006/$pkg"
  (cd "$NPM_DIR/$pkg" && npm publish --access public)
done

echo "Publishing main package @zhing2006/krew..."
(cd "$NPM_DIR/krew" && npm publish --access public)

echo "All packages published successfully!"
