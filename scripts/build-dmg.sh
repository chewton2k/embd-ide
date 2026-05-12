#!/usr/bin/env bash
#
# leo — macOS production build & install script.
#
# Invoked by `npm run tauri:build`. Builds `src-tauri` as a macOS .app bundle,
# verifies the result, and installs it to /Applications. Also scrubs the
# legacy `embd` project's on-disk artifacts so users upgrading from that name
# get a clean install.
#
# Usage:
#   scripts/build-dmg.sh [flags]
#
# Flags:
#   --no-install        Build but do not copy into /Applications.
#   --no-clean          Skip the cargo clean + cache/legacy cleanup steps.
#   --universal         Build a universal (arm64 + x86_64) binary.
#   --dmg               Also package a .dmg alongside the .app (requires
#                       `create-dmg`; install with `brew install create-dmg`).
#   --verbose           Enable extra logging (`set -x`).
#   -h | --help         Show this help.
#
# Environment overrides:
#   NEW_PRODUCT         Override the product name (default: leo).
#   NEW_IDENTIFIER      Override the bundle id (default: com.leo.ide).
#   SKIP_INSTALL=1      Same as --no-install.
#   SKIP_CLEAN=1        Same as --no-clean.
#   NO_COLOR=1          Disable colored output.
#
# Signing & notarization are handled by Tauri — see tauri.conf.json and the
# APPLE_* environment variables documented at https://tauri.app.

# ── Strict mode ────────────────────────────────────────────────────

set -Eeuo pipefail

SCRIPT_NAME="$(basename "$0")"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJ_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Capture the wall-clock start for a final timing line.
BUILD_START_EPOCH="$(date +%s)"

# ── Color / logging ────────────────────────────────────────────────

if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
  C_RESET=$'\033[0m'
  C_BOLD=$'\033[1m'
  C_DIM=$'\033[2m'
  C_RED=$'\033[31m'
  C_GREEN=$'\033[32m'
  C_YELLOW=$'\033[33m'
  C_BLUE=$'\033[34m'
else
  C_RESET='' C_BOLD='' C_DIM='' C_RED='' C_GREEN='' C_YELLOW='' C_BLUE=''
fi

log_step()  { printf '%s▸%s %s\n' "$C_BLUE" "$C_RESET" "$1"; }
log_info()  { printf '  %s%s%s\n'   "$C_DIM"  "$1"       "$C_RESET"; }
log_ok()    { printf '%s✓%s %s\n'   "$C_GREEN" "$C_RESET" "$1"; }
log_warn()  { printf '%s!%s %s\n'   "$C_YELLOW" "$C_RESET" "$1" >&2; }
log_error() { printf '%s✗%s %s\n'   "$C_RED"   "$C_RESET" "$1" >&2; }

# Error trap — prints the failing command + line so CI logs are readable.
on_err() {
  local exit_code=$?
  local line=$1
  local cmd=$2
  log_error "Failed at line ${line}: ${cmd}"
  log_error "Exit code: ${exit_code}"
  exit "$exit_code"
}
trap 'on_err "$LINENO" "$BASH_COMMAND"' ERR

# Interrupt trap — clean exit on Ctrl-C.
trap 'log_warn "Aborted"; exit 130' INT TERM

# ── Flags ──────────────────────────────────────────────────────────

SKIP_INSTALL="${SKIP_INSTALL:-0}"
SKIP_CLEAN="${SKIP_CLEAN:-0}"
BUILD_UNIVERSAL=0
BUILD_DMG=0
VERBOSE=0

usage() {
  sed -n '2,/^# ── Strict mode/p' "$0" | sed 's/^# \{0,1\}//' | sed '$d'
  exit "$1"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-install) SKIP_INSTALL=1 ;;
    --no-clean)   SKIP_CLEAN=1 ;;
    --universal)  BUILD_UNIVERSAL=1 ;;
    --dmg)        BUILD_DMG=1 ;;
    --verbose)    VERBOSE=1 ;;
    -h|--help)    usage 0 ;;
    *)            log_error "Unknown argument: $1"; usage 2 ;;
  esac
  shift
done

[[ "$VERBOSE" == 1 ]] && set -x

# ── Identity ───────────────────────────────────────────────────────

NEW_PRODUCT="${NEW_PRODUCT:-leo}"
NEW_IDENTIFIER="${NEW_IDENTIFIER:-com.leo.ide}"

# Legacy identities are fully scrubbed on install. Add any future rename here
# (both arrays must grow together).
LEGACY_PRODUCTS=("embd")
LEGACY_IDENTIFIERS=("com.embd.ide")

if [[ "$BUILD_UNIVERSAL" == 1 ]]; then
  BUNDLE_SUBDIR="universal-apple-darwin/release/bundle/macos"
  TAURI_TARGET_ARG=(--target universal-apple-darwin)
else
  BUNDLE_SUBDIR="release/bundle/macos"
  TAURI_TARGET_ARG=()
fi
APP_PATH="$PROJ_DIR/src-tauri/target/$BUNDLE_SUBDIR/${NEW_PRODUCT}.app"
DMG_OUT_DIR="$PROJ_DIR/dist-dmg"

# ── Preflight ──────────────────────────────────────────────────────

log_step "Preflight"

if [[ "$(uname -s)" != "Darwin" ]]; then
  log_error "This script targets macOS. Detected: $(uname -s)"
  exit 1
fi

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log_error "Missing required tool: $1"
    [[ -n "${2:-}" ]] && log_info "Install via: $2"
    exit 1
  fi
}

need node    "https://nodejs.org or brew install node"
need npm     "ships with node"
need cargo   "https://rustup.rs"
need rustc   "https://rustup.rs"
need npx     "ships with node"

if ! xcode-select -p >/dev/null 2>&1; then
  log_error "Xcode Command Line Tools not installed"
  log_info "Install via: xcode-select --install"
  exit 1
fi

# Node >= 18 required by Vite 7 / Tauri 2 tooling.
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
if (( NODE_MAJOR < 18 )); then
  log_error "Node $NODE_MAJOR.x detected; need >= 18"
  exit 1
fi

# Ensure deps are installed (but don't force reinstall).
if [[ ! -d "$PROJ_DIR/node_modules" ]]; then
  log_info "node_modules missing — running npm install"
  ( cd "$PROJ_DIR" && npm install --no-audit --no-fund )
fi

if [[ "$BUILD_DMG" == 1 ]] && ! command -v create-dmg >/dev/null 2>&1; then
  log_error "--dmg requested but 'create-dmg' is not installed"
  log_info "Install with: brew install create-dmg"
  exit 1
fi

# Warn if no signing identity is configured. Builds still produce a .app, but
# users on other Macs will hit Gatekeeper / notarization warnings.
if [[ -z "${APPLE_SIGNING_IDENTITY:-}${APPLE_CERTIFICATE:-}" ]]; then
  log_warn "No APPLE_SIGNING_IDENTITY / APPLE_CERTIFICATE set — the resulting"
  log_warn "bundle will be unsigned and won't pass Gatekeeper on other Macs."
fi

log_ok "Environment OK (macOS $(sw_vers -productVersion), Node $(node -v), $(rustc --version | awk '{print $1, $2}'))"

# ── Helpers ────────────────────────────────────────────────────────

# Cache-only cleanup for the CURRENT bundle id. Preserves persistent state
# (recent projects, AI conversations, settings) stored in Application Support
# and Preferences.
clean_identifier_caches() {
  local id="$1"
  local targets=(
    "$HOME/Library/WebKit/$id"
    "$HOME/Library/Caches/$id"
    "$HOME/Library/HTTPStorages/$id"
    "$HOME/Library/HTTPStorages/${id}.binarycookies"
    "$HOME/Library/Logs/$id"
    "$HOME/Library/Saved Application State/${id}.savedState"
  )
  for t in "${targets[@]}"; do
    if [[ -e "$t" ]]; then
      rm -rf -- "$t"
    fi
  done
}

# Full scrub for LEGACY ids — remove every trace, including Application Support
# and Preferences. Never run on the current id.
clean_identifier_all() {
  local id="$1"
  clean_identifier_caches "$id"
  local targets=(
    "$HOME/Library/Application Support/$id"
    "$HOME/Library/Preferences/${id}.plist"
    "$HOME/Library/Cookies/${id}.binarycookies"
  )
  for t in "${targets[@]}"; do
    if [[ -e "$t" ]]; then
      rm -rf -- "$t"
    fi
  done
}

# Remove an installed .app from /Applications, using sudo only if needed.
remove_installed_app() {
  local name="$1"
  local path="/Applications/${name}.app"
  [[ -e "$path" ]] || return 0
  if [[ -w "/Applications" ]] && [[ -w "$path" || -O "$path" ]]; then
    rm -rf -- "$path"
  else
    log_info "Using sudo to remove $path"
    sudo rm -rf -- "$path"
  fi
}

# Graceful process kill: SIGTERM, wait, then SIGKILL if still alive.
kill_process() {
  local name="$1"
  if pgrep -x "$name" >/dev/null 2>&1; then
    log_info "Stopping running $name"
    killall "$name" 2>/dev/null || true
    # Give it up to 3 seconds to exit cleanly before escalating.
    local i=0
    while (( i < 30 )) && pgrep -x "$name" >/dev/null 2>&1; do
      sleep 0.1; ((i++))
    done
    if pgrep -x "$name" >/dev/null 2>&1; then
      killall -KILL "$name" 2>/dev/null || true
    fi
  fi
}

# Format a byte count as a human-readable size (uses BSD `du -h`).
human_size() {
  local path="$1"
  [[ -e "$path" ]] && du -sh "$path" 2>/dev/null | awk '{print $1}' || echo "?"
}

# ── Step 1: Cleanup ────────────────────────────────────────────────

if [[ "$SKIP_CLEAN" == 1 ]]; then
  log_step "Cleanup (skipped)"
else
  log_step "Cleanup"

  log_info "cargo clean --release"
  ( cd "$PROJ_DIR/src-tauri" && cargo clean --release >/dev/null 2>&1 || true )

  kill_process "$NEW_PRODUCT"
  for legacy in "${LEGACY_PRODUCTS[@]}"; do
    kill_process "$legacy"
  done

  log_info "Clearing ephemeral caches for $NEW_IDENTIFIER"
  clean_identifier_caches "$NEW_IDENTIFIER"

  for legacy_id in "${LEGACY_IDENTIFIERS[@]}"; do
    log_info "Scrubbing legacy data for $legacy_id"
    clean_identifier_all "$legacy_id"
  done

  for legacy in "${LEGACY_PRODUCTS[@]}"; do
    if [[ -e "/Applications/${legacy}.app" ]]; then
      log_info "Removing /Applications/${legacy}.app"
      remove_installed_app "$legacy"
    fi
  done
fi

# ── Step 2: Build ──────────────────────────────────────────────────

log_step "Build"
cd "$PROJ_DIR"

# Assemble extra CLI args for `tauri build`. Using the `${arr[@]+...}` idiom
# so expansion is safe under `set -u` on macOS's stock bash 3.2 when the array
# is empty.
BUILD_ARGS=()
if (( ${#TAURI_TARGET_ARG[@]} > 0 )); then
  BUILD_ARGS+=("${TAURI_TARGET_ARG[@]}")
fi

log_info "npx tauri build ${BUILD_ARGS[*]+${BUILD_ARGS[*]}}"
npx tauri build ${BUILD_ARGS[@]+"${BUILD_ARGS[@]}"}

if [[ ! -d "$APP_PATH" ]]; then
  log_error "Build finished but $APP_PATH was not produced"
  log_info "Expected bundle subdir: target/$BUNDLE_SUBDIR"
  exit 1
fi

log_ok "Built $APP_PATH ($(human_size "$APP_PATH"))"

# ── Step 3: Verify bundle ──────────────────────────────────────────

log_step "Verify"

# Confirm the Info.plist identifier matches what we expect. Protects against
# a stale tauri.conf.json / cargo cache producing the wrong bundle id.
PLIST="$APP_PATH/Contents/Info.plist"
if [[ -f "$PLIST" ]]; then
  BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$PLIST" 2>/dev/null || true)"
  BUNDLE_VER="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$PLIST" 2>/dev/null || true)"
  if [[ -n "$BUNDLE_ID" && "$BUNDLE_ID" != "$NEW_IDENTIFIER" ]]; then
    log_error "Bundle identifier mismatch: got '$BUNDLE_ID', expected '$NEW_IDENTIFIER'"
    exit 1
  fi
  log_info "Identifier:  $BUNDLE_ID"
  log_info "Version:     ${BUNDLE_VER:-unknown}"
fi

# Code-signing status (informational). If the signature is missing or ad-hoc,
# `codesign --verify --deep --strict` will exit non-zero for an unsigned or
# corrupt bundle — we catch that but don't fail the overall build, since a
# local dev build without an Apple Developer certificate is a valid case.
CODESIGN_STATUS="unsigned"
if command -v codesign >/dev/null 2>&1; then
  if codesign --verify --deep --strict "$APP_PATH" 2>/dev/null; then
    SIGN_INFO="$(codesign -dvv "$APP_PATH" 2>&1 || true)"
    if grep -q 'Authority=Developer ID Application' <<<"$SIGN_INFO"; then
      CODESIGN_STATUS="Developer ID signed"
    elif grep -q 'Signature=adhoc' <<<"$SIGN_INFO"; then
      CODESIGN_STATUS="ad-hoc signed"
    else
      CODESIGN_STATUS="signed (unknown authority)"
    fi
  fi
fi
log_info "Signing:     $CODESIGN_STATUS"

# Gatekeeper assessment — will say "rejected" for unsigned/unnotarized apps.
if command -v spctl >/dev/null 2>&1; then
  if spctl --assess --type execute "$APP_PATH" 2>/dev/null; then
    log_info "Gatekeeper:  accepted"
  else
    log_info "Gatekeeper:  rejected (expected for un-notarized local builds)"
  fi
fi

# ── Step 4: Optional DMG ───────────────────────────────────────────

if [[ "$BUILD_DMG" == 1 ]]; then
  log_step "Packaging .dmg"
  mkdir -p "$DMG_OUT_DIR"
  rm -f "$DMG_OUT_DIR/${NEW_PRODUCT}"*.dmg
  DMG_NAME="${NEW_PRODUCT}-${BUNDLE_VER:-dev}.dmg"
  ( cd "$DMG_OUT_DIR" \
    && create-dmg \
        --volname "$NEW_PRODUCT" \
        --window-size 540 380 \
        --icon-size 96 \
        --icon "${NEW_PRODUCT}.app" 140 190 \
        --app-drop-link 400 190 \
        --hide-extension "${NEW_PRODUCT}.app" \
        --no-internet-enable \
        "$DMG_NAME" \
        "$APP_PATH" >/dev/null )
  log_ok "Wrote $DMG_OUT_DIR/$DMG_NAME ($(human_size "$DMG_OUT_DIR/$DMG_NAME"))"
fi

# ── Step 5: Install ────────────────────────────────────────────────

if [[ "$SKIP_INSTALL" == 1 ]]; then
  log_step "Install (skipped)"
else
  log_step "Install to /Applications"

  DEST="/Applications/${NEW_PRODUCT}.app"
  remove_installed_app "$NEW_PRODUCT"

  if [[ -w "/Applications" ]]; then
    cp -R "$APP_PATH" "/Applications/"
  else
    log_info "Using sudo to copy to /Applications"
    sudo cp -R "$APP_PATH" "/Applications/"
  fi

  # Drop stale LaunchServices cache so Finder forgets the old bundle and
  # shows the new icon/name correctly. Non-fatal if unavailable.
  LSREGISTER=/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister
  if [[ -x "$LSREGISTER" ]]; then
    log_info "Refreshing LaunchServices"
    "$LSREGISTER" -kill -r -domain local -domain system -domain user \
        >/dev/null 2>&1 || true
    "$LSREGISTER" -f "$DEST" >/dev/null 2>&1 || true
  fi

  # Remove intermediate bundle to keep the target/ dir lean across rebuilds.
  rm -rf "$PROJ_DIR/src-tauri/target/$BUNDLE_SUBDIR"

  log_ok "Installed $DEST"
fi

# ── Summary ────────────────────────────────────────────────────────

BUILD_ELAPSED=$(( $(date +%s) - BUILD_START_EPOCH ))
printf '\n%s[leo build] Done in %ss%s\n' "$C_BOLD" "$BUILD_ELAPSED" "$C_RESET"
if [[ "$SKIP_INSTALL" == 1 ]]; then
  printf '%sArtifact:%s %s\n' "$C_DIM" "$C_RESET" "$APP_PATH"
else
  printf '%sInstalled:%s /Applications/%s.app\n' "$C_DIM" "$C_RESET" "$NEW_PRODUCT"
fi
[[ "$BUILD_DMG" == 1 ]] && printf '%sDMG:%s %s\n' "$C_DIM" "$C_RESET" "$DMG_OUT_DIR/$DMG_NAME"
