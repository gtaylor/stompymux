#!/usr/bin/env bash

set -euo pipefail

root=${1:?repository root is required}
pattern='(^|[^[:alnum:]_])(strtok|localtime|ctime|inet_ntoa|strerror)[[:space:]]*\('

matches=$(rg -n --glob '*.[ch]' "$pattern" "$root/src") && status=0 || status=$?
case $status in
  0)
    printf '%s\n' "$matches"
    echo "direct use of a libc shared-state API is forbidden; use a reentrant or caller-buffered replacement" >&2
    exit 1
    ;;
  1)
    ;;
  *)
    echo "hidden-libc-state scan failed with status $status" >&2
    exit "$status"
    ;;
esac
