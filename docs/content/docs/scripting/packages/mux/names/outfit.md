---
title: "mux.names.outfit"
type: docs
linkTitle: "outfit"
manualLinkTitle: "outfit"
---

Generates a mercenary outfit name such as `35th Harrow Heavy Horse`.

Raises `mux.error.codes.arg.invalid` for a
malformed spec.

## Signature

```lua
mux.names.outfit(spec)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `spec?` | `NameSpec` |  |

## Returns

- `OutfitName outfit`

## Related errors

- `mux.error.codes.arg.invalid`
