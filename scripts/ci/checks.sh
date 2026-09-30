#!/usr/bin/env bash
# The checks every pull request needs, on any OS (Git Bash on Windows). Vitest needs Node.js.
set -euo pipefail
cd "$(dirname "$0")/../.."

bun install --frozen-lockfile
bun run check
bun run test
cd src-tauri
cargo clippy --all-targets --features dev-tools -- -D warnings
cargo test --features dev-tools
