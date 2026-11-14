#!/usr/bin/env bash
# Claude Code Stop hook: an agent can't end its turn with Rust changes that fail the fast checks.
# Exit 2 hands the failure back to the agent to fix.
set -uo pipefail
input=$(cat)
# Already continuing because of this hook: let it stop, so a stuck failure can't loop forever.
if echo "$input" | grep -q '"stop_hook_active": *true'; then exit 0; fi
cd "$(dirname "$0")/../.." || exit 0
if [ -z "$(git status --porcelain -- '*.rs' 'Cargo.toml' 'Cargo.lock' '*.toml' 2>/dev/null)" ]; then exit 0; fi
[ -d /opt/homebrew/opt/rustup/bin ] && export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
if ! out=$(cargo xtask verify --fast 2>&1); then
  echo "cargo xtask verify --fast failed. Fix it before finishing:" >&2
  echo "$out" | grep -vE '^rune:' | tail -40 >&2
  exit 2
fi
exit 0
