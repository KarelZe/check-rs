#!/bin/sh
# Builds target/release/Dame-<version>.dmg: a compressed disk image holding
# Dame.app and a link to /Applications to drag it onto. The app supports
# Apple silicon only, so the image is built on an Apple silicon Mac.
set -eu

if [ "$(uname -m)" != arm64 ]; then
  echo "error: build the disk image on an Apple silicon Mac" >&2
  exit 1
fi
cd "$(dirname "$0")/.."
scripts/bundle-macos.sh >/dev/null

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
dmg=target/release/Dame-$version.dmg
staging=$(mktemp -d)
cp -R target/release/Dame.app "$staging/"
ln -s /Applications "$staging/Applications"

# hdiutil sometimes fails with "Resource busy" on CI runners; a retry helps.
attempt=1
until hdiutil create -quiet -ov -volname Dame -srcfolder "$staging" -fs HFS+ -format UDZO "$dmg"; do
  if [ "$attempt" -ge 3 ]; then
    exit 1
  fi
  attempt=$((attempt + 1))
  sleep 5
done
rm -rf "$staging"
echo "$dmg"
