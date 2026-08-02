---
title: Styled object text
linkTitle: Styled text
type: docs
weight: 30
---

# Styled object text

Object names, descriptions, and inside descriptions support a small,
declarative markup language. It replaces the old dependency on softcode color
escapes without restoring softcode evaluation; OSC link actions occur only
when a user activates a rendered link.

```text
@name drone=[fg=bright-cyan]Aegis[/]
@desc drone=A shell of [fg=#d78700]burnished amber[/] metal.
@idesc dropship=[fg=#8090ff bold]Cool blue light[/] fills the cabin.
```

## Syntax

`[fg=COLOR]` changes the foreground and `[bg=COLOR]` changes the background.
`[bold]`, `[blink]`, `[underline]`, and `[inverse]` enable formatting. Multiple
directives can share one tag when separated by whitespace, for example
`[fg=blue bg=white bold]`. A grouped tag creates one style scope, so one `[/]`
restores every setting that it changed. `[reset]` restores terminal defaults
and closes all open styles. Use `[[` for a literal opening bracket.

OSC 8 hyperlinks use the same scoped representation:

```text
[link="https://example.com"]Website[/]
[send="look"]Look[/]
[prompt="cast fireball"]Prepare a spell[/]
```

`[link]` accepts `http:`, `https:`, and `ftp:` URIs. Command targets are raw
UTF-8 text and are percent-encoded when rendered. Targets are double quoted;
escape a quote as `\"` and a backslash as `\\`. Formatting may be nested
inside a link, but links may not be nested.

The named palette contains the eight ANSI names `black`, `red`, `green`,
`yellow`, `blue`, `magenta`, `cyan`, and `white`, plus their `bright-`
variants. `gray` and `grey` alias `bright-black`. Arbitrary RGB colors use
`#RRGGBB`. Game administrators can override these names or add new
case-insensitive names in the `[colors]` table of `stompymux.toml`; configured
colors are specified as `[RED, GREEN, BLUE]` arrays.

BattleTech maps, status displays, menus, and notifications use this markup and
the same named palette rather than maintaining a separate terminal-color path.

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
receive different ANSI color sequences. Text encoding is always UTF-8.

OSC support is also connection-specific, but is independent of color and
screen-reader selection. The server checks the `OSC_HYPERLINKS`,
`OSC_HYPERLINKS_SEND`, and `OSC_HYPERLINKS_PROMPT` NEW-ENVIRON USERVARs for an
exact `1` value. Unsupported link tags render as their visible contents without
OSC escape sequences.

The `color` command displays or overrides the current connection's selection.
Use `color auto` for negotiation, `color off` for plain text, or `color 16`,
`color 256`, and `color truecolor` to force a depth for the current session.
This also provides an explicit opt-in for a screen-reader session. The
persistent player `ANSI` flag remains the outer control and must be enabled.
