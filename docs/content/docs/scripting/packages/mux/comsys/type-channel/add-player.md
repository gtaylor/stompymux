---
title: add_player
type: docs
toc_hide: false
---

Adds a player to this channel and registers a player-local command alias.

## Function

### Synopsis

```lua
channel:add_player( player, alias, quiet )
```

### Arguments

`number or Object player`
: The player to add.

`string alias`
: A player-local alias containing 1–5 printable ASCII characters without
  spaces.

`boolean quiet`
: When `true`, suppresses the channel-wide join announcement.

### Returns

Nothing.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the channel is stale.
- `mux.object.invalid` if the object reference is invalid or is not a player.
- `mux.object.unavailable` if the player is being destroyed.
- `mux.arg.invalid` if the alias is invalid or already in use, or if `quiet`
  is not a boolean.
- `mux.internal` if the alias collection cannot be expanded.

## Notes

This trusted administrative operation bypasses the channel join lock. The
alias is added to the player's normal channel aliases and can be used for
channel commands. A quiet join still sends the player the normal direct join
and alias confirmations.

Adding another alias for an existing active member does not add another
membership record or announce another join.

## See Also

- [`mux`](../../../)
- [`Channel`](../)
- [`Channel:boot_player`](../boot-player/)
- [`Channel:who`](../who/)
