---
draft: true
title: fasa_base_cost_ref
type: docs
toc_hide: false
---

Calculates the FASA base cost of a unit template.

## Function

### Synopsis

```lua
btech.unit.fasa_base_cost_ref( reference )
```

### Arguments

`string reference`
: The unit template reference.

### Returns

`number value`
: The numeric result.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error. The reference is resolved from the configured unit-template database. This operation requires a build with advanced economy support.

## See Also

- [`btech`](../../)
