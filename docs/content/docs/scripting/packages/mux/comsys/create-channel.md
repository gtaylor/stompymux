---
title: "mux.comsys.create_channel"
type: docs
linkTitle: "create_channel"
manualLinkTitle: "create_channel"
---

Creates a private communication channel using the native channel-name
rules. Names must be non-empty printable ASCII, contain no spaces, and be
shorter than 50 bytes.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid`, or `mux.error.codes.channel.invalid` when the name already exists.

## Signature

```lua
mux.comsys.create_channel(name)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `name` | `string` | New channel name. |

## Returns

- `Channel channel`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.channel.invalid`
