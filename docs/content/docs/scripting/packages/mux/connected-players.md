---
title: mux.connected_players
type: docs
toc_hide: false
---

Lists player connections visible to the normal `who` command.

## Function

### Synopsis

```lua
mux.connected_players( )
```

### Arguments

None.

### Returns

`table players`
: An array of connection records.

## Examples

```lua
for _, player in ipairs(mux.connected_players()) do
  mux.notify(ctx.enactor, player.name)
end
```

## Notes

Each record contains `object` (`Object`), `name` (string), `connected_for` (elapsed seconds), and `idle_for` (elapsed seconds). Hidden players and privileged connection details are omitted.

## See Also

- [`mux`](../)
- [`mux.who_summary`](../who-summary/)
- [`Object`](../type-object/)
