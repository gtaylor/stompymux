---
title: show_crit_status_ref
type: docs
toc_hide: false
---

Sends a template's critical-status display to a player.

## Function

### Synopsis

```lua
btech.unit.show_crit_status_ref( reference, player, section )
```

### Arguments

`string reference`
: The unit template reference.

`number player`
: The recipient player dbref.

`string section`
: The section passed to the critical-status renderer.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database.

## See Also

- [`btech`](../../)
