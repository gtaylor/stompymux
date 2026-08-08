---
title: mux.who_summary
linkTitle: mux.who_summary
type: docs
weight: 209
---

# `mux.who_summary`

Returns the non-privileged WHO summary.

## Function

### Synopsis

```lua
mux.who_summary( )
```

### Arguments

None.

### Returns

`table summary`
: A table with `hidden`, `record`, and `maximum` fields.

## Examples

```lua
local summary = mux.who_summary()
local maximum = summary.maximum or "no"
```

## Notes

`hidden` and `record` are numbers. `maximum` is a number or `nil` when the game has no player limit.

## See Also

- [`mux`](../)
- [`mux.connected_players`](../connected-players/)
