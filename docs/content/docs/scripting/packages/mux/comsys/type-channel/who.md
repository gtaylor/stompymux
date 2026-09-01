---
title: who
type: docs
toc_hide: false
---

Returns structured channel membership records.

## Function

### Synopsis

```lua
channel:who( options? )
```

### Arguments

`table or nil options`
: Optional settings. The exact options table accepts one field:

  | Field | Type | Description |
  | --- | --- | --- |
  | `all` | boolean | Includes inactive membership records. |

Unknown option fields are rejected.

### Returns

`table members`
: A dense array of records shaped as
  `{ object = Object, listening = boolean }`.

### Raises

- `mux.unavailable.checking` during `@lua/check`.
- `mux.channel.invalid` if the channel handle is stale.
- `mux.arg.invalid` for invalid options.

## Notes

Without `all`, the native active-member test is applied. Trusted Lua does not
apply actor-relative hidden-member filtering. `{ all = true }` corresponds to
`@chan/who <name>/all`.

## See Also

- [`mux`](../../../)
- [`Channel`](../)
- [`Channel:user_count`](../user-count/)
