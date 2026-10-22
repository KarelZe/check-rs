#!/bin/sh
# Builds target/release/Dame.app: the AppKit front end plus the `dame`
# engine it starts over the Hub protocol. The bundle is signed ad hoc, which
# is enough to run it on this machine; distribution needs a Developer ID.
set -eu

cd "$(dirname "$0")/.."
cargo build --locked --release -p dame-cli -p dame-macos

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
app=target/release/Dame.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/dame-macos target/release/dame "$app/Contents/MacOS/"

# The icon set, built from the 1024 px icon with the tools macOS ships.
iconset=$(mktemp -d)/AppIcon.iconset
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" crates/dame-macos/assets/icon.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" crates/dame-macos/assets/icon.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns -o "$app/Contents/Resources/AppIcon.icns" "$iconset"
rm -rf "$(dirname "$iconset")"

sed "s/@VERSION@/$version/g" crates/dame-macos/Info.plist >"$app/Contents/Info.plist"
plutil -lint -s "$app/Contents/Info.plist"
codesign --force --sign - "$app/Contents/MacOS/dame"
codesign --force --sign - "$app"
echo "$app"
