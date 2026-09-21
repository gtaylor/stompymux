---
title: "Channel:max_user_count"
type: docs
---

Returns the channel's currently allocated membership capacity.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.channel.invalid`.

## Signature

```lua
Channel:max_user_count()
```

## Parameters

None.

## Returns

- `integer count`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
