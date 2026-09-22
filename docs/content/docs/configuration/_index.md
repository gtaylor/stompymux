---
title: Configuration
linkTitle: Configuration
description: Set up the Rust game server
type: docs
weight: 20
---

The server loads `stompymux.toml` from the directory passed to `--game-dir`
(default `game/`). The shipped file includes `aliases.toml` and configures
the listener, SQLite database, Lua source directory, BattleTech assets, and
runtime limits. Paths to game content are resolved from that game directory;
include paths are resolved from the file that declares them.

[The TOML reference](./stompymux-toml/) describes the available sections and
first-run bootstrap. Command-line `--listen-address` and `--port` values
override the corresponding TOML settings for that run. Wizards can inspect
and edit supported live settings with `@admin`; those edits do not rewrite the
configuration file and are lost on restart.

[Build options](./compile-time-directives/) covers Cargo builds and the
optional source-generation tools.
