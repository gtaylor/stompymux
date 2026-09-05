---
title: set_mechwarrior_template
type: docs
toc_hide: false
---

Sets or clears a player's ejected-pilot template override.

## Function

### Synopsis

```lua
btech.player.set_mechwarrior_template( player, reference )
```

### Arguments

`DbRef|Object player`
: The player.

`string|nil reference`
: A non-empty resource name of at most 24 bytes that identifies an existing,
  well-formed unit template, or `nil` to clear the override.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.player`](../)
