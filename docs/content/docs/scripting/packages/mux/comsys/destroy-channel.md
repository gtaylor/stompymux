---
title: "mux.comsys.destroy_channel"
type: docs
---

Permanently removes a live channel and its membership storage. The supplied
handle and every flag handle derived from it become stale.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.channel.invalid`.

## Signature

```lua
mux.comsys.destroy_channel(channel)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `channel` | `Channel` | Live channel handle. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
