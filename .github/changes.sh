#!/usr/bin/env bash
# The checks a pull request needs, from the paths it changes (one per line on stdin, as
# `git diff --name-only` prints them), as GitHub Actions outputs:
#   checks=true  it changes more than docs and the release workflow: `cargo xtask verify` on Linux
#   macos=true   it changes more than those and xtask: verify on macOS too
#   proof=true   it changes what people see: its body must carry proof. These are the crates
#                xtask/src/pr.rs calls USER_FACING; change both together.
# ci.yml and proof.yml skip a job whose output is false. A skipped job counts as passed for main's
# required checks, so a PR of docs alone can merge without waiting on builds.
set -euo pipefail

checks=false
macos=false
proof=false
while IFS= read -r path || [ -n "$path" ]; do
  case "$path" in
    crates/gxm-app/* | crates/gxm-journeys/* | crates/gxm-core/* | crates/gxm-cli/*) proof=true ;;
  esac
  case "$path" in
    # The release workflow runs only on a release, so a PR's checks can't try it.
    *.md | .claude/* | LICENSE* | .github/workflows/release.yml) continue ;;
  esac
  checks=true
  case "$path" in
    xtask/*) ;;
    *) macos=true ;;
  esac
done

printf 'checks=%s\nmacos=%s\nproof=%s\n' "$checks" "$macos" "$proof"
