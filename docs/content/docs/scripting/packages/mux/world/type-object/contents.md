---
title: "Object:contents"
type: docs
linkTitle: "contents"
manualLinkTitle: "contents"
---

Returns matching objects directly contained by or attached to this object.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid`, or `mux.error.codes.object.invalid`.

## Signature

```lua
Object:contents(options)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `options?` | `ContentsOptions` | Optional type and visibility filters. |

## Returns

- `Object[] contents`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.object.invalid`
