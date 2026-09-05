---
title: apply_damage
type: docs
toc_hide: false
---

Applies clustered damage to a live unit.

## Function

### Synopsis

```lua
btech.unit.apply_damage( unit, request )
```

### Arguments

`DbRef|Object unit`
: The live unit.

`table request`
: Damage fields: `amount`, `cluster_size`, `direction_code`, and optional messages and critical control.

### Returns

None.

## See Also

- [`btech`](../../)
- [`btech.unit`](../)
