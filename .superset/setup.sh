#!/usr/bin/env bash
# Prepare a fresh Superset workspace for stompymux development.
#
# A new worktree starts with empty third_party/ submodules, no build tree, and
# an empty (gitignored) game/data directory. This fills all three in.
set -euo pipefail

workspace="${SUPERSET_WORKSPACE_PATH:-$PWD}"
cd "$workspace"

echo "==> Initializing submodules"
git submodule update --init --recursive

echo "==> Installing documentation dependencies"
npm --prefix docs install

# game/data is gitignored, so a new worktree starts with no game database and
# the server bootstraps a fresh world on first start. That is the safe default:
# a branch that changes the schema will refuse to load the root checkout's
# older database. Set SUPERSET_COPY_GAME_DB=1 when you want to work against the
# root checkout's world instead (branch and database schema must agree).
root_db="${SUPERSET_ROOT_PATH:-}/game/data/stompymux.db"
workspace_db="game/data/stompymux.db"
if [[ "${SUPERSET_COPY_GAME_DB:-0}" == "1" && -f "$root_db" && ! -f "$workspace_db" ]]; then
  echo "==> Copying game database from $root_db"
  mkdir -p game/data
  # Use SQLite's online backup so a running server in the root checkout cannot
  # hand us a torn copy. Fall back to cp if sqlite3 is unavailable.
  if command -v sqlite3 >/dev/null 2>&1; then
    sqlite3 "$root_db" ".backup '$workspace_db'"
  else
    cp "$root_db" "$workspace_db"
  fi
else
  echo "==> Starting from an empty game/data; the server bootstraps a database on first run"
fi

echo "==> Building"
just build

echo "==> Installing into game/"
just install

echo "==> Workspace ready. Use the Run button (or 'just run') to start the game."
