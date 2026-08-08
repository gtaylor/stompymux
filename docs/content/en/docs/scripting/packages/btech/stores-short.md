---
title: btech.stores_short
linkTitle: btech.stores_short
type: docs
weight: 262
---

# `btech.stores_short`

Returns a part quantity or lists stored parts using short names.

## Function

### Synopsis

```lua
btech.stores_short( target, part_name )
```

### Arguments

`number target`
: The stores-bearing object dbref.

`string part_name`
: A recognized part name.

### Returns

`table result`
: A one-element array containing the numeric quantity.

## Function

Lists every nonempty part stack using short part names.

### Synopsis

```lua
btech.stores_short( target )
```

### Arguments

`number target`
: The stores-bearing object dbref.

### Returns

`table stores`
: A flat array of serialized `name:quantity` strings using short part names.

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
- [`btech.stores`](../stores/)
