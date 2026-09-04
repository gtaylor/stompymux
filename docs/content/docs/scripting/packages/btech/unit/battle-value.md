---
draft: true
title: battle_value
type: docs
toc_hide: false
---

Calculates the current battle value of a live unit.

## Function

### Synopsis

```lua
btech.unit.battle_value(unit)
```

### Arguments

`DbRef|Object unit`
: The live unit.

### Returns

`BtechBattleValue value`
: A record containing numeric `total`, `offensive`, and `defensive` fields.

## Notes

The value is calculated from the unit's current armor, internals, movement,
tonnage, weapons, heat efficiency, and operational equipment. Invalid targets
and arguments raise a Lua error.

## See Also

- [`btech`](../../)
- [`btech.template`](../../template/)
