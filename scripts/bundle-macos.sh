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
mkdir -p "$app/Contents/MacOS"
cp target/release/dame-macos target/release/dame "$app/Contents/MacOS/"
sed "s/@VERSION@/$version/g" crates/dame-macos/Info.plist >"$app/Contents/Info.plist"
plutil -lint -s "$app/Contents/Info.plist"
codesign --force --sign - "$app/Contents/MacOS/dame"
codesign --force --sign - "$app"
echo "$app"
