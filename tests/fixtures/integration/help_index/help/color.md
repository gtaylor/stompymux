+++
title = "Color markup"
description = "Add safe named, hexadecimal, or RGB colors to object text"
keywords = ["color", "colour", "color markup", "hex color", "rgb color", "truecolor"]
article_tags = ["show_in_index"]
+++

# Color markup

Object names, descriptions, and inside descriptions accept color markup. Color
markup changes presentation only; it does not evaluate softcode or Lua.

Use `[color=COLOR]` for foreground color and `[bg=COLOR]` for background
color. The older `fg` name remains an alias.
Separate multiple changes with spaces to apply them as one style. Close the
most recent style with `[/]`.

```text
@name drone=[fg=bright-cyan]Aegis[/]
@attribute/set drone/Desc=A shell of [fg=#d78700]burnished amber[/] metal.
@attribute/set dropship/Idesc=[bg=#101830 fg=bright-white]Cool light fills the cabin.[/]
```

Colors accept three forms: an opaque CSS/X11 name such as `red`, exactly six
hexadecimal digits in `#RRGGBB` form, or `rgb(RED,GREEN,BLUE)` with three
decimal channels from 0 through 255. For example, `red`, `#ff0000`, and
`rgb(255,0,0)` are equivalent. RGB functions do not allow spaces,
percentages, or alpha channels.

The game configuration may provide additional named colors. Custom names work
anywhere a predefined color is accepted, but cannot override a CSS/X11 name.
BattleTech maps, status displays, menus, and notifications use the same named
palette.

Formatting tags are `[bold]`, `[blink]`, `[underline]`, and `[inverse]`. They
can share a tag with colors, as in `[fg=blue bg=white bold]`. One `[/]` closes
everything opened by that tag. `[reset]` closes all active styles. Write `[[`
to display a literal `[` character.

Styled text also supports capability-aware command and web links. See
`help osc8` for `[send]`, `[prompt]`, and `[link]`.

Markup must be correctly nested. Unknown colors, unknown tags, raw terminal
escape sequences, unmatched `[/]`, and unclosed styles are rejected.

Messages sent with `say` or `page` are always plain text. Any markup or raw
terminal formatting in the message body is removed before delivery.
Ordinary player messages sent to communication channels follow the same rule.
Wizard messages sent with `@chan/emit` may contain styling.

The server adapts each resolved color to truecolor, 256-color, or 16-color ANSI
according to the connected client. Players without the ANSI flag, and clients
reporting screen-reader mode, receive plain text.

Use `color` to inspect the current connection's mode. `color auto` restores
negotiated behavior. `color off`, `color 16`, `color 256`, and
`color truecolor` override it for the current connection. An override is
useful when a client reports the wrong capability or when a screen-reader user
explicitly wants styled output. The player ANSI flag must still be enabled.
