set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

stylua := env("STYLUA", "stylua")

default: checks

checks: fmt-check test

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
