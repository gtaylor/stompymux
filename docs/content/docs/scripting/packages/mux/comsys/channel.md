---
title: "mux.comsys.channel"
type: docs
---

Retrieves an existing communication channel by case-insensitive name.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid`, or `mux.error.codes.channel.invalid`.

## Signature

```lua
mux.comsys.channel(name)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `name` | `string` | Existing channel name without embedded NUL bytes; the returned handle preserves canonical spelling. |

## Returns

- `Channel channel`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.channel.invalid`
