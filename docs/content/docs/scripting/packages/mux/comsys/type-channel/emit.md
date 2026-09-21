---
title: "Channel:emit"
type: docs
---

Emits an administrative channel message through native delivery, history,
receive-lock, and message-count behavior.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.channel.invalid`, or `mux.error.codes.arg.invalid`.

## Signature

```lua
Channel:emit(message, options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `message` | `string` | Valid UTF-8 without embedded NUL bytes. |
| `options?` | `ChannelEmitOptions` | Unknown option fields are rejected. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.channel.invalid`
- `mux.error.codes.arg.invalid`
