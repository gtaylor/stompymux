---
title: truncate
type: docs
toc_hide: false
---

Truncates styled text to a maximum visible width.

## Function

### Synopsis

```lua
mux.text.truncate( value, width )
```

### Arguments

`string value`
: Styled or unstyled text.

`number width`
: A non-negative maximum visible byte width.

### Returns

`string truncated`
: The safely truncated text, including any resets needed for active styles.

## Examples

```lua
local label = mux.text.truncate(mux.text.style("Atlas", { bold = true }), 3)
```

## Notes

A negative width raises a Lua error. Markup and ANSI sequences do not count toward the limit.

## See Also

- [`mux`](../../)
- [`mux.text.width`](../width/)
- [`mux.text.strip_style`](../strip-style/)
