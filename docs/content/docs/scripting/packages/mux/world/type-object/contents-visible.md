---
title: contents_visible
type: docs
toc_hide: false
---

Tests whether a directly contained object is visible to a viewer.

## Function

### Synopsis

```lua
object:contents_visible( viewer, member )
```

### Arguments

`number or Object viewer`
: The object viewing the container.

`number or Object member`
: An object directly contained by the receiver.

### Returns

`boolean visible`
: Whether native look rules expose the member.

## Examples

```lua
for _, member in ipairs(room:contents()) do
  if room:contents_visible(ctx.enactor, member) then
    mux.world.pemit(ctx.enactor, member:name())
  end
end
```

## Notes

The receiver must be able to contain objects, and `member` must be directly contained by it. This method is unavailable during `@lua/check`.

## See Also

- [`mux`](../../../)
- [`Object`](../)
- [`Object:contents`](../contents/)
