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
| [`mux.telnet`](telnet/) | Telnet protocol state and capabilities. |

## Types

The `mux` module provides several Lua types that are core to most scripting.

| Type | Description |
| --- | --- |
| [`Object`](type-object/) | A validated handle for a native database object. |
| [`Attribute`](type-attribute/) | Access to an object's supported native attributes. |
| [`State`](type-state/) | Typed, persistent state in one object namespace. |

## Functions

The `mux` module provides functions for common in-game operations, excluding the btech
economy and combat functionality (which is addressed through the [btech](../btech/) module).

### Objects and state

| Function | Description |
| --- | --- |
| [`mux.object`](object/) | Creates a validated handle for a database object. |

### Styled text

StompyMUX supports a rich set of facilities for styling text. Colors, decorations, clickable
links, and more. See [Styled Text](../../../concepts/styled-text/) for an overview.

| Function | Description |
| --- | --- |
| [`mux.is_printable_ascii`](is-printable-ascii/) | Tests whether a string contains only printable ASCII bytes. |
| [`mux.markup`](markup/) | Validates styled-text markup. |
| [`mux.style`](style/) | Applies styles described by an options table. |
| [`mux.strip_style`](strip-style/) | Removes markup and ANSI styling. |
| [`mux.text_width`](text-width/) | Returns the visible byte width of styled text. |
| [`mux.truncate_text`](truncate-text/) | Truncates styled text to a visible width. |

### Connections and flows

| Function | Description |
| --- | --- |
| [`mux.notify`](notify/) | Sends a message to an object. |
| [`mux.connected_players`](connected-players/) | Lists player connections visible to the normal `who` command. |
| [`mux.who_summary`](who-summary/) | Returns the non-privileged WHO summary. |
| [`mux.flow_start`](flow-start/) | Starts an interactive flow on a descriptor. |

### Server operations

| Function | Description |
| --- | --- |
| [`mux.log`](log/) | Appends a message to a named file under `game/logs/`. |

## Availability and limits

Runtime database operations are unavailable during `@lua/check`. The Lua
sandbox does not expose filesystem, operating-system, debugger, FFI, coroutine,
or dynamic code-loading APIs. VM memory and persistent-state limits still apply
while using these functions.
