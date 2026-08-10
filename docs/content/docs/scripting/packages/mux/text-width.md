---
title: mux.text_width
type: docs
toc_hide: false
---

Measures the visible byte width of styled text.

## Function

### Synopsis

```lua
mux.text_width( value )
```

### Arguments

`string value`
: Styled or unstyled text.

### Returns

`number width`
: The number of visible bytes, excluding markup and ANSI styling.

## Notes

The result is a byte width, not a Unicode display-cell calculation.

## See Also

- [`mux`](../)
- [`mux.truncate_text`](../truncate-text/)
- [`mux.strip_style`](../strip-style/)
