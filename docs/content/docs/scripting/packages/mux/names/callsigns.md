---
title: "mux.names.callsigns"
type: docs
linkTitle: "callsigns"
manualLinkTitle: "callsigns"
---

Generates `count` distinct pilot callsigns where the list allows it.

Raises `mux.error.codes.arg.invalid` for a
malformed spec or count.

## Signature

```lua
mux.names.callsigns(spec, count)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `spec?` | `NameSpec` |  |
| `count` | `integer` | From 1 to 500. |

## Returns

- `string[] callsigns`

## Related errors

- `mux.error.codes.arg.invalid`
