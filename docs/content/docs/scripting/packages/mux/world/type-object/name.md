---
title: "Object:name"
type: docs
linkTitle: "name"
manualLinkTitle: "name"
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
local function show_name(ctx)
  local object = mux.world.object(ctx.object)
  local name = object:name()
  mux.world.pemit(ctx.enactor, "This object is named " .. name)
end
```
