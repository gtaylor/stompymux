+++
title = "Color markup"
description = "Add safe named or hexadecimal colors to object text"
keywords = ["color", "colour", "color markup", "hex color", "truecolor"]
article_tags = ["show_in_index"]
+++

# Color markup

Object names, descriptions, and inside descriptions accept color markup. Color
markup changes presentation only; it does not evaluate softcode or Lua.

Use `[fg=COLOR]` for foreground color and `[bg=COLOR]` for background color.
Close the most recent style with `[/]`.

```text
@name drone=[fg=bright-cyan]Aegis[/]
@desc drone=A shell of [fg=#d78700]burnished amber[/] metal.
@idesc dropship=[bg=#101830][fg=bright-white]Cool light fills the cabin.[/][/]
```

The predefined colors are `black`, `red`, `green`, `yellow`, `blue`,
`magenta`, `cyan`, and `white`, plus each name prefixed by `bright-`.
`gray` and `grey` are aliases for `bright-black`. Hexadecimal colors use
exactly six digits in `#RRGGBB` form.

Formatting tags are `[bold]`, `[underline]`, and `[inverse]`. `[reset]` closes
all active styles. Write `[[` to display a literal `[` character.

Markup must be correctly nested. Unknown colors, unknown tags, raw terminal
escape sequences, unmatched `[/]`, and unclosed styles are rejected.

Messages sent with `say` or `page` are always plain text. Any markup or raw
terminal formatting in the message body is removed before delivery.
Ordinary player messages sent to communication channels follow the same rule.
Wizard messages sent with `@chan/emit` may contain styling.

The server adapts hexadecimal color to truecolor, 256-color, or 16-color ANSI
according to the connected client. Players without the ANSI flag, and clients
reporting screen-reader mode, receive plain text.

Use `color` to inspect the current connection's mode. `color auto` restores
negotiated behavior. `color off`, `color 16`, `color 256`, and
`color truecolor` override it for the current connection. An override is
useful when a client reports the wrong capability or when a screen-reader user
explicitly wants styled output. The player ANSI flag must still be enabled.
