#!/bin/bash
set -e

PROJ_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE_DIR="$PROJ_DIR/src-tauri/target/release/bundle"
APP_PATH="$BUNDLE_DIR/macos/embd.app"
DMG_DIR="$BUNDLE_DIR/dmg"
DMG_FINAL="$DMG_DIR/embd.dmg"
DMG_RW="$DMG_DIR/rw_embd.dmg"
VOLNAME="embd"

# Step 1: Build the .app bundle via Tauri
echo "Building app..."
cd "$PROJ_DIR"
npx tauri build

# Step 2: Create DMG with Finder window layout
echo "Creating DMG..."
mkdir -p "$DMG_DIR"
rm -f "$DMG_FINAL" "$DMG_RW"

# Create staging directory
STAGING="$DMG_DIR/staging"
rm -rf "$STAGING"
mkdir -p "$STAGING"
cp -R "$APP_PATH" "$STAGING/"
ln -s /Applications "$STAGING/Applications"

# Create a read-write DMG with explicit size (avoids HFS+/UDRW issue on newer macOS)
hdiutil create -srcfolder "$STAGING" -volname "$VOLNAME" -format UDRO "$DMG_RW"
rm -rf "$STAGING"

# Mount it, customize the Finder window, then unmount
MOUNT_DIR="/Volumes/$VOLNAME"

# Convert to read-write so we can customize
DMG_TEMP="$DMG_DIR/temp_rw.dmg"
hdiutil convert "$DMG_RW" -format UDRW -o "$DMG_TEMP" 2>/dev/null || {
  # If UDRW conversion fails (newer macOS), use the DMG as-is without Finder customization
  echo "Note: Skipping Finder layout customization (unsupported on this macOS version)"
  hdiutil convert "$DMG_RW" -format UDZO -o "$DMG_FINAL"
  rm -f "$DMG_RW"
  echo ""
  echo "Done! DMG created at: $DMG_FINAL"
  open "$DMG_FINAL"
  exit 0
}
rm -f "$DMG_RW"

# Mount the read-write DMG
hdiutil attach "$DMG_TEMP" -mountpoint "$MOUNT_DIR"

# Use AppleScript to set up the Finder window
echo "Configuring Finder window layout..."
osascript <<APPLESCRIPT
tell application "Finder"
  tell disk "$VOLNAME"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set bounds of container window to {100, 100, 640, 440}
    set theViewOptions to icon view options of container window
    set arrangement of theViewOptions to not arranged
    set icon size of theViewOptions to 80
    set position of item "embd.app" of container window to {140, 170}
    set position of item "Applications" of container window to {400, 170}
    close
    open
    update without registering applications
    delay 1
    close
  end tell
end tell
APPLESCRIPT

# Ensure all writes are flushed
sync

# Unmount
hdiutil detach "$MOUNT_DIR" -quiet

# Convert to compressed read-only final DMG
hdiutil convert "$DMG_TEMP" -format UDZO -o "$DMG_FINAL"
rm -f "$DMG_TEMP"

echo ""
echo "Done! DMG created at: $DMG_FINAL"

# Open the DMG so the user can drag to Applications
open "$DMG_FINAL"
