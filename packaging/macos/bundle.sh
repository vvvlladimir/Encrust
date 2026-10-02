#!/usr/bin/env bash
# Builds target/Encrust.app around the release binary. See docs/decisions/0135.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
app="$root/target/Encrust.app"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

command -v rsvg-convert >/dev/null || {
	echo "rsvg-convert is missing: brew install librsvg" >&2
	exit 1
}

cargo build --release --manifest-path "$root/Cargo.toml" -p encrust-app

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

# iconutil wants both the point size and its @2x pixel double, 16 through 512.
iconset="$work/Encrust.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
	rsvg-convert -b none -w "$size" -h "$size" "$root/assets/icon/encrust-macos.svg" \
		-o "$iconset/icon_${size}x${size}.png"
	rsvg-convert -b none -w "$((size * 2))" -h "$((size * 2))" \
		"$root/assets/icon/encrust-macos.svg" \
		-o "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/encrust.icns"

version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -1)"
sed "s/__VERSION__/$version/g" "$root/packaging/macos/Info.plist" \
	>"$app/Contents/Info.plist"
cp "$root/target/release/encrust" "$app/Contents/MacOS/encrust"

# Without this the Finder keeps showing whatever icon it cached for the path.
touch "$app"
echo "built $app"
