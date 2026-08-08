---
title: btech.add_stores
linkTitle: btech.add_stores
type: docs
weight: 200
---

# `btech.add_stores`

Adds a quantity of a part to an object's stores.

## Function

### Synopsis

```lua
btech.add_stores( target, part_name, quantity )
```

### Arguments

`number target`
: The dbref of the stores-bearing object.

`string part_name`
: A recognized part name.

`number quantity`
: The quantity to add, capped by the server limit.

### Returns

`boolean success`
: `true` after the operation completes without a legacy error.

## Notes

This function is available only in a running Lua callback. Invalid targets,
invalid arguments, and legacy error results raise a Lua error. The underlying
quantity is capped at the server's per-call maximum. A part name that does not
match returns the legacy false result, but mutation-result normalization still
returns `true`; callers should resolve user input with [`btech.part_match`](../part-match/)
before mutating stores.

## See Also

- [`btech`](../)
