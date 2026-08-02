---
title: Telnet negotiation
type: docs
weight: 37
---

# Telnet negotiation

The game uses libtelnet for Telnet command framing and option negotiation. A
new connection negotiates terminal type (TTYPE), window size (NAWS), UTF-8
character set, server status (MSSP), output compression (MCCP2), GMCP, and the
RFC 1572 NEW-ENVIRON option.

## NEW-ENVIRON

After a client accepts NEW-ENVIRON, the server sends an unqualified `SEND`
request. Under RFC 1572 this requests the client's default environment,
including both well-known `VAR` and user-defined `USERVAR` entries. The server
does not request a MUD-specific extension or interpret received variables as
trusted identity or authorization data.

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

## OSC 8 capabilities

OSC 8 is not a Telnet option and has no separate negotiation. At output time,
the styled-text renderer checks three per-descriptor NEW-ENVIRON USERVARs:
`OSC_HYPERLINKS` for `http:`, `https:`, and `ftp:` links,
`OSC_HYPERLINKS_SEND` for `send:`, and `OSC_HYPERLINKS_PROMPT` for `prompt:`.
Each feature is enabled independently only when its variable is present with
the exact one-byte value `1`. An `INFO` update therefore affects subsequent
output immediately.

OSC capability checks are independent of the player ANSI flag, negotiated
color depth, and MTTS screen-reader state. Connections without a corresponding
capability receive the link's visible text as a plain fallback.
