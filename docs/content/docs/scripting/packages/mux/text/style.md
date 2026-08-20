---
title: style
type: docs
toc_hide: false
---

Applies styled-text markup described by an options table.

## Function

### Synopsis

```lua
mux.text.style( value, options )
```

### Arguments

`string value`
: The text to style; it must not contain an embedded NUL byte.

`table options`
: Style fields to apply.

### Returns

`string styled`
: The input wrapped in validated styled-text markup.

## Examples

```lua
local heading = mux.text.style("Warning", {
  foreground = "#ff7000",
  bold = true,
})
```

## Notes

`options.foreground` and `options.background` accept a built-in or configured color name or `#RRGGBB`. `bold`, `underline`, and `inverse` are booleans. Missing fields have no effect; invalid types and colors raise an error.

## See Also

- [`mux`](../../)
- [`mux.text.markup`](../markup/)
- [`mux.text.strip_style`](../strip-style/)
