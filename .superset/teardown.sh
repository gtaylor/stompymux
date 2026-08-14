#!/usr/bin/env bash
# Tear down a Superset workspace: stop this workspace's game server so it does
# not keep running (and keep holding its port) after the worktree is deleted.
set -euo pipefail

workspace="${SUPERSET_WORKSPACE_PATH:-$PWD}"
cd "$workspace"

pidfile=game/logs/superset-run.pid
if [[ -f "$pidfile" ]]; then
  pid=$(<"$pidfile")
  # Only touch it if it is still our stompymux, never some recycled PID.
  if [[ "$pid" =~ ^[0-9]+$ ]] && [[ "$(readlink -f "/proc/$pid/exe" 2>/dev/null || true)" == "$workspace/game/stompymux" ]]; then
    echo "==> Shutting down stompymux (pid $pid)"
    kill "$pid" 2>/dev/null || true
    for _ in {1..30}; do
      kill -0 "$pid" 2>/dev/null || break
      sleep 1
    done
    kill -9 "$pid" 2>/dev/null || true
  fi
  rm -f "$pidfile"
fi

# Build trees are large; drop them explicitly in case the worktree directory is
# not removed wholesale.
rm -rf .build .san-build .iwyu-build
rm -f game/stompymux.local.toml

echo "==> Teardown complete"
