---
title: Styled object text
linkTitle: Styled text
type: docs
weight: 30
---

# Styled object text

Object names, descriptions, and inside descriptions support a small,
non-executable markup language. It replaces the old dependency on softcode
color escapes without restoring softcode evaluation.

```text
@name drone=[fg=bright-cyan]Aegis[/]
@desc drone=A shell of [fg=#d78700]burnished amber[/] metal.
@idesc dropship=[fg=#8090ff]Cool blue light[/] fills the cabin.
```

## Syntax

`[fg=COLOR]` changes the foreground and `[bg=COLOR]` changes the background.
`[bold]`, `[underline]`, and `[inverse]` enable formatting. `[/]` restores the
style active before the matching open tag, while `[reset]` restores terminal
defaults and closes all open styles. Use `[[` for a literal opening bracket.

The named palette contains the eight ANSI names `black`, `red`, `green`,
`yellow`, `blue`, `magenta`, `cyan`, and `white`, plus their `bright-`
variants. `gray` and `grey` alias `bright-black`. Arbitrary RGB colors use
`#RRGGBB`.

Markup is validated when a builder sets the value. Malformed tags and literal
terminal escape sequences are rejected. The validated markup itself is stored
in the database; terminal-specific ANSI is generated only when output is sent
to a client.

Wizards see the markup form of the examined object's name, description, and
inside description in `@examine`, so the exact stored value can be copied,
edited, and set again.

Player-authored `say` and `page` message bodies are plain text channels.
Styled markup and raw terminal escapes are stripped before those messages are
delivered. Styling on player names remains independent.

## Client adaptation

The server discovers terminal capabilities with MTTS-over-TTYPE when the
client supports it. RGB colors are emitted directly for truecolor clients and
mapped to the nearest xterm-256 or ANSI-16 color for less capable clients.
The existing player `ANSI` flag remains the persistent opt-in. Plain clients
and clients reporting MTTS screen-reader mode receive text with formatting
removed.

Color depth is connection-specific, so separate sessions for one player may
receive different encodings.

The `color` command displays or overrides the current connection's selection.
Use `color auto` for negotiation, `color off` for plain text, or `color 16`,
`color 256`, and `color truecolor` to force a depth for the current session.
This also provides an explicit opt-in for a screen-reader session. The
persistent player `ANSI` flag remains the outer control and must be enabled.
