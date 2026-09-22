---
title: "mux.world.pemit"
type: docs
linkTitle: "pemit"
manualLinkTitle: "pemit"
---

Sends valid UTF-8 text to an object.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.connection.invalid`, `mux.error.codes.object.invalid`.

## Signature

```lua
mux.world.pemit(object, message)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `object` | `DbRef\|Object` |  |
| `message` | `string` |  |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.connection.invalid`
- `mux.error.codes.object.invalid`
