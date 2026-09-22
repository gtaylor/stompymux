---
title: Installation
description: Build and run the Rust StompyMUX server
type: docs
weight: 10
---

## Requirements

StompyMUX currently targets Linux. Install a current stable Rust toolchain
(including Cargo), a C compiler and `make` for vendored LuaJIT, and Git for
obtaining the source. SQLite and LuaJIT are built through Cargo dependencies;
you do not need to install their development headers separately.

The optional [`just`](https://github.com/casey/just) task runner provides
shortcuts for common commands. To build the documentation site, also install
Node.js/npm, Go, and Hugo Extended 0.164.0 or newer.

## Build the server

From the `stompymux-rs/` directory of your checkout:

```sh
cargo build
```

For an optimized binary, run `cargo build --release`. The Cargo package uses
the Rust 2024 edition. No CMake build or Git submodule update is needed.

## Run a game directory

The repository includes `game/` with configuration, Lua modules, assets, and
an existing SQLite world at `game/data/stompymux.db`. To keep the supplied
world intact while trying the server, make a copy and run that copy:

```sh
cp -a game game-local
cargo run -- serve --game-dir game-local
```

The server reads `game-local/stompymux.toml` and saves supported world changes
to the configured SQLite database. Run only one server against a given database.
The stock configuration listens on `127.0.0.1:5555`. You can override the
listener for one run:

```sh
cargo run -- serve --game-dir game-local --listen-address 127.0.0.1 --port 5556
```

Connect with a Telnet-capable MUD client at the configured address and port.
Use the credentials for an existing player in the copied world.

## Start a new world

To bootstrap a new game in the copied directory, move its existing database
out of the configured path before starting the server:

```sh
mv game-local/data/stompymux.db game-local/data/stompymux.db.backup
cargo run -- serve --game-dir game-local
```

When the configured database is absent, startup creates the schema and the
configured bootstrap objects. It writes distinct `GOD` and `Wizard` passwords
to `game-local/bootstrap-credentials.txt` with owner-only permissions. Keep
that file private. After logging in as `GOD`, use
`@newpassword <player>=<password>` to replace the initial passwords. If a credentials
file already exists, move it aside before bootstrapping; startup will not
overwrite it. An existing empty or invalid database is rejected rather than
replaced.

## Next steps

- [Development workflows](./development/)
- [Configuration](./configuration/)
