---
title: mux.comsys
type: docs
weight: 14
sidebar_root_for: self
---

`mux.comsys` is the trusted communication-channel API. It operates directly on
the live channel registry as God; changes take effect immediately and are not
rolled back if the Lua callback later fails. Existing `@chan/*` commands retain
their normal Wizard checks and player-facing output.

All functions are unavailable during `@lua/check`. Unknown or stale channels
raise `mux.channel.invalid`. A Channel handle stays stale after destruction even
if another channel is created with the same name.

## Functions

### `channel`

```lua
local public = mux.comsys.channel("Public")
```

Returns the existing [Channel](type-channel/) using a case-insensitive lookup.
`Channel:name()` returns its canonical spelling. Unknown names raise
`mux.channel.invalid`.

### `create_channel`

```lua
local staff = mux.comsys.create_channel("Staff")
```

Creates and returns a private channel. Names must be non-empty printable ASCII,
contain no spaces, and be shorter than 50 bytes. Invalid names raise
`mux.arg.invalid`; duplicate names raise `mux.channel.invalid`.

### `destroy_channel`

```lua
mux.comsys.destroy_channel(staff)
```

Permanently removes a live Channel and its membership storage. The handle and
all flag handles derived from it become stale.

### `list_channels`

```lua
for _, channel in ipairs(mux.comsys.list_channels()) do
  mux.log("channels.log", channel:name())
end
```

Returns every channel as a dense array sorted case-insensitively by name, with
the original spelling as the tie-breaker.

## Types and constants

- [Channel](type-channel/) exposes channel properties and operations as methods.
- [ChannelFlags](type-channel-flags/) reads and changes the administrative flags.
- [`mux.comsys.flags`](flags/) provides typed channel-flag constants.
