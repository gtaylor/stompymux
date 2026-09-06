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
last value. Colors use the same CSS/X11 or configured names, `#RRGGBB`, and
`rgb(RED,GREEN,BLUE)` values as ordinary styled text.

Tier 3 links can add a custom hover tooltip and a numbered right-click menu:

```text
[send="attack" tooltip="Choose an action" title="Combat" title.bold menu.1.label="Attack" menu.1.send="attack" menu.2.label="Inspect" menu.2.prompt="examine target" menu.3.separator menu.4.label="Guide" menu.4.link="https://example.com/guide"]Attack[/]
```

Menu indices begin at `1` and must be contiguous. Each item is either
`menu.N.separator` or combines `menu.N.label="Label"` with exactly one of
`menu.N.send="command"`, `menu.N.prompt="command"`, or
`menu.N.link="URI"`. Menu labels, actions, `tooltip`, and `title` are quoted;
spaces are allowed, and quotes and backslashes use `\"` and `\\`.

Titles may use the base visual properties with a `title.` prefix, including
`title.color`, `title.bg`, `title.bold`, and the font and decoration properties
listed above. A title requires a menu, and title styling requires title text.

Tier 4 adds dynamic visibility, spoilers, and disabled links:

```text
[send="look" visibility.action=conceal visibility.delay=500]Dismiss me[/]
[send="hint" visibility.action=reveal,conceal visibility.delay=3000]Hint[/]
[send="look" spoiler]Hidden answer[/]
[send="look" spoiler disabled]Reveal-only answer[/]
```

`visibility.action` is required and accepts `conceal`, `reveal`, or
`reveal,conceal`. Optional `visibility.delay` is an unsigned millisecond value.
Visibility may instead expire on `visibility.expire.input`,
`visibility.expire.prompt`, or `visibility.expire.output`; output expiry may
set `visibility.expire.outputDelay`. `visibility.wholeline` applies concealment
to the entire line. Mudlet visibility management supports single-line links
only.

`spoiler` hides the link text until its first activation. `disabled` prevents
the primary action and context menu; combining both creates a reveal-only
spoiler. Boolean properties accept a bare name or `=true`/`=false`.

Tier 5 adds client-local selection groups:

```text
[send="say difficulty hard" selection.group="difficulty" selection.value="hard" selection.exclusive selection.selected selected.bg=red selected.bold]Hard[/]
[send="say strength toggled" selection.group="buffs" selection.value="strength" selection.exclusive=false]Strength[/]
```

Every selection requires quoted, non-empty `selection.group` and
`selection.value` strings. `selection.exclusive=true` creates radio-button
behavior; `false` permits multiple selected values. `selection.toggle=false`
prevents deselection by activating the same link again, `selection.selected`
sets the initial state, and `selection.disabled` marks a selection as disabled.
The `selected.` and `disabled.` visual states can show the current state.

Mudlet appends `selected=true` or `selected=false` to activated `send` and
`prompt` commands, replacing an existing `selected` query item. Treat that
query name as reserved for selection callbacks. Web links may maintain local
selection styling, but current Mudlet versions do not append the callback to
their URL. Selection state and group/value uniqueness are per client console.

Tier 6 can reuse a server-configured preset and then override individual
properties on the link:

```text
[send="attack" preset="osc8-demo-danger"]Attack[/]
[send="repair" preset="osc8-demo-button" bg=blue tooltip="Repair armor"]Repair[/]
```

Preset names are quoted, case-sensitive, and configured by administrators in
the `[osc8.presets]` table of `stompymux.toml`. A link's own directives override
the preset. Presets may provide partial configurations, such as a shared
`selection.group`, as long as the merged link is complete and valid. A link
menu replaces a preset menu as a whole.

Mudlet keyboard and screen-reader support needs no additional markup. Authors
should nevertheless provide short descriptive tooltips and meaningful
selection group/value names because assistive technology announces them.

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

`OSC_HYPERLINKS_TOOLTIP=1` enables custom tooltips, while
`OSC_HYPERLINKS_MENU=1` enables context menus and titles. Menu actions are
included only when the client also advertises their `OSC_HYPERLINKS`,
`OSC_HYPERLINKS_SEND`, or `OSC_HYPERLINKS_PROMPT` scheme. Unsupported actions
and redundant separators are omitted. Title styling additionally requires
`OSC_HYPERLINKS_STYLE_BASIC=1`. Unsupported Tier 3 metadata has no plain-text
fallback and does not affect the link's visible contents.

Tier 4 uses `OSC_HYPERLINKS_VISIBILITY`, `OSC_HYPERLINKS_SPOILER`, and
`OSC_HYPERLINKS_DISABLED`. Each is emitted independently only for an exact
value of `1`. Unsupported visibility and spoiler metadata is omitted. A link
marked `disabled=true` becomes plain text when disabled-link support is absent,
preventing an unavailable action from becoming clickable. Spoilers are only a
display effect: clients without spoiler support display their contents.

Tier 5 uses `OSC_HYPERLINKS_SELECTION`. Without an exact value of `1`, the
selection object is omitted and the underlying link remains active. Selection
does not require custom state styling; `selected.` and `disabled.` properties
still require `OSC_HYPERLINKS_STYLE_STATES=1`. The published specification
includes nested `selection.disabled`; some current Mudlet builds do not yet
consume that nested field, so authors should not use it for security-sensitive
restrictions. Use Tier 4 top-level `disabled` when the entire link must remain
non-interactive.

Tier 6 uses `OSC_HYPERLINKS_COMPACT` and `OSC_HYPERLINKS_PRESETS`. Compact
support changes only the generated JSON property names. Preset definitions are
sent once per connection, before the first normal output after preset support
is learned. Without preset support, the server expands the preset inline so
supported behavior and ANSI-compatible base styling are preserved.

An advertised compact capability reserves the web query parameter `config`.
An advertised preset capability similarly reserves `preset`; existing web
parameters with those names are percent-encoded before generated parameters
are added.

Player-authored say, page, and ordinary channel messages continue to remove all
styled-text markup before delivery.
