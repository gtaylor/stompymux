---
title: Development workflows
description: Build, test, and edit the Rust server and game Lua modules
type: docs
weight: 15
---

The Rust server lives in `src/`; game Lua modules, help content, maps, and
unit templates live under `game/`. Work from `stompymux-rs/` so Cargo and the
`justfile` resolve their paths correctly.

## Rust server workflow

Edit the relevant Rust module, then format and test the package:

```sh
cargo fmt
cargo test
cargo run -- serve --game-dir game-local
```

Create `game-local/` from `game/` as described in [Installation](./installation/)
when you want a disposable world for development. Stop and restart the server
to load a new Rust binary. `just build`, `just test`, and `just build-and-run`
are shortcuts; the last command uses the default `game/` directory.

Unit tests live beside their implementations. Integration scenarios and
fixtures live in `tests/`. To run one scenario, find its suite in
`tests/suites/` and filter by its module name, for example:

```sh
cargo test --test btech_08 btech_status::
```

`just checks` runs formatting, generated Lua type and API documentation checks,
and the Rust test suite.

## Lua workflow

Edit modules under `game/lua/` (or your copied game directory). Format Lua
sources with `just fmt-lua`. A Wizard can run `@lua/reload` to load the changed
modules without restarting the server. Reload diagnostics appear in the server
log and in the Wizard's response. See [Scripting](./scripting/) for the Lua
package API.

Rust-owned Lua declarations and generated reference pages have separate
updaters. After changing a Lua API contract, run `just update-lua-types` and
`just update-lua-docs`; `just check-lua-docs` checks the generated reference.

## Documentation workflow

The documentation site uses Hugo Extended with the Docsy theme. Install the
Node dependencies once, then build or serve the site:

```sh
npm --prefix docs ci
just docsite
just docsite-serve
```

Edit articles under `docs/content/`. The local preview opens at
[http://localhost:1313/](http://localhost:1313/).
