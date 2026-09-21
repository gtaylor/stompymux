---
title: "mux.log"
type: docs
---

Appends a newline-terminated message to a permitted file under `game/logs`.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid`.

## Signature

```lua
mux.log(filename, message)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `filename` | `string` |  |
| `message` | `string` |  |

## Returns

- `boolean written`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
