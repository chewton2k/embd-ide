#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
echo "→ Running Rust tests..."
(cd src-tauri && cargo test --quiet)
echo "→ Running frontend tests..."
npx vitest run
echo "→ Type-checking..."
npx svelte-check --threshold error
echo "→ Rust check..."
(cd src-tauri && cargo check --quiet)
echo "✓ All checks passed."
