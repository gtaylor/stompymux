---
title: "mux package"
linkTitle: "mux"
type: docs
weight: 10
sidebar_root_for: self
no_list: true
---

`mux` is the built-in server API available to every Lua module. It is supplied by the game server rather than loaded with `require`.

## Subpackages

| Package | Description |
| --- | --- |
| [`mux.comsys`](comsys/) | Trusted communication-channel management. |
| [`mux.config`](config/) | Read-only access to scalar server configuration. |
| [`mux.error`](error/) | Structured errors, checked error codes, and error-handling helpers. |
| [`mux.session`](session/) | Interactive flows and active player-session information. |
| [`mux.telnet`](telnet/) | Telnet protocol state and capabilities. |
| [`mux.text`](text/) | Styled-text validation, formatting, and measurement helpers. |
| [`mux.world`](world/) | Database objects and their persistent state. |

## Functions

- [`check_db`](check-db/)
- [`log`](log/)

See the [value types](types/) used in signatures and returned records.
