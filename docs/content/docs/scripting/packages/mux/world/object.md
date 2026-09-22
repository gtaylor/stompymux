---
title: "mux.world.object"
type: docs
linkTitle: "object"
manualLinkTitle: "object"
---

Creates a validated object handle from a dbref or existing handle.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.object.invalid`.

## Signature

```lua
mux.world.object(dbref)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `dbref` | `DbRef\|Object` |  |

## Returns

- `Object object`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.object.invalid`
