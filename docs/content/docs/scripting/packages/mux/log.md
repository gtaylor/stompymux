---
title: mux.log
type: docs
toc_hide: false
---

Appends a message to a named server log file.

## Function

### Synopsis

```lua
mux.log( filename, message )
```

### Arguments

`string filename`
: The name of an existing readable and writable file directly under
  `game/logs/`. Names may not contain `/`, `..`, embedded NUL bytes, or exceed
  200 bytes.

`string message`
: Text to append, followed by a newline. Embedded NUL bytes are rejected.

### Returns

`true` when the message was written or was empty; otherwise `false`.

## Examples

```lua
if not mux.log("combat.log", "A unit was destroyed") then
  error("unable to write combat log")
end
```

## Notes

This function uses the same validation and five-minute cached file handles as
the Wizard `@log` command. It is unavailable during `@lua/check`. Lua modules
run as trusted server logic, so restrict filenames and logged data deliberately.

## See Also

- [`mux`](../)
