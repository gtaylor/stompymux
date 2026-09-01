---
title: ChannelFlags
type: docs
sidebar_root_for: self
---

Create a `ChannelFlags` handle with `Channel:flags()`. Methods accept only typed
constants from [`mux.comsys.flags`](../flags/).

| Method | Behavior |
| --- | --- |
| `list()` | Returns set constants in `PUBLIC`, `LOUD`, `TRANSPARENT` order. |
| `has(flag)` | Tests whether a flag is set. |
| `add(flag)` | Sets a flag and returns whether the channel changed. |
| `remove(flag)` | Clears a flag and returns whether the channel changed. |

Flag changes run immediately and are not part of the Lua State transaction.
The collection becomes stale with its originating Channel and then raises
`mux.channel.invalid`.
