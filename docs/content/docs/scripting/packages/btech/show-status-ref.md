---
title: show_status_ref
type: docs
toc_hide: false
---

Sends a unit template's status display to a player.

## Function

### Synopsis

```lua
btech.show_status_ref( reference, player )
```

### Arguments

`string reference`
: The unit template reference.

`number player`
: The recipient player dbref.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database.

## See Also

- [`btech`](../)
