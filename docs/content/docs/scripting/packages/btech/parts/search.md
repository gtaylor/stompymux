---
title: search
type: docs
toc_hide: false
---

Searches the part catalogue by name.

## Function

### Synopsis

```lua
btech.parts.search( query )
```

### Arguments

`string query`
: A non-empty, case-insensitive wildcard pattern matched against each complete
  part name.

Use `*` to match any sequence of characters, `?` to match one character, and
`\` to escape the following wildcard character. For example, `*laser*` finds
part names containing "laser".

### Returns

`BtechPart[] parts`
: The matching part records.

## See Also

- [`btech`](../../)
- [`btech.parts`](../)
