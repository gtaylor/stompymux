---
title: mux.telnet_environment_has
linkTitle: mux.telnet_environment_has
type: docs
weight: 210
---

# `mux.telnet_environment_has`

Tests whether an RFC 1572 NEW-ENVIRON variable is defined on a live connection.

## Function

### Synopsis

```lua
mux.telnet_environment_has( descriptor, kind, name )
```

### Arguments

`number descriptor`
: A live descriptor ID, normally `ctx.descriptor`.

`string kind`
: Either `"var"` or `"uservar"`.

`string name`
: The binary-safe variable name.

### Returns

`boolean defined`
: Whether the variable is present, including with an empty value.

## Notes

The two kinds are distinct namespaces. Client-provided names and values are untrusted input. An invalid descriptor or kind raises an error; this function is unavailable during `@lua/check`.

## See Also

- [`mux`](../)
- [`mux.telnet_environment_get`](../telnet-environment-get/)
