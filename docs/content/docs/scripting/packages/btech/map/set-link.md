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

### Returns

None.

## Notes

This function stores the link configuration but does not rebuild live link
objects. After changing a link, call [`btech.map.update_links`](../update-links/)
on its parent map to update the live `BUILD`, `LEAVE`, and `ENTRANCE` objects.

## See Also

- [`btech`](../../)
- [`btech.map`](../)
- [`btech.map.update_links`](../update-links/)
