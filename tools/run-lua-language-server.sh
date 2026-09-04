#!/usr/bin/env bash
set -euo pipefail

readonly LUA_LANGUAGE_SERVER_VERSION="3.19.1"

declare -a candidates=()
if [[ -n "${LUA_LANGUAGE_SERVER:-}" ]]; then
  if resolved_override="$(command -v -- "${LUA_LANGUAGE_SERVER}" 2>/dev/null)"; then
    candidates+=("${resolved_override}")
  else
    echo "LUA_LANGUAGE_SERVER does not name an executable: ${LUA_LANGUAGE_SERVER}" >&2
    exit 1
  fi
else
  if path_candidate="$(command -v lua-language-server 2>/dev/null)"; then
    candidates+=("${path_candidate}")
  fi

  if [[ -n "${HOME:-}" ]]; then
    shopt -s nullglob
    candidates+=(
      "${HOME}"/.vscode/extensions/sumneko.lua-${LUA_LANGUAGE_SERVER_VERSION}-*/server/bin/lua-language-server
      "${HOME}"/.vscode-insiders/extensions/sumneko.lua-${LUA_LANGUAGE_SERVER_VERSION}-*/server/bin/lua-language-server
      "${HOME}"/.vscode-server/extensions/sumneko.lua-${LUA_LANGUAGE_SERVER_VERSION}-*/server/bin/lua-language-server
      "${HOME}"/.vscode-server-insiders/extensions/sumneko.lua-${LUA_LANGUAGE_SERVER_VERSION}-*/server/bin/lua-language-server
    )
    shopt -u nullglob
  fi
fi

declare -a rejected=()
for candidate in "${candidates[@]}"; do
  if [[ ! -x "${candidate}" ]]; then
    rejected+=("${candidate} (not executable)")
    continue
  fi

  candidate_version="$("${candidate}" --version 2>/dev/null || true)"
  candidate_version="${candidate_version%%$'\n'*}"
  if [[ "${candidate_version}" == "${LUA_LANGUAGE_SERVER_VERSION}" ]]; then
    exec "${candidate}" "$@"
  fi
  rejected+=("${candidate} (version ${candidate_version:-unknown})")
done

echo "LuaLS ${LUA_LANGUAGE_SERVER_VERSION} was not found." >&2
if ((${#rejected[@]} > 0)); then
  echo "Rejected candidates:" >&2
  printf '  %s\n' "${rejected[@]}" >&2
fi
echo "Install the pinned development tools or set LUA_LANGUAGE_SERVER to the LuaLS ${LUA_LANGUAGE_SERVER_VERSION} executable." >&2
exit 1
