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

Pass `local`, `claude-cloud`, or `codex-cloud` as the first argument (or set `STOMPYMUX_ENV`)
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
just test
cargo run -- serve --game-dir game-local
```

Create `game-local/` from `game/` as described in [Installation](./installation/)
when you want a disposable world for development. Stop and restart the server
to load a new Rust binary. `just build`, `just test`, and `just build-and-run`
are shortcuts; the last command uses the default `game/` directory.

Unit tests live beside their implementations. Integration scenarios and
fixtures live in `tests/`; the scenarios are grouped into sixteen consolidated
suites in the `stompymux-suites` package under `tests/suites/`, and each suite
is its own test binary. Tests that run the server's own executables live in
the server package's `cli` target under `tests/cli/`, because cargo only
exposes `CARGO_BIN_EXE_*` paths to that package's integration tests. Tests run
under [cargo-nextest](https://nexte.st/), which runs every test in its own
process and schedules all suites together. While iterating, build and run only
what your change touches:

```sh
just check                          # type-check every target, no codegen
just test-unit btech::los           # unit tests in src/, filtered by name
just test-scenario btech_status     # one scenario file from tests/
just test-suite btech_08 status     # one suite, filtered by test name
just list-scenarios                 # which suite includes which scenario
```

`just test-scenario` finds the suites that include the scenario module and
runs `cargo nextest run --test <suite> <scenario>::`, so only those suites'
binaries are built. Run the whole suite with `just test` before handing work back.

Workspace crates compile unoptimized and incrementally while dependencies are
fully optimized, so after the first build a small edit rebuilds everything in
well under a minute. The `dev` and `test` profiles are identical on purpose:
`cargo build`, `cargo run`, and `cargo test` then share one set of artifacts,
so switching between them never recompiles the server crate. For the same
reason the shared test helpers in `tests/support/` are a dependency of the
suites package, not a dev-dependency of the server: the server library's
unit-test build then compiles alongside the library instead of after it. Avoid
`cargo clean` unless the target directory is corrupt; it throws away the
optimized dependencies along with the incremental caches.

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
