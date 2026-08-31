---
title: check_db
type: docs
toc_hide: false
---

Checks the game database for inconsistencies and repairs any damage found.

## Function

### Synopsis

```lua
mux.check_db()
```

### Arguments

None.

### Returns

Nothing.

## Examples

```lua
mux.check_db()
```

## Notes

This function performs the same default consistency check as `@dbck`. It does
not send the command's completion message to a player. Any database damage
found is still written to the server log.

The function is unavailable during `@lua/check` and raises
`mux.unavailable.checking` if called there.

## See Also

- [`mux`](../)
