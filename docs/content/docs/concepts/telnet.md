---
title: Telnet negotiation
description: Details on supported telnet sub-negotiations
type: docs
weight: 37
---

The game uses libtelnet for Telnet command framing and option negotiation. A
new connection negotiates terminal type (TTYPE), window size (NAWS), UTF-8
character set, server status (MSSP), output compression (MCCP2), GMCP, and the
RFC 1572 NEW-ENVIRON option.

## TTYPE and MTTS

The server requests the [Telnet Terminal-Type option][ttype] and sends up to
three `TTYPE SEND` sub-negotiations after the client agrees. This follows the
[MUD Terminal Type Standard (MTTS)][mtts] convention: the first response names
the MUD client, the second identifies its terminal emulation, and the third can
be an `MTTS <bitvector>` capability report.

Terminal type and MTTS select the connection's color depth. `DUMB` disables
color, `XTERM` and terminal names containing `256COLOR` select 256 colors, and
names containing `TRUECOLOR` select truecolor. From an MTTS bitvector the
server recognizes ANSI, 256-color, truecolor, and screen-reader capabilities.
A screen-reader report suppresses styled output unless the player explicitly
overrides the color setting for that session and has the persistent `ANSI` flag
enabled. Before negotiation, the server assumes `vt100` with 16-color ANSI
support.

## NAWS

The server requests [Negotiate About Window Size (NAWS)][naws]. Each valid
four-byte sub-negotiation replaces the connection's width and height with the
two unsigned 16-bit values supplied by the client. The current dimensions are
available to server-side features and to the `@telnet` diagnostic. The defaults
are 80 columns by 25 rows; they are restored if the client declines or later
disables NAWS.

## CHARSET

The server offers the [CHARSET option][charset] and requests `UTF-8` as its only
choice. It also accepts a client-initiated CHARSET request when `UTF-8` appears
among the offered values, and rejects requests that have no supported choice.
CHARSET does not enable transcoding: the game uses UTF-8 for all connections,
defaults to UTF-8 when negotiation is unavailable, and rejects malformed UTF-8
command input.

## MSSP

After a client accepts the [MUD Server Status Protocol (MSSP)][mssp], the server
sends one status sub-negotiation containing:

| Variable | Value |
| --- | --- |
| `NAME` | Configured game name |
| `PLAYERS` | Number of connected player sessions |
| `UPTIME` | Server start time as a Unix timestamp |
| `CODEBASE` | `StompyMUX` |
| `PORT` | Configured Telnet port |

The status is a snapshot produced when MSSP is enabled on the connection; the
server does not stream later changes.

## MCCP2

The server offers [MUD Client Compression Protocol version 2 (MCCP2)][mccp2]
for outbound traffic. When the client accepts, the server sends the empty
MCCP2 start sub-negotiation and immediately begins a zlib stream. Every
subsequent server-to-client byte, including later Telnet negotiations, is
inside that compressed stream. Client-to-server traffic is not compressed;
MCCP3 is not supported.

## GMCP

The server offers the [Generic MUD Communication Protocol (GMCP)][gmcp]. Its
current package support is intentionally small: when an enabled client sends
[`Core.Ping`][gmcp-core], with or without a payload, the server replies with a
bodyless `Core.Ping`. Other GMCP packages are ignored. In particular, clients
should not assume that negotiating the GMCP Telnet option implies support for
the broader collection of standard packages.

## NEW-ENVIRON

After a client accepts [NEW-ENVIRON][new-environ], the server sends an
unqualified `SEND` request. Under RFC 1572 this requests the client's default
environment, including both well-known `VAR` and user-defined `USERVAR`
entries. The server does not request a MUD-specific extension or interpret
received variables as trusted identity or authorization data.

The environment is scoped to one live descriptor and discarded at disconnect.
`IS` and `INFO` messages add, replace, or remove values. `VAR` and `USERVAR`
are separate namespaces, and a defined empty value remains distinct from an
absent value. Storage is limited to 64 entries, 256 bytes per name, 4096 bytes
per value, and 65536 bytes total per connection. Malformed or oversized
messages are rejected atomically.

C code can query a descriptor with
`descriptor_telnet_environment_has()` and
`descriptor_telnet_environment_get()` from
`mux/network/telnet_environment.h`. Names and values are length-delimited so
escaped protocol bytes and empty values are preserved. Lua code has equivalent
functions in the built-in `mux` package. Wizards can inspect all negotiated
state with `@telnet <player>`; non-printable bytes are escaped in its output.
The diagnostic groups each value beneath the Telnet option that supplied it.

## ECHO

[Telnet ECHO][echo] is negotiated dynamically rather than during initial
connection setup. The server asks the client to suppress local echo while the
user enters a password, then restores client-side echo when the input no longer
contains a secret. This is only display protection; password input still
travels over the connection and requires a transport-security layer to be
confidential on the network.

## OSC 8 capabilities

OSC 8 is not a Telnet option and has no separate negotiation. At output time,
the styled-text renderer checks three per-descriptor NEW-ENVIRON USERVARs:
`OSC_HYPERLINKS` for `http:`, `https:`, and `ftp:` links,
`OSC_HYPERLINKS_SEND` for `send:`, and `OSC_HYPERLINKS_PROMPT` for `prompt:`.
Each feature is enabled independently only when its variable is present with
the exact one-byte value `1`. An `INFO` update therefore affects subsequent
output immediately.

Tier 2 visual styling checks `OSC_HYPERLINKS_STYLE_BASIC` for base colors,
font styles, and decorations, and `OSC_HYPERLINKS_STYLE_STATES` for interactive
state overrides. Each is enabled independently only for an exact `1`. The
renderer uses full JSON property names and appends the percent-encoded object
as the link URI's reserved `config` parameter.

Tier 3 checks `OSC_HYPERLINKS_TOOLTIP` for custom hover text and
`OSC_HYPERLINKS_MENU` for context menus and menu titles. Menu actions also
require the corresponding web, send, or prompt capability. Unsupported actions
and redundant separators are filtered before the full-name JSON configuration
is emitted. Styled titles additionally require `OSC_HYPERLINKS_STYLE_BASIC`.

Tier 4 checks `OSC_HYPERLINKS_VISIBILITY`, `OSC_HYPERLINKS_SPOILER`, and
`OSC_HYPERLINKS_DISABLED` independently for dynamic hide/reveal behavior,
spoiler text, and permanently non-interactive links. Unsupported visibility
and spoiler data is omitted. A requested `disabled=true` link is rendered as
plain text if the disabled capability is absent, preserving its non-actionable
intent.

Tier 5 checks `OSC_HYPERLINKS_SELECTION` for client-local radio-button and
checkbox state. An exact value of `1` enables the `selection` JSON object;
without it the primary link remains active without selection behavior.
Selection callbacks add or replace the `selected` query item on decoded send
and prompt commands. Web selection remains local to the client and does not
currently alter the opened URL.

Selection is independent of `OSC_HYPERLINKS_STYLE_STATES`, so behavior can be
enabled without custom selected/disabled visuals.

Tier 6 checks `OSC_HYPERLINKS_COMPACT` and `OSC_HYPERLINKS_PRESETS`
independently. Compact support selects Mudlet's abbreviated JSON keys. Preset
support causes the configured session-scoped definitions to be sent once,
immediately before the first normal output after negotiation, and subsequent
links reference them by name. Clients without preset support receive merged
inline configuration instead.

An advertised Tier 2 through Tier 5 configuration capability, or Tier 6
compact support, reserves the
`config` query parameter in web links, even when a particular link adds no
configuration. Existing parameters with that name are percent-encoded before
output. Preset support separately reserves the `preset` query parameter.

OSC capability checks are independent of the player ANSI flag, negotiated
color depth, and MTTS screen-reader state. Connections without a corresponding
capability receive the link's visible text as a plain fallback.

[charset]: https://www.rfc-editor.org/rfc/rfc2066
[echo]: https://www.rfc-editor.org/rfc/rfc857
[gmcp-core]: https://mudstandards.org/gmcp/core/
[gmcp]: https://mudstandards.org/mud/gmcp/
[mccp2]: https://mudstandards.org/mud/mccp2/
[mssp]: https://mudstandards.org/mud/mssp/
[mtts]: https://mudstandards.org/mud/mtts/
[naws]: https://www.rfc-editor.org/rfc/rfc1073
[new-environ]: https://www.rfc-editor.org/rfc/rfc1572
[ttype]: https://www.rfc-editor.org/rfc/rfc1091
