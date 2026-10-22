#!/bin/sh
# Renders crates/dame-macos/assets/icon.svg into icon.png (1024 px), from which
# scripts/bundle-macos.sh builds the app icon. Run it after editing the SVG;
# it needs ImageMagick (`magick`). Quick Look renders the SVG with WebKit,
# and ImageMagick cuts the rounded tile out of its opaque background.
set -eu

cd "$(dirname "$0")/.."
assets=crates/dame-macos/assets
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

qlmanage -t -s 1024 -o "$work" "$assets/icon.svg" >/dev/null
magick "$work/icon.svg.png" \
  \( -size 1024x1024 xc:black -fill white -draw "roundrectangle 100,100 923,923 185,185" \) \
  -alpha off -compose CopyOpacity -composite \
  -strip -depth 8 -define png:compression-level=9 "$assets/icon.png"
