---
title: markup
type: docs
toc_hide: false
---

Validates styled-text markup and returns it unchanged.

## Function

### Synopsis

```lua
mux.markup( value )
```

### Arguments

`string value`
: Styled-text markup to validate.

### Returns

`string markup`
: The validated input string.

## Examples

```lua
local warning = mux.markup("[fg=#ff7000][bold]Warning[/][/]")
local look = mux.markup('[send="look"]Look[/]')
```

## Notes

Invalid tags, colors, link targets, or nesting raise a Lua error. `[send]` and `[prompt]` targets are percent-encoded during rendering; `[link]` accepts `http:`, `https:`, and `ftp:` URIs.

## See Also

- [`mux`](../)
- [`mux.style`](../style/)
- [`mux.strip_style`](../strip-style/)
