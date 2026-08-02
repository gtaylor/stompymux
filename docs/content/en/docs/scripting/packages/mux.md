---
title: mux package
linkTitle: mux
type: docs
weight: 10
---

# `mux`

`mux` is the built-in table available to every Lua module. It is supplied by
the game server rather than loaded with `require`. Lua callbacks and commands
run with the server's `#1` authority model; scripts should still use the
callback context to identify the object and enactor that triggered them.

## `mux.object(dbref)`

Returns an object handle. Handles validate the referenced dbref whenever they
are used. Two handles for the same object compare equal; use the `dbref`
property when passing an object to an API outside `mux`.

```lua
local object = mux.object(ctx.object)

mux.notify(ctx.enactor, object.name)
```

An object handle has these read-only properties:

- `dbref`
- `name`
- `type`: `room`, `thing`, `exit`, or `player`
- `description`: the native description or `nil`
- `inside_description`: the native inside description or `nil`

`object:contents()` and `object:exits()` return unfiltered arrays of object
handles in native database order. The corresponding
`object:contents_visible(viewer, member)` and
`object:exits_visible(viewer, exit)` methods apply native look visibility
rules. A member or exit must be attached directly to the receiving object.

```lua
local room = mux.object(ctx.object)

for _, member in ipairs(room:contents()) do
  if room:contents_visible(ctx.enactor, member) then
    mux.notify(ctx.enactor, member.name)
  end
end
```

`exit:enter_lock_passes(enactor)` tests an exit's default traversal lock
without sending lock messages or moving the enactor.

Methods accepting objects permit either an object handle or a dbref. Passing
an invalid or garbage object raises a Lua error.

## Typed persistent state

`object:state(namespace)` returns persistent state belonging to one named
subsystem on the object. Namespaces and keys are exact and case-sensitive.
They begin with an ASCII letter and may contain letters, digits, `_`, `-`,
`.`, and `/`.

```lua
local state = mux.object(ctx.object):state("counter")
local count = state:get("count", 0) + 1

state:set("count", count)
```

Values retain their Lua scalar type across reloads and server restarts.
Supported values are strings, booleans, integers, and finite numbers. Empty
strings are values. `state:set(key, nil)` and `state:delete(key)` delete a
value; `delete` reports whether the key existed.

The complete state API is:

- `state:get(key[, default])`
- `state:has(key)`
- `state:set(key, value)`
- `state:delete(key)`
- `state:keys()`
- `state:entries()`, returning `{ key = ..., value = ... }` records
- `state:get_many(keys)`, returning a key-to-value table for present keys
- `state:set_many(values)`

Enumeration is sorted by key. A bulk update is part of the same callback
transaction as every other state write.

Every Lua callback has an implicit transaction. Reads observe writes made
earlier in that callback, including writes to multiple objects. A successful
callback commits all of them together. A Lua error or memory-limit error
discards all state writes made by the callback. Do not
hold transactions across flow steps or other player input; persist an explicit
reservation instead.

The configured per-value, per-object entry, and per-object byte limits apply
before data leaves the Lua VM. Exceeding a limit raises a Lua error.

## Styled text

`mux.is_printable_ascii(value)` returns `true` when every byte in the string is
between space (`0x20`) and `~` (`0x7e`), inclusive. The empty string is valid;
control bytes, embedded NUL bytes, DEL, and non-ASCII UTF-8 characters return
`false`. This is useful when validating player-facing identifiers.

```lua
assert(mux.is_printable_ascii("Atlas-1"))
assert(not mux.is_printable_ascii("caf\u00e9"))
```

`mux.markup(value)` validates and returns the same styled-text markup accepted by
`@name`, `@desc`, and `@idesc`.

```lua
local heading = mux.markup("[fg=#ff7000][bold]Warning[/][/]")
```

The same function validates OSC 8 link markup. Lua constructs links directly;
there are no separate hyperlink functions.

```lua
local look = mux.markup('[send="look"]Look[/]')
local website = mux.markup('[link="https://example.com"]Website[/]')
```

`[send]` and `[prompt]` accept raw commands and percent-encode them when
rendered. `[link]` accepts `http:`, `https:`, and `ftp:` URIs. Escape quotes and
backslashes in dynamic targets before interpolating them into the quoted tag.
Unsupported clients receive only the visible contents.

`mux.style(value, options)` applies styles without constructing markup.
`foreground` and `background` accept built-in or configured color names, or
`#RRGGBB`; `bold`, `underline`, and `inverse` are booleans.

```lua
local heading = mux.style("Warning", {
  foreground = "#ff7000",
  bold = true,
})
```

`mux.strip_style(value)` removes markup and ANSI styling.
`mux.text_width(value)` returns the visible byte width without counting either
representation.
`mux.truncate_text(value, width)` truncates to a visible width while retaining
and safely resetting active styles. These helpers are intended for appearance
layouts containing styled object names.

Styled Lua output is adapted to each recipient's negotiated terminal color
depth when sent with `mux.notify` or returned from an appearance hook.

## `mux.notify(object, message)`

Sends a message to an object, normally the triggering player.

```lua
mux.notify(ctx.enactor, "The counter advances.")
```

`object` must be a valid dbref and `message` must be a string.

## `mux.connected_players()`

Returns an array of player connections visible to the normal `who` command.
Each entry has `object`, `name`, `connected_for`, and `idle_for` fields.
`object` is the player object handle; the duration fields are elapsed seconds.

```lua
for _, player in ipairs(mux.connected_players()) do
  mux.notify(ctx.enactor, player.name)
end
```

The result does not expose hidden players or privileged connection details.

## `mux.who_summary()`

Returns the non-privileged WHO summary table with `hidden`, `record`, and
`maximum` fields. `maximum` is `nil` when the game has no player limit.

```lua
local summary = mux.who_summary()
local maximum = summary.maximum or "no"
```

## Telnet environment

`mux.telnet_environment_has(descriptor, kind, name)` reports whether an RFC
1572 NEW-ENVIRON variable is defined on a live connection.
`mux.telnet_environment_get(descriptor, kind, name)` returns its string value,
or `nil` when it is absent. A defined empty value returns `""`.

`descriptor` is normally `ctx.descriptor`. `kind` must be `"var"` or
`"uservar"`; the two namespaces are distinct. Names and values are binary-safe
Lua strings, and all received data is untrusted client input.

```lua
if mux.telnet_environment_has(ctx.descriptor, "var", "USER") then
  local user = mux.telnet_environment_get(ctx.descriptor, "var", "USER")
end
```

Both functions raise an error for an invalid descriptor or kind and are
unavailable during `@lua/check`.

## `mux.flow_start(descriptor, module, first_step)`

Attaches an [interactive flow](../flows/) to a descriptor and shows its
first prompt.

```lua
mux.flow_start(ctx.descriptor, "confirm_delete.lua", "confirm")
```

`descriptor` is normally `ctx.descriptor` from the calling command or event.
Raises a Lua error if the descriptor doesn't exist, already has a flow
running, or `module` has no `first_step` in its `flows` table.

## Availability and limits

The `mux` table is the only server interface exposed to Lua modules. Runtime
database functions, including object enumeration and identity, are unavailable
during `@lua/check`. The Lua
sandbox does not expose filesystem, operating-system, debugger, FFI, coroutine,
or dynamic code-loading APIs. VM memory and persistent-state limits still apply
while using these functions.
