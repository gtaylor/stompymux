---
title: "Channel:who"
type: docs
---

Returns channel membership records. By default the native active-member
filter is applied; `options.all` includes inactive records.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.channel.invalid`, or `mux.error.codes.arg.invalid`.

## Signature

```lua
Channel:who(options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `options?` | `ChannelWhoOptions` | Unknown option fields are rejected. |

## Returns

- `ChannelMember[] members`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
- `mux.error.codes.arg.invalid`
