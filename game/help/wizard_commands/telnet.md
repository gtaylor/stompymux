+++
title = "@telnet"
keywords = ["@telnet"]
article_tags = ["wizard_commands"]
description = "Inspect negotiated Telnet state"
wizard_only = true
+++

# @telnet

`@telnet <player>` is a wizard-only connection diagnostic. It shows the
negotiated Telnet options, terminal capabilities, and RFC 1572 NEW-ENVIRON
variables for every active connection belonging to the named player.
Values are grouped under the protocol that supplied them: TTYPE/MTTS, NAWS,
CHARSET, NEW-ENVIRON, GMCP, MSSP, MCCP2, or ECHO.

```text
@telnet Alex
```

NEW-ENVIRON `VAR` and `USERVAR` names are separate namespaces. Empty values
are displayed as `""`. Non-printable and protocol-control bytes are escaped as
`\xNN`; the values are untrusted information reported by the client.

Names, account aliases and dbrefs such as `#2` are accepted. Each connection has a
separate block identified by its Rust session ID. Local/remote Q states show
pending negotiations and queued reversals; only YES means negotiated. MCCP2's
starting/active transport state is displayed separately: once started, compression
continues until disconnect even if negotiation later changes to NO. ECHO shows the
requested client behavior, not a guarantee that the client hides input.

Output is private to your invoking session and explicitly marks truncation when
it reaches the configured output limit. No switches are supported. Use `@session`
for connection and byte-counter summaries.

Rendering diagnostics also report effective color depth, the session's `color`
override, and enabled OSC capability names. Advertised terminal capability remains
visible separately from the effective rendering choice.
