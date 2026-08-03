+++
title = "OSC 8 hyperlinks"
description = "Add capability-aware command and web links to styled text"
keywords = ["osc8", "osc 8", "hyperlinks", "send links", "prompt links"]
article_tags = ["show_in_index"]
wizard_only = true
+++

# OSC 8 hyperlinks

Styled text supports OSC 8 links with the same scoped syntax used for colors.
The visible text remains readable when a client does not advertise the
corresponding capability.

```text
[link="https://example.com"]Visit the website[/]
[send="look"]Look[/]
[prompt="cast fireball"]Prepare a spell[/]
[send="attack" color=red bold hover.color=yellow]Attack[/]
```

`[link]` accepts `http:`, `https:`, and `ftp:` URIs. `[send]` immediately sends
a command when clicked, while `[prompt]` places it in the client's input line.
Commands are written normally; the server performs the required URI percent
encoding. Double quotes and backslashes in targets use `\"` and `\\`.

Links may contain color and formatting tags, but links cannot be nested. Link
targets must be printable UTF-8, and web URIs must already use percent encoding
for bytes that are not valid URI characters.

Visual styles can be placed directly on a link's opening tag. Base properties
are `color` (or the older `fg` name), `bg`, `bold`, `italic`, `underline`,
`overline`, `strikethrough`, and `text-decoration-color`. Boolean properties
accept a bare name or `=true`/`=false`. Decorations also accept `wavy`,
`dotted`, and `dashed`:

```text
[send="attack" color=red bg=black bold underline=wavy]Attack[/]
```

Interactive styles use a state and full property name separated by a dot:

```text
[send="attack" color=red hover.color=yellow active.bg=bright-red]Attack[/]
```

The states are `active`, `hover`, `focus-visible`, `focus`, `visited`,
`selected`, `disabled`, `link`, and `any-link`. Repeated properties use the
last value. Colors use the same configured names, built-in names, and
`#RRGGBB` values as ordinary styled text.

The server emits each link only when the connection reports an exact value of
`1` for its NEW-ENVIRON USERVAR: `OSC_HYPERLINKS`,
`OSC_HYPERLINKS_SEND`, or `OSC_HYPERLINKS_PROMPT`. These capabilities are
independent of the player's ANSI setting. Without support, only the text inside
the tag is displayed.

When `OSC_HYPERLINKS_STYLE_BASIC=1`, base styles are emitted as a percent-
encoded OSC 8 `config` object using Mudlet's full JSON property names. When
`OSC_HYPERLINKS_STYLE_STATES=1`, supported state objects are included as well.
Without those capabilities, base properties fall back to ANSI where possible;
state changes are omitted. Decoration variants reduce to their plain ANSI
decoration. Styles nested inside an already-open link remain ordinary ANSI and
are not promoted into the link's OSC configuration.

Player-authored say, page, and ordinary channel messages continue to remove all
styled-text markup before delivery.
