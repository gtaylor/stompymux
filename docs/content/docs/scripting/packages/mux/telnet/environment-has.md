---
title: "mux.telnet.environment_has"
type: docs
---

Tests whether a binary-safe RFC 1572 NEW-ENVIRON variable is defined.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.connection.invalid`.

## Signature

```lua
mux.telnet.environment_has(descriptor, kind, name)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `descriptor` | `integer` | Live descriptor ID, normally `ctx.descriptor`. |
| `kind` | `TelnetEnvironmentKind` | NEW-ENVIRON variable namespace. |
| `name` | `string` | Binary-safe variable name. |

## Returns

- `boolean defined Whether the variable is present, including with an empty value.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.connection.invalid`
