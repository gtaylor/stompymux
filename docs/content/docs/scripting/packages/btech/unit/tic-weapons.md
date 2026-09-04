---
title: tic_weapons
type: docs
toc_hide: false
---

Lists the weapons assigned to a unit's target-interlock circuit.

## Function

### Synopsis

```lua
btech.unit.tic_weapons( unit, tic )
```

### Arguments

`number unit`
: The unit dbref.

`integer tic`
: The zero-based TIC number.

### Returns

`table values`
: A flat array of converted legacy result tokens.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../../)
