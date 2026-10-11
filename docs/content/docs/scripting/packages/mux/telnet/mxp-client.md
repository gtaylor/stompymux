---
title: "mux.telnet.mxp_client"
type: docs
linkTitle: "mxp_client"
manualLinkTitle: "mxp_client"
---

Describes the MXP client on a live descriptor.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.connection.invalid`.

## Signature

```lua
mux.telnet.mxp_client(descriptor)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `descriptor` | `integer` | Live descriptor ID, normally `ctx.descriptor`. |

## Returns

- `TelnetMxpClient? client Reported client details, or nil when MXP is not negotiated.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.connection.invalid`
