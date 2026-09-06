# Shared Lua packages

This directory is for require-only helpers shared by object logic and global
logic. Require packages with dotted names, for example
`require("formatting.names")` for `packages/formatting/names.lua`.

## Text output in stompymux-rs

`mux.world.pemit(object, value)` accepts a string using C bracket markup, or an
immutable document returned by `mux.text.markdown(source)`. Documents render
independently for each session and follow the same callback rollback rules as
string output. They cannot be concatenated or modified; compose the Markdown
source string before constructing the document.

```lua
local styled = mux.text.style("Notice", {foreground="cyan", bold=true})
mux.world.pemit(ctx.enactor, styled)
mux.world.pemit(ctx.enactor, mux.text.markdown(
  "**Notice**\n\n[Read help](help:about)\n\n`[fg=red]` is literal here."
))
```

`mux.text.markup(value)` validates and returns bracket markup unchanged.
`mux.text.strip_style`, `width` and `truncate` recognize both markup and legacy
SGR; width and truncation use terminal columns and whole Unicode graphemes.
`mux.text.is_printable_ascii(value)` checks bytes in the ASCII printable range.
`style` supports foreground, background, bold, underline and inverse. Invalid
markup or resource-limit violations raise Lua errors.

Markdown enables CommonMark, tables, task lists and strikethrough. Raw HTML is
suppressed. It never interprets bracket tags, including inside code examples.
Documents are transient output values, not persistent Lua state values.
