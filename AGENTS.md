# stompymux-rs AGENTS.md

stompymux-rs is a Rust rewrite of stompymux, a C-based MUD server that includes a Battletech-based realtime combat system.

## Repository layout

- `game`: Files needed to run the game server.
- `game/help`: Markdown+TOML-frontmatter articles served by the `help` command.
- `game/text`: Other larger blocks of static text (MOTD, new user notices, etc)
- `docs`: Docs for the game server and its sources.
- `src`: Location of all Rust sources for the game and its supporting utilities.
- `src/btech`: Battletech extensions that layer on top of the base MUX game server.
- `crates/map`: Battlefield map data shared by the server and map tools: layered hexes, terrain, map flags, hex geometry, the TOML map file format, and the `map-check` CLI. The server re-exports these types from `src/btech`. Keep it free of the server crate and of optional features.
- `crates/mapgen`: Standalone map generation library and `mapgen` CLI. It writes map files through `crates/map`. Keep it free of dependencies on the server crate so map editors can embed it.
- `crates/mappy`: The Mappy map editor (`just mappy`). It builds on `crates/map` and iced, and is not a default workspace member, so `cargo build` and `just test` skip it; `just test-mappy` runs its tests.
- `crates/lua-tools`: `lua-type-updater` and `lua-doc-updater`, which read Rust sources as text. Keep them free of the server crate so they build in seconds.
- `tests`: Integration scenario files (`tests/*.rs`) and fixtures (`tests/fixtures`).
- `tests/suites`: The `stompymux-suites` package. Each suite binary compiles a group of scenario files from `tests/`.
- `tests/support`: The `stompymux-test-support` helper library the suites share. Never make it a dev-dependency of the server package: the server library's unit-test build would then wait for the library to finish compiling.
- `tests/cli`: The server package's one integration target, `cli`, for tests that run its executables through `CARGO_BIN_EXE_*`. It does not use `stompymux-test-support`.

## Principals

* We're pre-1.0 so you need not write migration, shim, or "legacy bridge" code because there are on the whole no existing production users of the codebase. Do not litter the project with code which "supports legacy users" or somesuch.
* Write all tools and misc scripts in rust instead of introducing Python. Place them in `src/bin/` when they need the server crate; tools that do not should live in their own crate under `crates/` so they never wait on a server build.

## Core workflow

1. Make your changes.
1. While iterating, build and run only what your change touches. Tests run under cargo-nextest.
   - `just check` type-checks every target without codegen (fastest feedback on compile errors).
   - `just test-unit <filter>` runs unit tests in `src/`; only the library's unit-test binary is built.
   - `just test-scenario <name>` runs one scenario file from `tests/` or `tests/cli/` (for example `just test-scenario btech_status`); only the suite binaries that include it are built. `just list-scenarios` shows which suite includes which scenario.
   - `just test-suite <suite> [filter]` runs one suite from `tests/suites/`, or the `cli` target.
1. Run `cargo fmt`, `just lint`, and `just test` before handing back to the human. CI also checks generated Lua types and docs, maps, and fails on any clippy warning.
1. Keep the `dev` and `test` Cargo profiles identical, and do not enable dependency features only under `[dev-dependencies]`. Either one makes `cargo build`, `cargo run`, and `cargo test` compile separate copies of the 200k-line server crate.

## Rust rules

* This code targets the 2024 Rust edition.
* Strongly prefer early returns. Avoid deep nesting.
* Crates should export their internals from their main lib.rs, and downstream users of those crates should not -- in general -- be going down into modules within the crate to access things.
* Avoid unsafe code blocks where possible.
* Leave a comment describing what each Rust file is for.
* Do not leave comments which refer to older ways of doing things.
* Document functions, types, and other variables with comments.
* Do make sure that major functions and modules have adequate Rustdoc.
* Add unit tests beside implementations (#[cfg(test)] modules) and integration tests under each crate’s tests/ directory.
* Ensure that there is a newline between Rust blocks (ex: functions, structs, enums, etc) for readability.

## Lua bindings and Lua code

- We use LuaJIT 2.1 for all Lua code. LuaJIT comes with non-standard [extensions](https://luajit.org/extensions.html) to the upstream Lua 5.1 language.
- Use [StyLua](https://github.com/JohnnyMorganz/StyLua) for code formatting.
- LuaLS stubs may be found in game/lua/types. These drive IDE validation and hover hinting.
- Run `just update-lua-types` any time you add, remove, or change inputs or outputs for Rust functions that are bound to Lua packages.
- Don't make any manual edits to docs/content/docs/scripting/packages. This is generated by `just update-lua-types`.
- Avoid magic strings in parameters. Instead export and accept constants. See the mux.world.access package as an example.

## Testing Practices

- Integration tests must not interact with the production `game/` directory. Copy fixtures to tests/fixtures/game and tests/fixtures instead.
- Avoid hardcoded sleeps where possible. Prefer watches and other techniques to keep our test suite time low.
- `tests` directory structure doesn't have to exactly match the source structure, but keep test suites grouped into topical subdirectories.