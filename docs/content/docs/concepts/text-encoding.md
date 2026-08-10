---
title: Text encoding
description: An overview of how telnet text encoding is handled
type: docs
weight: 35
---

The server uses UTF-8 for all client text. Telnet connections default to UTF-8
and negotiate `UTF-8` with the CHARSET option when the client supports it.
Malformed UTF-8 commands are rejected instead of being partially interpreted.

Messages, descriptions, attribute values, room and thing names, exit names and
aliases, macro expansions, channel history, help files, and Lua-generated text
may contain UTF-8. SQLite text loaded by the server must contain valid UTF-8.
Legacy ANSI styling remains supported because its escape sequences are ASCII
bytes within the UTF-8 stream.

The following lookup-sensitive values remain printable ASCII:

- player names and player aliases;
- channel names and per-player channel command aliases; and
- macro aliases.

Channel names and channel aliases cannot contain spaces. ASCII identifiers use
their existing case-insensitive matching. UTF-8 exit aliases use
case-insensitive matching for ASCII letters and exact-byte matching for other
characters; the server does not perform Unicode normalization or case folding.

Lua code can apply the same byte-level printable-ASCII test with
`mux.is_printable_ascii(value)`.
