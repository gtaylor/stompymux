---
title: "mux.session.flow_start"
type: docs
linkTitle: "flow_start"
manualLinkTitle: "flow_start"
---

Attaches an interactive flow to a descriptor and displays its first prompt.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.connection.invalid`, `mux.error.codes.connection.unavailable`, or `mux.error.codes.module.invalid`.

## Signature

```lua
mux.session.flow_start(descriptor, module, first_step)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `descriptor` | `integer` |  |
| `module` | `string` | Require-style module path. |
| `first_step` | `string` | Key in the module's `flows` table. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.connection.invalid`
- `mux.error.codes.connection.unavailable`
- `mux.error.codes.module.invalid`
