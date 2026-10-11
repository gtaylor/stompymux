---
title: "mux.names.roster"
type: docs
linkTitle: "roster"
manualLinkTitle: "roster"
---

Generates `count` people in one culture with distinct family names where the lists
allow it.

Raises `mux.error.codes.arg.invalid` for a
malformed spec or count.

## Signature

```lua
mux.names.roster(spec, count)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `spec?` | `NameSpec` |  |
| `count` | `integer` | From 1 to 500. |

## Returns

- `PersonName[] roster`

## Related errors

- `mux.error.codes.arg.invalid`
