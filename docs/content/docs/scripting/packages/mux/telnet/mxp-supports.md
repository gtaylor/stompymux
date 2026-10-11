---
title: "mux.telnet.mxp_supports"
type: docs
linkTitle: "mxp_supports"
manualLinkTitle: "mxp_supports"
---

Tests whether a live descriptor's MXP client advertised a tag, such as `"image"`.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.connection.invalid`.

## Signature

```lua
mux.telnet.mxp_supports(descriptor, tag)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `descriptor` | `integer` | Live descriptor ID, normally `ctx.descriptor`. |
| `tag` | `string` | Case-insensitive MXP tag or `tag.attribute` name. |

## Returns

- `boolean supported False when MXP is not negotiated or the tag was not advertised.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.connection.invalid`
