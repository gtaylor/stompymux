---
draft: true
title: add_stores
type: docs
toc_hide: false
---

Adds a quantity of a part to an object's stores.

## Function

### Synopsis

```lua
btech.parts.add_stores( target, part_name, quantity )
```

### Arguments

`number target`
: The dbref of the stores-bearing object.

`string part_name`
: A recognized part name.

`integer quantity`
: The quantity to add, capped by the server limit.

### Returns

`true success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets,
invalid arguments, and legacy error results raise a Lua error. The underlying
quantity is capped at the server's per-call maximum. A part name that does not
match returns the legacy false result, but mutation-result normalization still
returns `true`; callers should resolve user input with [`btech.parts.match`](../match/)
before mutating stores.

## See Also

- [`btech`](../../)
