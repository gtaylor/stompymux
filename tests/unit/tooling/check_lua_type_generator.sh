#!/usr/bin/env bash
set -euo pipefail

generator=$(realpath "$1")
repository=$(realpath "$2")
fixtures="$repository/tests/fixtures/lua_type_generator"
workspace=$(mktemp -d)
trap 'rm -rf -- "$workspace"' EXIT

mkdir -p "$workspace/src/mux/lua/packages/mux" "$workspace/src/btech" \
  "$workspace/include"
cp "$fixtures/include/lua_fixture.h" "$workspace/include/"
cp "$fixtures/include/selector_macro_registration.h" "$workspace/include/"
cp "$fixtures/shared_contract.h" "$workspace/src/mux/lua/packages/mux/"
cp "$fixtures/valid_mux.c" "$workspace/src/mux/lua/packages/mux/"
cp "$fixtures/valid_mux_extension.c" \
  "$workspace/src/mux/lua/packages/mux/"
cp "$fixtures/valid_btech.c" "$workspace/src/btech/"
cp "$fixtures/selector_irrelevant.c" "$workspace/src/btech/"

clang-format-22 --dry-run --Werror "$fixtures/valid_mux.c" \
  "$fixtures/valid_mux_extension.c" "$fixtures/valid_btech.c" \
  "$fixtures/shared_contract.h" \
  "$fixtures/include/selector_macro_registration.h" \
  "$fixtures/selector_catalog_only.c" \
  "$fixtures/selector_irrelevant.c" \
  "$fixtures/selector_macro_missing_contract.c" \
  "$fixtures/selector_macro_registration.c" \
  "$fixtures/selector_missing_command.c" \
  "$fixtures/selector_nested_registration.c" \
  "$fixtures/selector_unexpected_registration.c" \
  "$fixtures/selector_unvisited_contract.h"

common=(
  --repo-root "$workspace"
  -p "$workspace"
)
compiler_flags=(
  --
  -std=c23
  -I"$workspace/include"
  -I"$workspace/src/mux/lua/packages/mux"
)
mux_source="$workspace/src/mux/lua/packages/mux/valid_mux.c"
extension_source="$workspace/src/mux/lua/packages/mux/valid_mux_extension.c"
btech_source="$workspace/src/btech/valid_btech.c"
irrelevant_source="$workspace/src/btech/selector_irrelevant.c"

(
  umask 077
  "$generator" --write --output-dir output-a "${common[@]}" \
    "$mux_source" "$extension_source" "$btech_source" \
    "$irrelevant_source" \
    "${compiler_flags[@]}"
) >"$workspace/write.log" 2>&1
test ! -s "$workspace/write.log"
test "$(stat -c '%a' "$workspace/output-a/mux.d.lua")" = 644
test "$(stat -c '%a' "$workspace/output-a/btech.d.lua")" = 644
cmp "$fixtures/expected/mux.d.lua.expected" "$workspace/output-a/mux.d.lua"
cmp "$fixtures/expected/btech.d.lua.expected" "$workspace/output-a/btech.d.lua"

# Header contracts must be reached through at least one selected translation
# unit. The shared contract above is exercised by the successful generation;
# this deliberately unreferenced header must fail closed.
cp "$fixtures/selector_unvisited_contract.h" \
  "$workspace/src/mux/lua/packages/mux/"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$mux_source" "$extension_source" "$btech_source" \
  "${compiler_flags[@]}" >"$workspace/unvisited-header.log" 2>&1; then
  echo "unvisited header contract unexpectedly passed" >&2
  exit 1
fi
grep -F 'LuaLS contract marker was not visited by a selected translation unit' \
  "$workspace/unvisited-header.log"
rm "$workspace/src/mux/lua/packages/mux/selector_unvisited_contract.h"

# A macro can hide the calls which form a registration, so every recognized
# binding-directory source is selected even when no registration token occurs
# in its unexpanded text.
macro_source="$workspace/src/mux/lua/packages/mux/selector_macro_registration.c"
cp "$fixtures/selector_macro_registration.c" "$macro_source"
"$generator" --write --output-dir output-macro "${common[@]}" \
  "$mux_source" "$extension_source" "$btech_source" "$macro_source" \
  "${compiler_flags[@]}"
grep -F 'function mux.macro_fixture() end' \
  "$workspace/output-macro/mux.d.lua"
rm "$macro_source"

# This source has neither a contract marker nor a literal registration token:
# the path rule alone must select it, expand the header macro, and find the
# undisposed registration.
macro_missing="$workspace/src/mux/lua/packages/mux/selector_macro_missing_contract.c"
cp "$fixtures/selector_macro_missing_contract.c" "$macro_missing"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$macro_missing" "${compiler_flags[@]}" \
  >"$workspace/macro-missing.log" 2>&1; then
  echo "macro registration without a contract unexpectedly passed" >&2
  exit 1
fi
grep -F 'registered MUX handler/leaf lacks a LuaLS callable' \
  "$workspace/macro-missing.log"
rm "$macro_missing"

# All MUX Lua implementation files are selected, so a header-hidden callback
# cannot evade the out-of-scope guard merely by living beside the recognized
# public package directories.
outside_macro="$workspace/src/mux/lua/selector_macro_missing_contract.c"
cp "$fixtures/selector_macro_missing_contract.c" "$outside_macro"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$outside_macro" "${compiler_flags[@]}" \
  >"$workspace/outside-macro.log" 2>&1; then
  echo "out-of-scope header macro unexpectedly passed" >&2
  exit 1
fi
grep -F 'Lua C callback appears outside a recognized source scope' \
  "$workspace/outside-macro.log"
rm "$outside_macro"

# Registrations embedded in expressions are unsupported rather than silently
# ignored by the direct-statement pairing logic.
nested_registration="$workspace/src/mux/lua/packages/mux/selector_nested_registration.c"
cp "$fixtures/selector_nested_registration.c" "$nested_registration"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$nested_registration" "${compiler_flags[@]}" \
  >"$workspace/nested-registration.log" 2>&1; then
  echo "nested callback registration unexpectedly passed" >&2
  exit 1
fi
grep -F 'unsupported Lua C callback registration pattern' \
  "$workspace/nested-registration.log"
rm "$nested_registration"

# Catalog owners are selected by their native declaration spelling even when
# they have no contract marker of their own.
catalog_only="$workspace/src/btech/selector_catalog_only.c"
cp "$fixtures/selector_catalog_only.c" "$catalog_only"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$catalog_only" "${compiler_flags[@]}" \
  >"$workspace/catalog-only.log" 2>&1; then
  echo "catalog-only source unexpectedly passed" >&2
  exit 1
fi
grep -F 'native Lua catalog lacks a LuaLS catalog block' \
  "$workspace/catalog-only.log"
rm "$catalog_only"

# A public table registration outside the recognized MUX binding scopes is an
# error rather than an implicitly omitted API surface.
unexpected_registration="$workspace/src/btech/selector_unexpected_registration.c"
cp "$fixtures/selector_unexpected_registration.c" "$unexpected_registration"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$unexpected_registration" "${compiler_flags[@]}" \
  >"$workspace/unexpected-registration.log" 2>&1; then
  echo "out-of-scope registration unexpectedly passed" >&2
  exit 1
fi
grep -F 'Lua C callback appears outside a recognized source scope' \
  "$workspace/unexpected-registration.log"

# The generic BTech dispatcher is a narrow exception, not an exemption for
# arbitrary direct callback registration in the entire BTech binding tree.
btech_unexpected="$workspace/src/mux/lua/packages/btech/selector_unexpected_registration.c"
mkdir -p "$(dirname "$btech_unexpected")"
cp "$fixtures/selector_unexpected_registration.c" "$btech_unexpected"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$btech_unexpected" "${compiler_flags[@]}" \
  >"$workspace/btech-unexpected-registration.log" 2>&1; then
  echo "direct BTech-tree callback unexpectedly passed" >&2
  exit 1
fi
grep -F 'Lua C callback appears outside a recognized source scope' \
  "$workspace/btech-unexpected-registration.log"
rm "$btech_unexpected"
rm "$unexpected_registration"

# A selected source without a compilation command must fail before Clang is
# invoked. The database intentionally contains only the irrelevant source.
missing_command="$workspace/src/btech/selector_missing_command.c"
cp "$fixtures/selector_missing_command.c" "$missing_command"
mkdir "$workspace/missing-command-db"
printf '[{"directory":"%s","file":"%s","arguments":["clang-22","-c","%s"]}]\n' \
  "$workspace" "$irrelevant_source" "$irrelevant_source" \
  >"$workspace/missing-command-db/compile_commands.json"
if "$generator" --check --output-dir output-a --repo-root "$workspace" \
  -p "$workspace/missing-command-db" "$missing_command" \
  >"$workspace/missing-command.log" 2>&1; then
  echo "selected source without a compilation command unexpectedly passed" >&2
  exit 1
fi
grep -F 'selected Lua type source has no compilation command' \
  "$workspace/missing-command.log"
rm "$missing_command"

unreadable_source="$workspace/src/btech/selector_unreadable.c"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$unreadable_source" "${compiler_flags[@]}" \
  >"$workspace/unreadable-source.log" 2>&1; then
  echo "unreadable source candidate unexpectedly passed" >&2
  exit 1
fi
grep -F 'cannot read Lua type generator source candidate' \
  "$workspace/unreadable-source.log"

mux_identity=$(stat -c '%i:%Y' "$workspace/output-a/mux.d.lua")
btech_identity=$(stat -c '%i:%Y' "$workspace/output-a/btech.d.lua")
"$generator" --write --output-dir output-a "${common[@]}" \
  "$mux_source" "$extension_source" "$btech_source" \
  "${compiler_flags[@]}"
test "$mux_identity" = "$(stat -c '%i:%Y' "$workspace/output-a/mux.d.lua")"
test "$btech_identity" = \
  "$(stat -c '%i:%Y' "$workspace/output-a/btech.d.lua")"

"$generator" --check --output-dir output-a "${common[@]}" \
  "$btech_source" "$extension_source" "$mux_source" \
  "${compiler_flags[@]}"
"$generator" --write --output-dir output-b "${common[@]}" \
  "$btech_source" "$extension_source" "$mux_source" \
  "${compiler_flags[@]}"
cmp "$workspace/output-a/mux.d.lua" "$workspace/output-b/mux.d.lua"
cmp "$workspace/output-a/btech.d.lua" "$workspace/output-b/btech.d.lua"

cp "$workspace/output-a/mux.d.lua" "$workspace/expected-mutated-mux"
sed -i 's/---Returns a fixture value\./---Returns an updated fixture value./' \
  "$workspace/expected-mutated-mux"
sed -i 's/---Returns a fixture value\./---Returns an updated fixture value./' \
  "$mux_source"
mkdir "$workspace/output-mutated"
cp "$workspace/output-a/mux.d.lua" "$workspace/output-mutated/mux.d.lua"
cp "$workspace/output-a/btech.d.lua" "$workspace/output-mutated/btech.d.lua"
chmod 0640 "$workspace/output-mutated/mux.d.lua"
"$generator" --write --output-dir output-mutated "${common[@]}" \
  "$mux_source" "$extension_source" "$btech_source" \
  "${compiler_flags[@]}"
cmp "$workspace/expected-mutated-mux" \
  "$workspace/output-mutated/mux.d.lua"
cmp "$workspace/output-a/btech.d.lua" \
  "$workspace/output-mutated/btech.d.lua"
test "$(stat -c '%a' "$workspace/output-mutated/mux.d.lua")" = 640
cp "$fixtures/valid_mux.c" "$mux_source"

printf '\n' >>"$workspace/output-a/mux.d.lua"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$mux_source" "$extension_source" "$btech_source" \
  "${compiler_flags[@]}" \
  >"$workspace/stale.log" 2>&1; then
  echo "stale generated output unexpectedly passed" >&2
  exit 1
fi
grep -F 'generated Lua type file is stale' "$workspace/stale.log"

cp "$fixtures/missing_contract.c" "$workspace/src/mux/lua/packages/mux/"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$workspace/src/mux/lua/packages/mux/missing_contract.c" \
  "${compiler_flags[@]}" \
  >"$workspace/missing.log" 2>&1; then
  echo "missing callable contract unexpectedly passed" >&2
  exit 1
fi
grep -F 'registered MUX handler/leaf lacks a LuaLS callable' \
  "$workspace/missing.log"

cp "$fixtures/malformed_contract.c" "$workspace/src/mux/lua/packages/mux/"
if "$generator" --check --output-dir output-a "${common[@]}" \
  "$workspace/src/mux/lua/packages/mux/malformed_contract.c" \
  "${compiler_flags[@]}" \
  >"$workspace/malformed.log" 2>&1; then
  echo "malformed LuaLS contract unexpectedly passed" >&2
  exit 1
fi
grep -F 'malformed LuaLS contract header' "$workspace/malformed.log"

expect_failure() {
  local scope=$1
  local fixture=$2
  local message=$3
  local transformation=${4-}
  local destination="$workspace/$scope/$fixture"
  local log="$workspace/${fixture%.c}.log"

  mkdir -p "$(dirname "$destination")"
  cp "$fixtures/$fixture" "$destination"
  if [[ -n "$transformation" ]]; then
    sed -i "$transformation" "$destination"
  fi
  if "$generator" --check --output-dir output-a "${common[@]}" \
    "$destination" "${compiler_flags[@]}" >"$log" 2>&1; then
    echo "$fixture unexpectedly passed" >&2
    exit 1
  fi
  grep -F "$message" "$log"
}

expect_failure src/mux/lua/packages/mux duplicate_definition.c \
  'duplicate LuaLS definition key'
expect_failure src/mux/lua/packages/mux cross_section_duplicate.c \
  'duplicate LuaLS definition key'
expect_failure src/mux/lua/packages/mux unsupported_payload.c \
  'unsupported LuaLS payload line'
expect_failure src/mux/lua/packages/mux wrong_section_payload.c \
  'LuaLS payload line is not allowed in binding blocks'
expect_failure src/mux/lua/packages/mux embedded_meta.c \
  'LuaLS payload may not own the generated envelope'
expect_failure src/mux/lua/packages/mux embedded_return.c \
  'LuaLS payload may not own the generated envelope'
expect_failure src/mux/lua/packages/mux crlf_contract.c \
  'LuaLS contract contains a carriage return' $'s/$/\r/'
expect_failure src/mux/lua/packages/mux tab_payload.c \
  'LuaLS payload contains CR, tab, or trailing whitespace' $'s/@TAB@/\t/'
expect_failure src/mux/lua/packages/mux trailing_whitespace.c \
  'LuaLS payload contains CR, tab, or trailing whitespace' \
  's/@TRAILING@/ /'
expect_failure src/mux/lua/packages/mux empty_ignore.c \
  'LuaLS ignore reason must contain a non-whitespace character' \
  's/@SPACES@/   /'
expect_failure src/mux/lua/packages/mux orphan_comment.c \
  'LuaLS contract comment is not attached to a declaration'
expect_failure src/mux/lua/packages/mux orphan_callable.c \
  'MUX callable is not backed by a recognized registration'
expect_failure src/mux/lua/packages/mux stale_ignore.c \
  'MUX ignore is not backed by a recognized registration'
expect_failure src/mux/lua/packages/mux unresolved_registration.c \
  'MUX registration leaf is not a string literal'
expect_failure src/mux/lua/packages/mux unresolved_installer.c \
  'could not resolve Lua installer function parameter'
expect_failure src/mux/lua/packages/mux unrecognized_member_registration.c \
  'MUX registration handler could not be resolved'
expect_failure src/mux/lua/packages/mux bad_callable_name.c \
  'callable key and emitted function have different leaves'
expect_failure src/btech btech_bad_name.c \
  'BtechLuaEntry short name differs from qualified leaf'
expect_failure src/btech btech_missing_contract.c \
  'BtechLuaEntry lacks its exact LuaLS callable contract'
expect_failure src/btech btech_orphan_callable.c \
  'BTech callable does not exactly match a BtechLuaEntry'
expect_failure src/btech btech_malformed_entry.c \
  'malformed BtechLuaEntry initializer'
expect_failure src/btech btech_package_mismatch.c \
  'BtechLuaEntry qualified name differs from installer package/name'
expect_failure src/mux/lua/packages/mux catalog_mismatch.c \
  'LuaLS catalog differs from native values; missing: LOUD; extra: EXTRA'
expect_failure src/mux/lua/packages/mux object_type_mismatch.c \
  'LuaLS catalog differs from native values; missing: THING; extra: EXIT'
expect_failure src/mux/lua/packages/mux catalog_malformed_row.c \
  'native Lua catalog row field is not a string literal'
expect_failure src/mux/lua/packages/mux catalog_duplicate_value.c \
  'LuaLS catalog contains a duplicate projected value'
expect_failure src/mux/lua/packages/mux error_root_mismatch.c \
  'LuaLS error catalog root fields differ from native values; missing: fixture; extra: wrong'
expect_failure src/mux/lua/packages/mux error_nested_field_mismatch.c \
  'LuaLS error catalog tree differs from native values; missing: mux.state.invalid; extra: mux.state.wrong'
expect_failure src/mux/lua/packages/mux error_unknown_root.c \
  'native Lua error catalog entry has an unsupported root: unknown.failure'
