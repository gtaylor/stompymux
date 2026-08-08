---
title: btech.battle_value
linkTitle: btech.battle_value
type: docs
weight: 215
---

# `btech.battle_value`

Calculates the battle value of a live unit.

## Function

### Synopsis

```lua
btech.battle_value( unit )
```

### Arguments

`number unit`
: The unit dbref.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
- [`btech.battle_value_ref`](../battle-value-ref/)
