#!/bin/sh
# Fetch the four Craft apps (and craft-i18n / craft-fonts, and draw.io's web app) at the commits in upstream.lock.
set -e
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root/upstream"
grep -v '^#' "$root/upstream.lock" | while read -r name url rev mode; do
  [ -z "$name" ] && continue
  dir="$root/upstream/$name"
  if [ "$mode" = "shallow" ]; then
    # Large repositories (draw.io): just the tree at the pinned commit.
    [ -d "$dir/.git" ] || { git init --quiet "$dir" && git -C "$dir" remote add origin "$url"; }
    if [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null)" != "$rev" ]; then
      git -C "$dir" fetch --quiet --depth 1 origin "$rev" && git -C "$dir" checkout --quiet FETCH_HEAD
    fi
    echo "$name at $(git -C "$dir" rev-parse --short HEAD)"
    continue
  fi
  if [ ! -d "$dir/.git" ]; then git clone --quiet "$url" "$dir"; else git -C "$dir" fetch --quiet origin; fi
  if [ "$1" = "--latest" ]; then git -C "$dir" checkout --quiet origin/HEAD; else git -C "$dir" checkout --quiet "$rev"; fi
  echo "$name at $(git -C "$dir" rev-parse --short HEAD)"
  if [ -f "$root/patches/$name.patch" ]; then
    git -C "$dir" checkout --quiet -- . && git -C "$dir" apply --whitespace=nowarn "$root/patches/$name.patch"
    echo "  applied Themisio's patch to $name"
  fi
done
