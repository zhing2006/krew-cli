#!/usr/bin/env bash
# Download release binaries from GitHub and place them into npm sub-package directories.
# Usage: ./scripts/prepare-npm.sh <version>
# Example: ./scripts/prepare-npm.sh 0.1.0

set -euo pipefail

VERSION="${1:?Usage: $0 <version>}"
TAG="v${VERSION}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
NPM_DIR="$ROOT_DIR/npm"

echo "Downloading release binaries for $TAG..."

declare -A PLATFORM_MAP=(
  ["krew-win32-x64.exe"]="krew-win32-x64/krew.exe"
  ["krew-linux-x64"]="krew-linux-x64/krew"
  ["krew-linux-arm64"]="krew-linux-arm64/krew"
  ["krew-darwin-x64"]="krew-darwin-x64/krew"
  ["krew-darwin-arm64"]="krew-darwin-arm64/krew"
)

for asset in "${!PLATFORM_MAP[@]}"; do
  dest="${PLATFORM_MAP[$asset]}"
  dest_dir="$NPM_DIR/$(dirname "$dest")"
  dest_file="$NPM_DIR/$dest"

  echo "  Downloading $asset -> $dest"
  gh release download "$TAG" --pattern "$asset" --dir "$dest_dir" --clobber

  # Rename if the asset name differs from the target filename
  downloaded="$dest_dir/$asset"
  if [ "$downloaded" != "$dest_file" ] && [ -f "$downloaded" ]; then
    mv "$downloaded" "$dest_file"
  fi

  # Ensure Unix binaries are executable
  if [[ "$asset" != *.exe ]]; then
    chmod +x "$dest_file"
  fi
done

echo "Done! Binaries placed in npm/ sub-packages."
echo "Run ./scripts/npm-publish.sh to publish."
