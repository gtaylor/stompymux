---
title: btech.battle_value_ref
linkTitle: btech.battle_value_ref
type: docs
weight: 216
---

# `btech.battle_value_ref`

Calculates the battle value of a unit template.

## Function

### Synopsis

```lua
btech.battle_value_ref( reference )
```

### Arguments

`string reference`
: The unit template reference.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database.

## See Also

- [`btech`](../)
- [`btech.battle_value`](../battle-value/)
