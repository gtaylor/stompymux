---
title: mux package
linkTitle: mux
type: docs
weight: 10
sidebar_root_for: self
no_list: true
---

`mux` is the built-in server API available to every Lua module. It is supplied
by the game server rather than loaded with `require`.

## Subpackages

| Package | Description |
| --- | --- |
| [`mux.error`](error/) | Structured errors, checked error codes, and error-handling helpers. |
| [`mux.config`](config/) | Read-only access to scalar server configuration. |
| [`mux.comsys`](comsys/) | Trusted communication-channel management. |
| [`mux.telnet`](telnet/) | Telnet protocol state and capabilities. |
| [`mux.session`](session/) | Interactive flows and active player-session information. |
| [`mux.text`](text/) | Styled-text validation, formatting, and measurement helpers. |
| [`mux.world`](world/) | Database objects and their persistent state. |

## Functions

The root `mux` module provides functions for common in-game operations that do
not yet belong to a larger subpackage.

| Function | Description |
| --- | --- |
| [`mux.check_db`](check-db/) | Checks and repairs the game database without notifying a player. |
| [`mux.log`](log/) | Appends a message to a named file under `game/logs/`. |

## Availability and limits

Runtime database operations are unavailable during `@lua/check`. The Lua
sandbox does not expose filesystem, operating-system, debugger, FFI, coroutine,
or dynamic code-loading APIs. VM memory and persistent-state limits still apply
while using these functions.
