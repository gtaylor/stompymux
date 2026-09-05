---
title: set_link
type: docs
toc_hide: false
---

Sets or clears a child map's parent-link configuration.

## Function

### Synopsis

```lua
btech.map.set_link( child, link )
```

### Arguments

`DbRef|Object child`
: The child BattleTech map.

`BtechMapLink|nil link`
: The new parent link, or `nil` to clear it.

A non-`nil` link requires these fields:

`DbRef|Object parent`
: The parent BattleTech map. A map cannot be its own parent.

`integer x`, `integer y`
: A valid destination hex on the parent map.

The link may contain an `entrances` table with `north`, `east`, `south`, and
`west` fields. Each directional field is optional and uses one of these forms:

```lua
{ mode = "offset", offset = non_negative_integer }
{ mode = "exact", x = child_x, y = child_y }
```

Exact entrance coordinates must identify a valid hex on the child map.

### Returns

None.

## Notes

This function stores the link configuration but does not rebuild live link
objects. After setting a link, call
[`btech.map.update_links`](../update-links/) on its parent map to update the live
`BUILD`, `LEAVE`, and `ENTRANCE` objects.

When clearing a link, rebuilding the former parent removes its `BUILD` object,
but the current rebuild logic does not remove the former child's existing
`LEAVE` and `ENTRANCE` objects. Calling `update_links` on the now-unlinked child
does not remove those stale child-side objects either.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
- [`btech.map.update_links`](../update-links/)
