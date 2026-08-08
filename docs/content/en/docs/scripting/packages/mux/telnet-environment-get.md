---
title: mux.telnet_environment_get
linkTitle: mux.telnet_environment_get
type: docs
weight: 211
---

# `mux.telnet_environment_get`

Gets an RFC 1572 NEW-ENVIRON variable from a live connection.

## Function

### Synopsis

```lua
mux.telnet_environment_get( descriptor, kind, name )
```

### Arguments

`number descriptor`
: A live descriptor ID, normally `ctx.descriptor`.

`string kind`
: Either `"var"` or `"uservar"`.

`string name`
: The binary-safe variable name.

### Returns

`string or nil value`
: The binary-safe value, or `nil` when the variable is absent.

## Examples

```lua
if mux.telnet_environment_has(ctx.descriptor, "var", "USER") then
  local user = mux.telnet_environment_get(ctx.descriptor, "var", "USER")
end
```

## Notes

A defined empty value returns `""`. The two kinds are distinct namespaces and all received data is untrusted. An invalid descriptor or kind raises an error; this function is unavailable during `@lua/check`.

## See Also

- [`mux`](../)
- [`mux.telnet_environment_has`](../telnet-environment-has/)
