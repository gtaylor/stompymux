---
title: "Channel:message_count"
type: docs
linkTitle: "message_count"
manualLinkTitle: "message_count"
---

Returns the channel's lifetime delivered-message count.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.channel.invalid`.

## Signature

```lua
Channel:message_count()
```

## Parameters

None.

## Returns

- `integer count`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
