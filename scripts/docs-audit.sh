#!/usr/bin/env bash
# Docs drift audit — verifies that every public symbol and CLI command
# referenced by the docs (website/, README.md, PARITY.md) still exists in
# the source. Prevents stale-symbol drift (see AGENTS.md "Docs Maintenance").
#
# Usage:
#   bash scripts/docs-audit.sh            # report, exit 1 on any stale ref
#
# What it checks:
#   1. Every `larastvel_core::<module>::<symbol>` path in the docs must
#      resolve to a module/symbol in crates/larastvel-core/src.
#   2. Every command in the CLI reference table must exist in cli.rs
#      (`#[command(name = "...")]` for colon commands, variant for plain).
#   3. `make <target>` rows must match a `MakeTarget` variant.
#
# When documenting a new API, run this script to confirm the docs' symbols
# match the source.

set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE="$ROOT/crates/larastvel-core/src"
CLI="$ROOT/crates/larastvel-cli/src"

DOC_PATHS=("$ROOT/README.md" "$ROOT/PARITY.md" "$ROOT/website/guide" "$ROOT/website/reference")

gaps=0
ok=0

fail() {
  printf 'MISSING %s\n' "$1"
  gaps=$((gaps + 1))
}

pass() {
  ok=$((ok + 1))
}

# --- 1. larastvel_core:: paths -------------------------------------------------

check_core_ref() {
  local ref="$1"
  local path="${ref#larastvel_core::}"
  local parts=()
  IFS='::' read -r -a parts <<<"$path"

  if [[ ${#parts[@]} -eq 1 ]]; then
    # Top-level re-export (function or macro) must exist in lib.rs.
    if grep -qE "\b${parts[0]}\b" "$CORE/lib.rs"; then pass; else fail "$ref"; fi
    return
  fi

  local module="${parts[0]}"
  local symbol="${parts[-1]}"
  local mod_file="$CORE/$module.rs"
  local mod_dir="$CORE/$module"

  if [[ -f "$mod_file" ]]; then
    if grep -qE "\b$symbol\b" "$mod_file"; then pass; else fail "$ref"; fi
  elif [[ -d "$mod_dir" ]]; then
    if grep -rqE --include='*.rs' "\b$symbol\b" "$mod_dir"; then pass; else fail "$ref"; fi
  elif grep -qE "\b$module\b" "$CORE/lib.rs"; then
    # Module re-exported wholesale from lib.rs (e.g. `pub use sea_orm`).
    pass
  else
    fail "$ref"
  fi
}

# --- 2/3. CLI commands ---------------------------------------------------------

check_cli_command() {
  local cmd="$1"
  if [[ "$cmd" == make* ]]; then
    local target="${cmd#make }"
    local pascal
    pascal="$(printf '%s' "${target%%-*}" | sed 's/^\(.\)/\U\1/')"
    if grep -qE "^[[:space:]]+$pascal\b" "$CLI/cli.rs"; then pass; else fail "CLI command $cmd"; fi
    return
  fi

  if [[ "$cmd" == *:* ]]; then
    if grep -q "name = \"$cmd\"" "$CLI/cli.rs"; then pass; else fail "CLI command $cmd"; fi
  else
    local pascal
    pascal="$(printf '%s' "$cmd" | sed 's/^\(.\)/\U\1/')"
    if grep -qE "^[[:space:]]+$pascal\b" "$CLI/cli.rs"; then pass; else fail "CLI command $cmd"; fi
  fi
}

# --- run ----------------------------------------------------------------------

echo "Docs drift audit"
echo "================="
echo "1) larastvel_core:: symbol references"
refs="$(grep -rhoE 'larastvel_core::[A-Za-z_][A-Za-z0-9_:]*' "${DOC_PATHS[@]}" | sort -u)"
if [[ -z "$refs" ]]; then
  echo "  (no larastvel_core:: references found)"
else
  while IFS= read -r ref; do
    check_core_ref "$ref"
  done <<<"$refs"
fi

echo "2) CLI reference table commands"
if [[ -f "$ROOT/website/reference/cli.md" ]]; then
  cmds="$(grep -oE '^\| `[a-z][a-z0-9:-]*` ' "$ROOT/website/reference/cli.md" | sed -E 's/^\| `([^`]+)` .*/\1/')"
  if [[ -z "$cmds" ]]; then
    echo "  (no CLI commands found in cli.md)"
  else
    while IFS= read -r cmd; do
      check_cli_command "$cmd"
    done <<<"$cmds"
  fi
fi

echo "================="
printf 'Summary: %d OK, %d missing\n' "$ok" "$gaps"
if [[ "$gaps" -gt 0 ]]; then
  echo "Stale references found — update the docs or the source."
  exit 1
fi
echo "No stale references — docs and source are in sync."
