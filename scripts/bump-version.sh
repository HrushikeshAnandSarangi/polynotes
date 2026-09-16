#!/usr/bin/env bash
# Bumps the version across every manifest that must agree with a release tag.
# Usage: scripts/bump-version.sh 1.1.0
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>" >&2
  exit 1
fi

VERSION="$1"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

sed -i "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT/core/Cargo.toml"
sed -i "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT/polynotes/src-tauri/Cargo.toml"
sed -i "0,/\"version\": \"[^\"]*\"/s//\"version\": \"$VERSION\"/" "$ROOT/polynotes/src-tauri/tauri.conf.json"
sed -i "0,/\"version\": \"[^\"]*\"/s//\"version\": \"$VERSION\"/" "$ROOT/polynotes/package.json"

if command -v cargo >/dev/null 2>&1; then
  (cd "$ROOT" && cargo update -p core -p polynotes >/dev/null 2>&1) || true
fi

echo "Bumped version to $VERSION in:"
echo "  core/Cargo.toml"
echo "  polynotes/src-tauri/Cargo.toml"
echo "  polynotes/src-tauri/tauri.conf.json"
echo "  polynotes/package.json"
echo
echo "Next steps:"
echo "  git add -A && git commit -m \"chore: bump version to $VERSION\""
echo "  git push origin main"
echo "  git tag v$VERSION && git push origin v$VERSION"
