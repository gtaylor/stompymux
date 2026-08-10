---
title: btech.stores
type: docs
toc_hide: true
---

Returns a part quantity or lists an object's stored parts.

## Function

### Synopsis

```lua
btech.stores( target, part_name )
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
btech.stores( target )
```

### Arguments

`number target`
: The stores-bearing object dbref.

### Returns

`table stores`
: A flat array of serialized `name:quantity` strings.

## Examples

```lua
local btech = require("btech")
local quantity = btech.stores(bay_dbref, "IS.AC/10")[1]

mux.notify(ctx.enactor, "Available: " .. quantity)
```

## Notes

This function is available only in a running Lua callback. Invalid targets, invalid arguments, and legacy error results raise a Lua error.

## See Also

- [`btech`](../)
- [`btech.stores_short`](../stores-short/)
- [`btech.add_stores`](../add-stores/)
- [`btech.remove_stores`](../remove-stores/)
