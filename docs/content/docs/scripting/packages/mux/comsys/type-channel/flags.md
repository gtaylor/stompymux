---
title: "Channel:flags"
type: docs
linkTitle: "flags"
manualLinkTitle: "flags"
---

Opens the live administrative flag collection for this channel.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.channel.invalid`.

## Signature

```lua
Channel:flags()
```

## Parameters

None.

## Returns

- `ChannelFlags flags`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
