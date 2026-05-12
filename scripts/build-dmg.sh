#!/bin/bash
set -e

PROJ_DIR="$(cd "$(dirname "$0")/.." && pwd)"
APP_PATH="$PROJ_DIR/src-tauri/target/release/bundle/macos/leo.app"

# Step 1: Clean previous build cache to ensure fresh build
echo "Cleaning previous build..."
cd "$PROJ_DIR/src-tauri"
cargo clean --release 2>/dev/null || true

# Step 2: Kill running app and clear macOS WebView cache
echo "Clearing app cache..."
killall leo 2>/dev/null || true
rm -rf ~/Library/WebKit/com.leo.ide ~/Library/Caches/com.leo.ide

# Step 3: Build the .app bundle via Tauri
echo "Building app..."
cd "$PROJ_DIR"
npx tauri build

# Step 4: Install to /Applications
echo "Installing to /Applications..."
rm -rf /Applications/leo.app
cp -R "$APP_PATH" /Applications/
rm -rf "$PROJ_DIR/src-tauri/target/release/bundle/macos"

echo ""
echo "Done! leo has been installed to /Applications."
echo "Open it from Spotlight or your Applications folder."
