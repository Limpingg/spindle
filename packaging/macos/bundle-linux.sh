#!/bin/sh
# Bundle Spindle.app from a cross compiled binary, on Linux. Signing is done
# afterwards with rcodesign (apple-codesign). Usage: bundle-linux.sh <binary> <out.app> <version>
set -e
binary="$1"; app="$2"; version="$3"
here="$(cd "$(dirname "$0")" && pwd)"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Library/LaunchAgents"
executable=Spindle
identifier=com.nickaustin.spindle
cp "$binary" "$app/Contents/MacOS/$executable"
chmod 755 "$app/Contents/MacOS/$executable"
build="${version%%-*}"
sed -e "s/__VERSION__/$version/g" -e "s/__BUILD__/$build/g" \
    -e "s/__EXECUTABLE__/$executable/g" -e "s/__IDENTIFIER__/$identifier/g" "$here/Info.plist" \
    > "$app/Contents/Info.plist"
cp "$here/autostart.plist" "$app/Contents/Library/LaunchAgents/$identifier.autostart.plist"
cp "$here/spindle.icns" "$app/Contents/Resources/spindle.icns"
echo "APPL????" > "$app/Contents/PkgInfo"
echo "$app"
