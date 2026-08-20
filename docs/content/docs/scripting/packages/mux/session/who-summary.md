---
title: who_summary
type: docs
toc_hide: false
---

Returns the non-privileged WHO summary.

## Function

### Synopsis

```lua
mux.session.who_summary( )
```

### Arguments

None.

### Returns

`table summary`
: A table with `hidden`, `record`, and `maximum` fields.

## Examples

```lua
local summary = mux.session.who_summary()
local maximum = summary.maximum or "no"
```

## Notes

`hidden` and `record` are numbers. `maximum` is a number or `nil` when the game has no player limit.

## See Also

- [`mux.session`](../)
- [`mux.session.connected_players`](../connected-players/)
