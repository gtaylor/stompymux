---
title: Development workflows
description: Build, test, and edit the Rust server and game Lua modules
type: docs
weight: 15
---

The Rust server lives in `src/`; game Lua modules, help content, maps, and
unit templates live under `game/`. Work from `stompymux-rs/` so Cargo and the
`justfile` resolve their paths correctly.

## Set up your environment

Run the setup entrypoint from the repository root:

```sh
.devcontainer/setup.sh
```

It detects which supported environment it is running in and runs the matching
script from `.devcontainer/`:

| Environment | Detected by | Script | What it does |
| --- | --- | --- | --- |
| CI | `GITHUB_ACTIONS=true` | `setup-ci.sh` | Verifies the devcontainer image has every required tool. |
| Cloud | `CLAUDE_CODE_REMOTE=true` | `setup-cloud.sh` | Installs the toolchain (Node under `~/.local`, ahead of the image's Node on `PATH`), docs dependencies, and Codex. |
| Local | anything else | `setup-local.sh` | Checks the toolchain, then installs docs dependencies, Codex, and Claude Code. |

Claude Code cloud sessions run `setup.sh cloud` automatically through a
`SessionStart` hook in `.claude/settings.json`; the hook does nothing outside
the cloud.

Pass `ci`, `local`, or `cloud` as the first argument (or set `STOMPYMUX_ENV`)
to override detection.

The devcontainer is optional for local development. When you open the
repository in it, the image already contains the toolchain and
`postCreateCommand` runs `setup.sh` for you. Outside the devcontainer, provide
the toolchain yourself first; on Debian or Ubuntu the devcontainer installers
can do it:

```sh
sudo .devcontainer/install-tools.sh
.devcontainer/install-rust.sh
.devcontainer/setup.sh
```

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
