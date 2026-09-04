---
draft: true
title: stores
type: docs
toc_hide: false
---

Returns a part quantity or lists an object's stored parts.

## Function

### Synopsis

```lua
btech.parts.stores( target, part_name )
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

Lists every nonempty part stack on the object.

### Synopsis

```lua
btech.parts.stores( target )
```

### Arguments

`number target`
: The stores-bearing object dbref.

### Returns

`table stores`
: A flat array of legacy serialization tokens. A long display name containing
  spaces is split across multiple items, so an item is not guaranteed to hold
  one complete `name:quantity` record.

## Examples

```lua
local btech = require("btech")
local quantity = btech.parts.stores(bay_dbref, "IS.AC/10")[1]

mux.world.pemit(ctx.enactor, "Available: " .. quantity)
```

## Notes

This function is available only in a running Lua callback. Invalid targets,
invalid arguments, and legacy error results raise a Lua error. The record
splitting is a limitation of the shared legacy list adapter, which treats spaces
and `|` as separators.

## See Also

- [`btech`](../../)
- [`btech.parts.stores_short`](../stores-short/)
- [`btech.parts.add_stores`](../add-stores/)
- [`btech.parts.remove_stores`](../remove-stores/)
