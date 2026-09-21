set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

stylua := env("STYLUA", "stylua")

default: checks

checks: fmt-check check-lua-types test

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

test:
    cargo test

build:
    cargo build

run:
    cargo run serve

build-and-run: build run

# Regenerate LuaLS libraries and their isolated test fixtures from Rust contracts.
update-lua-types:
    cargo run --quiet --bin lua-type-updater -- --write
    {{stylua}} --check game/lua/types/mux.d.lua game/lua/types/btech.d.lua

# Fail when any checked-in LuaLS library or test fixture is stale.
check-lua-types:
    cargo run --quiet --bin lua-type-updater -- --check
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
