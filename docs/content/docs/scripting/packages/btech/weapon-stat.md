---
title: weapon_stat
type: docs
toc_hide: false
---

Returns one numeric weapon-catalog statistic as text.

## Function

### Synopsis

```lua
btech.weapon_stat( weapon, stat )
```

### Arguments

`string weapon`
: A recognized weapon part name.

`string stat`
: One of `VRT`, `TYPE`, `HEAT`, `DAMAGE`, `MIN`, `SR`, `MR`, `LR`, `CRIT`, `AMMO`, `WEIGHT`, or `BV`.

### Returns

`string result`
: The handler's serialized text result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. Although the selected statistic is numeric, this binding is registered as a text result.

## See Also

- [`btech`](../)
