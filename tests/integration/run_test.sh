#!/usr/bin/env bash
set -u -o pipefail

if (( $# < 4 )); then
  echo "usage: $0 SUITE BASE OVERLAY COMMAND [ARG ...]" >&2
  exit 2
fi

suite=$1
base=$2
overlay=$3
shift 3

artifact_root=${TMPDIR:-/tmp}
game_directory=$(mktemp -d "${artifact_root%/}/btmux-${suite}.XXXXXX") || exit 2

finish() {
  status=$?
  if (( status == 0 )) && [[ ${BTECH_KEEP_TEST_DIRS:-0} != 1 ]]; then
    rm -rf -- "$game_directory"
  else
    echo "Integration test artifacts retained at: $game_directory" >&2
  fi
}
trap finish EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

cp -a "$base/." "$game_directory/" || exit 2
if [[ $overlay != - ]]; then
  cp -a "$overlay/." "$game_directory/" || exit 2
fi

export BTECH_TEST_GAME_DIR=$game_directory
"$@" "$game_directory"
status=$?
exit "$status"
