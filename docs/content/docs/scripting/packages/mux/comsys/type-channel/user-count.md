---
title: "Channel:user_count"
type: docs
linkTitle: "user_count"
manualLinkTitle: "user_count"
---

Returns the number of channel membership records.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.channel.invalid`.

## Signature

```lua
Channel:user_count()
```

## Parameters

None.

## Returns

- `integer count`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
