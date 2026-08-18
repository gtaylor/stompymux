---
title: mux.error.codes
type: docs
toc_hide: false
---

Provides checked symbols for native `mux.*` error codes. `btech.*` symbols are
in [`btech.error.codes`](../../btech-error/codes/), and `testing.*` symbols are
available as `testing.error.codes` in test suites.

## Constants

| Symbol | String value | When raised |
| --- | --- | --- |
| `mux.error.codes.arg.invalid` | `mux.arg.invalid` | An API argument is invalid, including invalid error-code lookups and namespaces. |
| `mux.error.codes.unavailable.checking` | `mux.unavailable.checking` | A runtime-only `mux` operation is used during `@lua/check`. |
| `mux.error.codes.runtime` | `mux.runtime` | `mux.error.pcall` normalizes a non-error failure, or runtime error logging has no structured code. |
| `mux.error.codes.state.invalid` | `mux.state.invalid` | A state namespace, key, value, or batch update is invalid. |
| `mux.error.codes.state.value_too_large` | `mux.state.value_too_large` | A state write exceeds a configured per-value, entry, or object limit. |
| `mux.error.codes.state.unavailable` | `mux.state.unavailable` | State enumeration occurs outside an active callback or state changes during enumeration. |
| `mux.error.codes.object.invalid` | `mux.object.invalid` | An object handle or database object is invalid, stale, or unsuitable for the requested operation. |
| `mux.error.codes.object.unavailable` | `mux.object.unavailable` | `Object:enter_lock_passes` is unavailable in this runtime. |
| `mux.error.codes.attribute.invalid` | `mux.attribute.invalid` | An attribute name or value is invalid, not administrable, or cannot be written. |
| `mux.error.codes.connection.invalid` | `mux.connection.invalid` | A message, descriptor, Telnet-environment kind, or flow descriptor is invalid. |
| `mux.error.codes.connection.unavailable` | `mux.connection.unavailable` | Flow support is unavailable or the descriptor already has an active flow. |
| `mux.error.codes.text.invalid` | `mux.text.invalid` | Styled text, a style field, or a requested text width is invalid. |
| `mux.error.codes.module.invalid` | `mux.module.invalid` | A module name is invalid or a requested flow step is absent from its module. |
| `mux.error.codes.module.unavailable` | `mux.module.unavailable` | A required module cannot be loaded or is unavailable from the current module root. |
| `mux.error.codes.internal` | `mux.internal` | An out-of-range native error-code enum is converted through the fallback `lua_error_code_name`; there is no direct raise site. |

## Examples

```lua
if value_size > limit then
  mux.error.raise(
    mux.error.codes.state.value_too_large,
    "state value exceeds the configured limit"
  )
end

if err:is(mux.error.codes.state) then
  -- Handles every mux.state.* failure.
end
```

## Notes

Each leaf and intermediate node has a `.code` field, and `tostring(node)`
returns that dotted string. The intermediate nodes are derived from the leaves,
not native enum members: `mux`, `mux.arg`, `mux.unavailable`, `mux.state`,
`mux.object`, `mux.attribute`, `mux.connection`, `mux.text`, and `mux.module`.
They are useful with [`mux.error.is`](../is/) for a whole-namespace check; for
example, `mux.error.codes.state` matches every
`mux.state.*` code.

Unknown node keys raise when looked up instead of returning `nil`, which makes
a misspelled symbol fail at the point it is written. Nodes retain raw fields so
LuaJIT 5.1's `pairs()` can inspect them; LuaJIT has no `__pairs` metamethod.
Their `__newindex` blocks adding a key, but an existing raw field can still be
overwritten. Treat them as checked symbols, not as strictly immutable tables.

Pass a node or its plain dotted string wherever `mux.error` accepts a code.

## See Also

- [`mux.error`](../)
- [`Error`](../type-error/)
- [`mux.error.namespace`](../namespace/)
