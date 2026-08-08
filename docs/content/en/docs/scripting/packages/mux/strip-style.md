---
title: mux.strip_style
linkTitle: mux.strip_style
type: docs
weight: 204
---

# `mux.strip_style`

Removes styled-text markup and ANSI styling from a string.

## Function

### Synopsis

```lua
mux.strip_style( value )
```

### Arguments

`string value`
: Styled or unstyled text.

### Returns

`string plain`
: The visible unstyled text.

## Examples

```lua
local command = mux.strip_style(styled_name)
```

## Notes

This is useful before using player-facing text as a command or identifier.

## See Also

- [`mux`](../)
- [`mux.markup`](../markup/)
- [`mux.text_width`](../text-width/)
