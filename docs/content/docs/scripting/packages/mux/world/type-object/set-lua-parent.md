---
title: "Object:set_lua_parent"
type: docs
linkTitle: "set_lua_parent"
manualLinkTitle: "set_lua_parent"
---

Assigns this object's direct Lua parent path, or clears it when `parent` is nil.

Raises `mux.error.codes.unavailable.checking`, `mux.error.codes.arg.invalid` when `parent` is omitted or malformed, `mux.error.codes.module.invalid` for an invalid or unavailable path, `mux.error.codes.object.invalid`, or `mux.error.codes.object.unavailable`.

## Signature

```lua
Object:set_lua_parent(parent)
```

## Parameters

| Name | Type | Description |
| --- | --- | --- |
| `parent` | `string\|nil` | Existing `object_logic`-relative `.lua` path, or nil to clear it. This argument must be supplied explicitly. |

## Returns

No values.

## Related errors

- `mux.error.codes.unavailable.checking`
- `mux.error.codes.arg.invalid`
- `mux.error.codes.module.invalid`
- `mux.error.codes.object.invalid`
- `mux.error.codes.object.unavailable`
