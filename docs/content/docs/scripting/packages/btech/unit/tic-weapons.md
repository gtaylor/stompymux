---
title: tic_weapons
type: docs
toc_hide: false
---

Lists the weapons assigned to a target-interlock circuit.

## Function

### Synopsis

```lua
btech.unit.tic_weapons( unit, tic )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`integer tic`
: The zero-based circuit number.

### Returns

`BtechMountedWeapon[] weapons`
: The assigned mounted weapons.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
