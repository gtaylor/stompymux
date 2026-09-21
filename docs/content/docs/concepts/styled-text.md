---
title: Styled object text
linkTitle: Styled text
description: How to style in-game text
type: docs
weight: 30
---

Object names, descriptions, and inside descriptions support a small,
declarative markup language. It replaces the old dependency on softcode color
escapes without restoring softcode evaluation; OSC link actions occur only
when a user activates a rendered link.

```text
@name drone=[fg=bright-cyan]Aegis[/]
@description drone=A shell of [fg=#d78700]burnished amber[/] metal.
@internal-description dropship=[fg=#8090ff bold]Cool blue light[/] fills the cabin.
```

## Syntax

`[color=COLOR]` changes the foreground; the existing `fg` name remains an
alias. `[bg=COLOR]` changes the background.
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
[send="attack" color=red bold hover.color=yellow]Attack[/]
```

`[link]` accepts `http:`, `https:`, and `ftp:` URIs. Command targets are raw
UTF-8 text and are percent-encoded when rendered. Targets are double quoted;
escape a quote as `\"` and a backslash as `\\`. Formatting may be nested
inside a link, but links may not be nested.

Link-local visual properties share the opening link tag. The base properties
are `color`/`fg`, `bg`, `bold`, `italic`, `underline`, `overline`,
`strikethrough`, and `text-decoration-color`. Boolean values may be bare or
written as `=true`/`=false`; decorations also accept `wavy`, `dotted`, and
`dashed`. Repeated properties use the last value.

State-specific properties use dotted full names, such as
`hover.color=yellow`, `active.bg=bright-red`, or `visited.bold=false`. The
available states are `active`, `hover`, `focus-visible`, `focus`, `visited`,
`selected`, `disabled`, `link`, and `any-link`. Only properties on the link's
opening tag become OSC JSON. Separately nested style tags remain ANSI scopes.

Tier 3 adds quoted `tooltip` and `title` text plus indexed context-menu fields:

```text
[send="attack" tooltip="Choose an action" title="Combat" title.bold menu.1.label="Attack" menu.1.send="attack" menu.2.label="Inspect" menu.2.prompt="examine target" menu.3.separator menu.4.label="Guide" menu.4.link="https://example.com/guide"]Attack[/]
```

Menu indices begin at 1 and are contiguous. An entry is either
`menu.N.separator` or has a quoted `menu.N.label` and exactly one quoted
`menu.N.send`, `menu.N.prompt`, or `menu.N.link` action. Fields may appear in
any order. Title styling uses the base link properties with a `title.` prefix;
state-specific title styles are not supported. All quoted values use the same
`\"` and `\\` escapes as link targets and must contain printable UTF-8.

Tier 4 adds dynamic visibility and behavior fields:

```text
[send="look" visibility.action=conceal visibility.delay=500 visibility.expire.prompt visibility.wholeline]Dismiss[/]
[send="hint" visibility.action=reveal,conceal visibility.delay=3000]Hint[/]
[send="look" spoiler]Hidden answer[/]
[send="look" spoiler disabled]Reveal-only answer[/]
```

Visibility actions are `conceal`, `reveal`, and `reveal,conceal`. Delay and
`visibility.expire.outputDelay` values are unquoted unsigned millisecond
integers. Expiry triggers are `visibility.expire.input`,
`visibility.expire.prompt`, and `visibility.expire.output`; at least one must
be enabled when expiry fields are used. `outputDelay` requires output expiry.
`visibility.wholeline`, `spoiler`, and `disabled` use the standard bare or
explicit boolean syntax.

Tier 5 groups links into client-local selections:

```text
[send="say difficulty hard" selection.group="difficulty" selection.value="hard" selection.exclusive selection.selected selected.bg=red selected.bold]Hard[/]
[send="say strength toggled" selection.group="buffs" selection.value="strength" selection.exclusive=false]Strength[/]
```

Quoted, non-empty `selection.group` and `selection.value` fields are required.
The optional boolean fields are `selection.toggle`, `selection.selected`,
`selection.exclusive`, and `selection.disabled`. Exclusive groups behave as
radio buttons; non-exclusive groups permit multiple selected values. Group
membership and uniqueness are maintained by each client console, not by the
server renderer.

Tier 6 adds reusable, globally configured presets:

```text
[send="attack" preset="osc8-demo-danger"]Attack[/]
[send="repair" preset="osc8-demo-button" bg=blue tooltip="Repair armor"]Repair[/]
```

The quoted preset name is case-sensitive. Link-local directives recursively
override the preset; a link-local menu replaces the preset menu as one array.
Presets may be partial templates, but the merged link must meet the ordinary
menu, visibility, selection, and title validation rules. Unsupported clients
receive the merged configuration inline.

For accessible links, use a descriptive tooltip rather than relying on the raw
command fallback, and choose selection group/value names that communicate
their purpose to assistive technology.

Colors accept an opaque CSS/X11 name, exactly six hexadecimal digits in
`#RRGGBB` form, or `rgb(RED,GREEN,BLUE)` with integer channels from 0 through
255. The RGB function is case-insensitive but does not allow spaces,
percentages, or alpha channels. Game administrators can add case-insensitive
custom names in the `[colors]` table of `stompymux.toml`; configured colors use
`[RED, GREEN, BLUE]` integer arrays and may not overlap a built-in CSS/X11
name.

### Built-in named colors

The following 148 opaque CSS/X11 color names are built in. Names are
case-insensitive; spellings such as `gray`/`grey` and `aqua`/`cyan` are aliases
with the same RGB value. `transparent` is not supported because styled-text
colors do not have an alpha channel.

```text
aliceblue          antiquewhite       aqua               aquamarine
azure              beige              bisque             black
blanchedalmond     blue               blueviolet         brown
burlywood          cadetblue          chartreuse         chocolate
coral              cornflowerblue     cornsilk           crimson
cyan               darkblue           darkcyan           darkgoldenrod
darkgray           darkgreen          darkgrey           darkkhaki
darkmagenta        darkolivegreen     darkorange         darkorchid
darkred            darksalmon         darkseagreen       darkslateblue
darkslategray      darkslategrey      darkturquoise      darkviolet
deeppink           deepskyblue        dimgray            dimgrey
dodgerblue         firebrick          floralwhite        forestgreen
fuchsia            gainsboro          ghostwhite         gold
goldenrod          gray               green              greenyellow
grey               honeydew           hotpink            indianred
indigo             ivory              khaki              lavender
lavenderblush      lawngreen          lemonchiffon       lightblue
lightcoral         lightcyan          lightgoldenrodyellow lightgray
lightgreen         lightgrey          lightpink          lightsalmon
lightseagreen      lightskyblue       lightslategray     lightslategrey
lightsteelblue     lightyellow        lime               limegreen
linen              magenta            maroon             mediumaquamarine
mediumblue         mediumorchid       mediumpurple       mediumseagreen
mediumslateblue    mediumspringgreen  mediumturquoise    mediumvioletred
midnightblue       mintcream          mistyrose          moccasin
navajowhite        navy               oldlace            olive
olivedrab          orange             orangered          orchid
palegoldenrod      palegreen          paleturquoise      palevioletred
papayawhip         peachpuff          peru               pink
plum               powderblue         purple             rebeccapurple
red                rosybrown          royalblue          saddlebrown
salmon             sandybrown         seagreen           seashell
sienna             silver             skyblue            slateblue
slategray          slategrey          snow               springgreen
steelblue          tan                teal               thistle
tomato             turquoise          violet             wheat
white              whitesmoke         yellow             yellowgreen
```

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

Tier 2 uses the exact-`1` USERVARs `OSC_HYPERLINKS_STYLE_BASIC` and
`OSC_HYPERLINKS_STYLE_STATES`. The renderer sends percent-encoded JSON with
full Mudlet property names for advertised features. Base styles otherwise use
ANSI fallback where possible, decoration variants reduce to their plain form,
and interactive states are omitted. OSC styling is capability-driven and
remains independent of the player's ANSI preference; ANSI fallback continues
to respect that preference and the negotiated color depth.

Tier 3 uses `OSC_HYPERLINKS_TOOLTIP` and `OSC_HYPERLINKS_MENU`. The renderer
emits each feature only for an exact `1`; menu entries are further filtered by
the web, send, and prompt scheme capabilities. Filtering removes leading,
trailing, and duplicate separators, and an empty menu also omits its title.
Title styling requires `OSC_HYPERLINKS_STYLE_BASIC`; otherwise the title is
sent without a style object. Tier 3 metadata has no ANSI or plain-text fallback.

Tier 4 uses the exact-`1` USERVARs `OSC_HYPERLINKS_VISIBILITY`,
`OSC_HYPERLINKS_SPOILER`, and `OSC_HYPERLINKS_DISABLED`. Visibility supports
conceal, reveal, and reveal-then-conceal actions, optional millisecond delays,
input/prompt/output expiry triggers, and whole-line concealment. Spoilers hide
their text until activated, while disabled links block their action and menu.
Combining spoiler and disabled creates reveal-only content.

Unsupported visibility and spoiler configuration is omitted. If markup
requests `disabled=true` and the client does not advertise disabled-link
support, the renderer suppresses the OSC link and retains its visible text and
ANSI-compatible base styling. Visibility is intended for single-line links;
spoilers are a display effect rather than protection for confidential data.

When any Tier 2 through Tier 5 configuration capability or Tier 6 compact
capability is advertised, the
renderer percent-encodes an existing web query parameter named `config` so it
cannot be mistaken for Mudlet configuration.

Tier 5 uses the exact-`1` `OSC_HYPERLINKS_SELECTION` USERVAR. Without it, the
selection object is omitted while the ordinary link remains active. Selection
behavior is independent of `OSC_HYPERLINKS_STYLE_STATES`, which separately
controls `selected.` and `disabled.` visual properties.

Mudlet adds or replaces a `selected=true`/`selected=false` query item on
activated send and prompt commands. Selection-enabled commands must reserve
that query name for the callback. Web links can retain client-local selection
state, but current Mudlet code leaves their URL unchanged. The specification's
nested `selection.disabled` is emitted as written; some current Mudlet builds
do not consume it, so top-level Tier 4 `disabled` remains the choice for a
fully non-interactive link.

Tier 6 independently checks exact-`1` `OSC_HYPERLINKS_COMPACT` and
`OSC_HYPERLINKS_PRESETS` USERVARs. Compact clients receive the documented
short property names with no semantic change. Preset-capable clients receive
the global definitions once per connection before the first subsequent normal
output, then links carry `preset=NAME` plus only their overrides. Definitions
and overrides use the same compact or full-name representation. An advertised
preset capability reserves an existing web `preset` query parameter.

The `color` command displays or overrides the current connection's selection.
Use `color auto` for negotiation, `color off` for plain text, or `color 16`,
`color 256`, and `color truecolor` to force a depth for the current session.
This also provides an explicit opt-in for a screen-reader session. The
persistent player `ANSI` flag remains the outer control and must be enabled.
