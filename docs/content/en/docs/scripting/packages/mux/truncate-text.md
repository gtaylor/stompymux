---
title: mux.truncate_text
linkTitle: mux.truncate_text
type: docs
weight: 206
---

# `mux.truncate_text`

Truncates styled text to a maximum visible width.

## Function

### Synopsis

```lua
mux.truncate_text( value, width )
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
local label = mux.truncate_text(mux.style("Atlas", { bold = true }), 3)
```

## Notes

A negative width raises a Lua error. Markup and ANSI sequences do not count toward the limit.

## See Also

- [`mux`](../)
- [`mux.text_width`](../text-width/)
- [`mux.strip_style`](../strip-style/)
