---
title: strip_style
type: docs
toc_hide: false
---

Removes styled-text markup and ANSI styling from a string.

## Function

### Synopsis

```lua
mux.text.strip_style( value )
```

### Arguments

`string value`
: Styled or unstyled text.

### Returns

`string plain`
: The visible unstyled text.

## Examples

```lua
local command = mux.text.strip_style(styled_name)
```

## Notes

This is useful before using player-facing text as a command or identifier.

## See Also

- [`mux`](../../)
- [`mux.text.markup`](../markup/)
- [`mux.text.width`](../width/)
