---
title: "mux.telnet.environment_get"
type: docs
---

Gets a binary-safe RFC 1572 NEW-ENVIRON value.

Raises `mux.error.codes.unavailable.checking` or `mux.error.codes.connection.invalid`.

## Signature

```lua
mux.telnet.environment_get(descriptor, kind, name)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `descriptor` | `integer` | Live descriptor ID, normally `ctx.descriptor`. |
| `kind` | `TelnetEnvironmentKind` | NEW-ENVIRON variable namespace. |
| `name` | `string` | Binary-safe variable name. |

## Returns

- `string? value Binary-safe value, or nil when the variable is absent.`

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.connection.invalid`
