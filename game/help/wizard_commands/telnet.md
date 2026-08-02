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
