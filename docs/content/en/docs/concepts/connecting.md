---
title: Connecting and creating a character
linkTitle: Connecting
type: docs
weight: 15
---

The connect screen is menu-driven, backed by the same interactive-flow
engine [Lua uses](../../scripting/flows/) (`src/mux/network/input_flow.c`),
with the connect/create logic itself in
`src/mux/network/connect_flow.c`.

## Menu-driven connecting

Pressing enter on a blank line, or sending anything that isn't a recognized
`connect`/`create` command, shows a menu:

```text
c) Connect to an existing character
n) Create a new character
q) Quit
```

Choosing **c**onnect prompts for a character name, then a password, and
retries on a wrong password up to the server's configured retry limit before
disconnecting. Choosing **n**ew prompts for a name, a password, and a
confirmation of that password, retrying the password step on a mismatch.
Both paths end by handing off to the same login/creation logic a one-line
connect uses (below), so the menu is a different way to reach it, not a
separate implementation.

A partial one-line command, such as `connect somename` with no password,
skips straight to the right prompt instead of re-showing the menu.

## The one-line form

Existing clients and connect macros that blind-send
`connect <name> <password>` or `create <name> <password>` in a single line
keep working exactly as before - that form is detected up front and handled
synchronously, without ever starting the menu-driven flow. There is no
behavior change for anyone not using the new prompts.

## Rate limiting

A wrong password decrements a per-connection retry counter; hitting zero
disconnects. Separately, a global address-keyed token bucket throttles rapid
repeated connect/create attempts regardless of which form (one-line or
menu-driven) triggered them, disconnecting the attempt outright rather than
allowing a retry. Both apply identically no matter which path led to the
attempt.
