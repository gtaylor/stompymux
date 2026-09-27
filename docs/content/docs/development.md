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
| Claude Code cloud | `CLAUDE_CODE_REMOTE=true` | `setup-claude-cloud.sh` | Installs the toolchain (Node under `~/.local`, ahead of the image's Node on `PATH`), Rust, docs dependencies, and Codex. |
| Codex cloud | explicit `codex-cloud` argument | `setup-codex-cloud.sh` | Installs the toolchain (Node under `~/.local`, ahead of nvm's Node on `PATH`), Rust, docs dependencies, and Claude Code. |
| Local | anything else | `setup-local.sh` | Checks the toolchain, then installs docs dependencies, Codex, and Claude Code. |

Claude Code cloud sessions run `setup.sh claude-cloud` automatically through a
`SessionStart` hook in `.claude/settings.json`; the hook does nothing outside
the cloud.

Codex cloud has no repository-level startup hook that runs with internet
access, so configure it in the Codex environment settings instead. Set both the
**Setup script** and the **Maintenance script** to:

```sh
.devcontainer/setup.sh codex-cloud
```

The script is safe to re-run. It persists `PATH` changes to `~/.bashrc`, since
exports from the setup script do not reach the agent phase.

Pass `ci`, `local`, `claude-cloud`, or `codex-cloud` as the first argument (or set `STOMPYMUX_ENV`)
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
fixtures live in `tests/`; the scenarios are grouped into sixteen consolidated
suites under `tests/suites/`, and each suite is its own test binary. A full
`cargo test` builds the library twice (once as the unit-test binary and once
for the suites to link), plus all sixteen suite binaries, so it is the slowest
loop available. While iterating, build and run only what your change touches:

```sh
just check                          # type-check every target, no codegen
just test-unit btech::los        # unit tests in src/, filtered by name
just test-scenario btech_status     # one scenario file from tests/
just test-suite btech_08 status     # one suite, filtered by test name
just list-scenarios                 # which suite includes which scenario
```

`just test-scenario` finds the suite that includes the scenario module and
runs `cargo test --test <suite> <scenario>::`, so only that suite's binary is
built. Run the whole suite with `just test` before handing work back.

The test profile in `Cargo.toml` compiles incrementally, so after the first
full build a small edit rebuilds in well under a minute; the incremental
caches under `target/` take several gigabytes, and `cargo clean` throws them
away along with the fifteen-minute cold build, so avoid it unless the target
directory is corrupt.

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
