set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

stylua := env("STYLUA", "stylua")

default: checks

checks: fmt-check lint check-lua-types check-lua-docs check-maps test

fmt: fmt-lua fmt-rust

fmt-lua:
    {{stylua}} --glob '**/*.lua' --glob '!game/lua/types/**/*.lua' -- game/lua

fmt-rust:
    cargo fmt

fmt-check: fmt-check-rust fmt-check-lua

fmt-check-lua:
    {{stylua}} --check --glob '**/*.lua' -- game/lua

fmt-check-rust:
    cargo fmt --check

# Run every test in the workspace except Mappy's, which would compile iced and
# wgpu; `just test-mappy` runs those. nextest runs each test in its own process
# and schedules all suites together, so one slow suite no longer serializes
# the rest. It does not run doctests, so those follow separately.
test:
    cargo nextest run --workspace --exclude stompymux-mappy
    cargo test --workspace --exclude stompymux-mappy --doc --quiet

# Run the Mappy map editor's tests.
test-mappy *filter:
    cargo nextest run -p stompymux-mappy {{filter}}

# Compile-only pass over every target; skips codegen so it is the fastest way
# to find type errors while iterating.

# Type-check every package in the workspace (Mappy included) with all its targets.
check:
    cargo check --workspace --all-targets --all-features

# Run clippy over every package, target and feature; any warning fails the recipe.
lint:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# Only the library's own unit-test binary is built for this recipe.

# Run the unit tests beside the sources in src/, e.g. `just test-unit btech::los`.
test-unit *filter:
    cargo nextest run --lib {{filter}}

# Only that suite's binary is built, not the others.

# Run one integration suite, e.g. `just test-suite btech_08 status`. Suites live in
# tests/suites/ (package stompymux-suites); `cli` is the server package's binary-driven target.
test-suite suite *filter:
    cargo nextest run --test {{suite}} {{filter}}

# Finds every suite that includes the scenario module, in tests/suites/ or the cli target in
# tests/cli/, so only those suites are built, then filters the run to the scenario's tests.

# Run one scenario file by module name, e.g. `just test-scenario btech_status`.
test-scenario scenario *filter:
    #!/usr/bin/env bash
    set -euo pipefail
    targets=()
    for suite in $(grep -l -E '^mod {{scenario}};$' tests/suites/*.rs tests/cli/main.rs || true); do
        name="$(basename "$suite" .rs)"
        [[ "$name" == main ]] && name=cli
        targets+=(--test "$name")
    done
    if [[ ${#targets[@]} -eq 0 ]]; then
        echo "error: no suite includes scenario '{{scenario}}'" >&2
        echo "hint: run 'just list-scenarios' to see scenario names" >&2
        exit 2
    fi
    echo "==> {{scenario}} lives in: ${targets[*]}"
    cargo nextest run "${targets[@]}" "{{scenario}}::{{filter}}"

# List every integration suite and the scenario files it includes.
list-scenarios:
    #!/usr/bin/env bash
    set -euo pipefail
    for suite in tests/suites/*.rs tests/cli/main.rs; do
        name="$(basename "$suite" .rs)"
        [[ "$name" == main ]] && name=cli
        echo "$name:"
        grep -E '^mod [a-z0-9_]+;$' "$suite" | sed -E 's/^mod ([a-z0-9_]+);$/  \1/'
    done

build:
    cargo build

docsite:
    npm --prefix docs run build

docsite-serve:
    npm --prefix docs run serve

# Generate a battlefield map file, e.g.
# `just mapgen generate --biome desert --settlement town@center -o game/maps/dunes.toml`.
mapgen *args:
    cargo run --quiet -p stompymux-mapgen --bin mapgen -- {{args}}

# Fail when any map file in game/maps would not load.
check-maps:
    cargo run --quiet -p stompymux-map --bin map-check -- game/maps

# Open the Mappy map editor on a map directory (default game/maps).
mappy dir="game/maps":
    cargo run --profile mappy -p stompymux-mappy --bin mappy -- {{dir}}

update-lua-docs:
    cargo run --quiet -p stompymux-lua-tools --bin lua-doc-updater -- --write

check-lua-docs:
    cargo run --quiet -p stompymux-lua-tools --bin lua-doc-updater -- --check

# Build the LuaLS declaration and Lua API reference generators.
build-lua-tools:
    cargo build -p stompymux-lua-tools

run:
    cargo run serve

build-and-run: build run

# Regenerate LuaLS libraries and their isolated test fixtures from Rust contracts.
update-lua-types:
    cargo run --quiet -p stompymux-lua-tools --bin lua-type-updater -- --write
    {{stylua}} --check game/lua/types/mux.d.lua game/lua/types/btech.d.lua

# Fail when any checked-in LuaLS library or test fixture is stale.
check-lua-types:
    cargo run --quiet -p stompymux-lua-tools --bin lua-type-updater -- --check
    {{stylua}} --check game/lua/types/mux.d.lua game/lua/types/btech.d.lua

# Runs a differential Lua parity probe against the pinned C reference.
# Requires the pinned C binary at ../btmux-khi/build/stompymux (revision
# 2bbbe6fcdbabe69e229d73089f44bf38f91c0591) and a built Rust debug binary
# (cargo build; target/debug/stompymux-rs).
parity PROBE='runtime_surface':
    python3 tools/lua_parity_probe.py --probe tests/fixtures/lua-probes/{{PROBE}}.lua

# Runs the runtime_surface probe with the documented Rust-only btech facade
# extensions permitted. Each extra key is classified by
# tests/fixtures/lua-probes/runtime_surface.allow so the probe still fails on
# any unexplained drift beyond the documented extension surface.
parity-runtime-surface:
    #!/usr/bin/env bash
    set -eu
    args=()
    while IFS= read -r line; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        args+=(--allow-rust-extra "$line")
    done < tests/fixtures/lua-probes/runtime_surface.allow
    python3 tools/lua_parity_probe.py --probe tests/fixtures/lua-probes/runtime_surface.lua "${args[@]}"

# Runs the mux_text probe with the documented AD-TEXT-EXTENSIONS-001 markdown
# surface classified as a permitted Rust-only extension entry.
parity-mux-text:
    python3 tools/lua_parity_probe.py --probe tests/fixtures/lua-probes/mux_text.lua \
        --allow-rust-extra 'extension_markdown.document_type={"bytes":"7573657264617461","type":"string"}'

# Explicit local acceptance matrices; small tool tests run through cargo test.
autopilot-acceptance *args:
    cargo run --bin autopilot-acceptance -- {{args}}
