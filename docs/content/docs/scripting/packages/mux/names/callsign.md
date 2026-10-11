---
title: "mux.names.callsign"
type: docs
linkTitle: "callsign"
manualLinkTitle: "callsign"
---

Generates one pilot callsign such as `Switchback`.

Raises `mux.error.codes.arg.invalid` for a
malformed spec.

## Signature

```lua
mux.names.callsign(spec)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `spec?` | `NameSpec` |  |

## Returns

- `string callsign`

## Related errors

- `mux.error.codes.arg.invalid`
