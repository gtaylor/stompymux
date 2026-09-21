---
title: "Channel:add_player"
type: docs
---

Adds a player to this channel with a player-local command alias. The trusted
operation bypasses the channel join lock. A quiet join suppresses only the
channel-wide announcement; direct confirmations are still sent to the player.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.channel.invalid`, `mux.error.codes.object.invalid`, `mux.error.codes.object.unavailable`, `mux.error.codes.arg.invalid`, or `mux.error.codes.internal`.

## Signature

```lua
Channel:add_player(player, alias, quiet)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `player` | `DbRef\|Object` | Player to add. |
| `alias` | `string` | One to five printable ASCII characters without spaces. |
| `quiet` | `boolean` | Whether to suppress the channel-wide join announcement. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.internal`
