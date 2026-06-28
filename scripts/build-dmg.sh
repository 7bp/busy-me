#!/bin/bash
# Build a proper macOS .app bundle + DMG for Busy Me.
# Usage: ./scripts/build-dmg.sh [arch]
#   arch defaults to the host architecture, or specify "arm64" / "x64"

set -euo pipefail

ARCH="${1:-}"
APP_NAME="Busy Me"
DMG_NAME="Busy Me"
BINARY="busy-me"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# ── 1. Determine target ──────────────────────
if [ -z "$ARCH" ]; then
    ARCH_NAME="$(uname -m)"
    case "$ARCH_NAME" in
        arm64) RUST_TARGET="aarch64-apple-darwin" ;;
        x86_64) RUST_TARGET="x86_64-apple-darwin" ;;
        *) echo "Unknown arch: $ARCH_NAME"; exit 1 ;;
    esac
else
    case "$ARCH" in
        arm64|aarch64) RUST_TARGET="aarch64-apple-darwin" ;;
        x64|x86_64)    RUST_TARGET="x86_64-apple-darwin" ;;
        *) echo "Usage: $0 [arm64|x64]"; exit 1 ;;
    esac
fi

echo "→ Building for $RUST_TARGET"

# ── 2. Build ──────────────────────────────────
cargo build --release --target "$RUST_TARGET"

# ── 3. Create .app bundle ─────────────────────
BUILD_DIR="target/$RUST_TARGET/release"
BUNDLE_DIR="$BUILD_DIR/$APP_NAME.app"
MACOS_DIR="$BUNDLE_DIR/Contents/MacOS"
RESOURCES_DIR="$BUNDLE_DIR/Contents/Resources"

rm -rf "$BUNDLE_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$BUILD_DIR/$BINARY" "$MACOS_DIR/$BINARY"

# Info.plist
cp macos/Info.plist "$BUNDLE_DIR/Contents/Info.plist"

# App icon
if [ -f Resources/icon.icns ]; then
    cp Resources/icon.icns "$RESOURCES_DIR/icon.icns"
fi

# ── 4. Code-sign (ad-hoc for local use) ───────
echo "→ Signing with ad-hoc identity..."
codesign --force --deep --sign - "$BUNDLE_DIR" 2>/dev/null || true

echo "→ Bundle created: $BUNDLE_DIR"

# ── 5. Create DMG ─────────────────────────────
DMG_PATH="$BUILD_DIR/$DMG_NAME.dmg"
rm -f "$DMG_PATH"

# Use a temporary directory for DMG staging
STAGING_DIR="/tmp/busy-me-dmg"
rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR"

# Copy app into staging with a symlink to /Applications
cp -R "$BUNDLE_DIR" "$STAGING_DIR/"
ln -s /Applications "$STAGING_DIR/Applications"

echo "→ Creating DMG..."
hdiutil create -volname "$DMG_NAME" \
    -srcfolder "$STAGING_DIR" \
    -ov -format UDZO \
    "$DMG_PATH" 2>/dev/null

rm -rf "$STAGING_DIR"

echo "→ DMG created: $DMG_PATH"
echo "Done."
