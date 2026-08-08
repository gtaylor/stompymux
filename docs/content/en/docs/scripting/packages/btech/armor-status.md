---
title: btech.armor_status
linkTitle: btech.armor_status
type: docs
weight: 201
---

# `btech.armor_status`

Returns serialized armor values for one section of a live unit.

## Function

### Synopsis

```lua
btech.armor_status( unit, section )
```

### Arguments

`number unit`
: The unit dbref.

`string section`
: The section name.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
- [`btech.armor_status_ref`](../armor-status-ref/)
