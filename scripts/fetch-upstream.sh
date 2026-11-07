#!/bin/sh
# Fetch the four Craft apps (and craft-i18n / craft-fonts) at the commits in upstream.lock.
set -e
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root/upstream"
grep -v '^#' "$root/upstream.lock" | while read -r name url rev; do
  [ -z "$name" ] && continue
  dir="$root/upstream/$name"
  if [ ! -d "$dir/.git" ]; then git clone --quiet "$url" "$dir"; else git -C "$dir" fetch --quiet origin; fi
  if [ "$1" = "--latest" ]; then git -C "$dir" checkout --quiet origin/HEAD; else git -C "$dir" checkout --quiet "$rev"; fi
  echo "$name at $(git -C "$dir" rev-parse --short HEAD)"
  if [ -f "$root/patches/$name.patch" ]; then
    git -C "$dir" checkout --quiet -- . && git -C "$dir" apply --whitespace=nowarn "$root/patches/$name.patch"
    echo "  applied Themisio's patch to $name"
  fi
done
