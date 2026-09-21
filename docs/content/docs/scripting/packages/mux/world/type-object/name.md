---
title: "Object:name"
type: docs
---

Returns this object's current stored name.

Raises `mux.error.codes.object.invalid`.

## Signature

```lua
Object:name()
```

## Parameters

None.

## Returns

- `string name`

## Related errors

- `mux.error.codes.object.invalid`

## Example

```lua
-- In a command callback, ctx.object identifies the object running the command.
local object = mux.world.object(ctx.object)
local name = object:name()
mux.world.pemit(ctx.enactor, "This object is named " .. name)
```
