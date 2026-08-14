#!/usr/bin/env bash

set -euo pipefail

root=${1:?repository root is required}
build_root=${2:?build root is required}
allowlist=${3:?allowlist path is required}
query=${BTECH_CLANG_QUERY:-clang-query-22}
jobs=${BTECH_GLOBAL_SCAN_JOBS:-$(nproc)}
matcher='varDecl(hasGlobalStorage(), isDefinition(), unless(hasType(isConstQualified())), isExpansionInMainFile(), optionally(hasAncestor(functionDecl().bind("owner")))).bind("global")'

for tool in "$query" jq perl rg; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "writable-global check requires $tool" >&2
    exit 2
  fi
done
if [[ ! $jobs =~ ^[1-9][0-9]*$ ]]; then
  echo "BTECH_GLOBAL_SCAN_JOBS must be a positive integer" >&2
  exit 2
fi
if [[ ! -f $build_root/compile_commands.json ]]; then
  echo "writable-global check requires $build_root/compile_commands.json" >&2
  exit 2
fi

scan_root=$(mktemp -d)
cleanup() {
  rm -rf -- "$scan_root"
}
trap cleanup EXIT

parse_query_output() {
  perl -0777 -e '
    use strict;
    use warnings;
    my $root = shift @ARGV;
    my $text = do { local $/; <STDIN> };
    while ($text =~ /Match #[0-9]+:\s*(.*?)(?=\nMatch #[0-9]+:|\z)/sg) {
      my $block = $1;
      next unless $block =~ /Binding for "global":\n(VarDecl[^\n]*)/;
      my $decl = $1;
      next unless $decl =~ /<([^<>]+?):[0-9]+:[0-9]+(?:,[^>]*)?>/;
      my $path = $1;
      next unless $decl =~ /\b([A-Za-z_][A-Za-z0-9_]*) '\''[^'\'']+'\''/;
      my $symbol = $1;
      my $owner = "<file>";
      if ($block =~ /Binding for "owner":\n(FunctionDecl[^\n]*)/) {
        my $function = $1;
        $owner = $1
          if $function =~ /\b([A-Za-z_][A-Za-z0-9_]*) '\''[^'\'']+'\''/;
      }
      $path =~ s/^\Q$root\E\///;
      print "$path:$owner:$symbol\n";
    }
  ' "$root"
}

scan_source() {
  local source=$1
  "$query" -p "$build_root" "$source" \
    -c 'set bind-root false' -c 'set output dump' \
    -c "match $matcher" 2>/dev/null | parse_query_output
}

export root build_root query matcher
export -f parse_query_output scan_source

fixture=$scan_root/writable_global_fixture.c
printf '%s\n' \
  'const char *SHALLOW[] = {"value"};' \
  'const char *const DEEP[] = {"value"};' \
  'extern int PHANTOM;' \
  'static int RUNTIME;' \
  'static void first(void) { static char buffer[4]; (void)buffer; }' \
  'static void second(void) { static char buffer[4]; (void)buffer; }' \
  >"$fixture"
fixture_actual=$(scan_source "$fixture")
fixture_relative=${fixture#"$root"/}
if ! rg -Fxq "$fixture_relative:<file>:SHALLOW" <<<"$fixture_actual"; then
  echo "writable-global matcher failed to preserve the fixture path"
  exit 1
fi
for expected in '<file>:SHALLOW' '<file>:RUNTIME' 'first:buffer' \
                'second:buffer'; do
  if ! rg -q ":${expected}$" <<<"$fixture_actual"; then
    echo "writable-global matcher failed to report fixture: $expected"
    exit 1
  fi
done
if rg -q ':(DEEP|PHANTOM)$' <<<"$fixture_actual"; then
  echo "writable-global matcher reported an immutable or undefined fixture"
  exit 1
fi

jq -r '.[].file' "$build_root/compile_commands.json" |
  while IFS= read -r source; do
    if [[ $source != /* ]]; then
      source=$root/$source
    fi
    # In a case pattern, * also matches '/', so these cover every source depth.
    case "$source" in
      "$root"/src/btech/*.c|"$root"/src/mux/*.c)
        printf '%s\0' "$source"
        ;;
    esac
  done |
  sort -zu |
  xargs -0 -r -n 1 -P "$jobs" bash -c 'scan_source "$1"' _ |
  sort -u >"$scan_root/actual"

sed -E '/^[[:space:]]*(#|$)/d; s/[[:space:]]+#.*$//' "$allowlist" |
  sort -u >"$scan_root/allowed"

status=0
while IFS= read -r symbol; do
  echo "$symbol: writable global is not allowlisted"
  status=1
done < <(comm -23 "$scan_root/actual" "$scan_root/allowed")

while IFS= read -r symbol; do
  echo "$allowlist: stale writable-global allowlist entry: $symbol"
  status=1
done < <(comm -13 "$scan_root/actual" "$scan_root/allowed")

exit "$status"
