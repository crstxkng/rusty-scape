#!/usr/bin/env bash
# Builds a universal (Intel + Apple Silicon) "Rusty Scape.app" into dist/mac.
# Must run on macOS.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
export MACOSX_DEPLOYMENT_TARGET=11.0

rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null 2>&1 || true
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

APP="dist/mac/Rusty Scape.app"
rm -rf dist/mac
mkdir -p "$APP/Contents/MacOS"
lipo -create \
    target/aarch64-apple-darwin/release/rusty-scape \
    target/x86_64-apple-darwin/release/rusty-scape \
    -output "$APP/Contents/MacOS/rusty-scape"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Rusty Scape</string>
    <key>CFBundleDisplayName</key><string>Rusty Scape</string>
    <key>CFBundleIdentifier</key><string>io.itch.rusty-scape</string>
    <key>CFBundleVersion</key><string>${VERSION}</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundleExecutable</key><string>rusty-scape</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>LSMinimumSystemVersion</key><string>${MACOSX_DEPLOYMENT_TARGET}</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Ad-hoc signature so Apple Silicon will launch it (players still need right-click > Open once).
codesign --force --deep --sign - "$APP" || true
echo "macOS build ready in dist/mac"
