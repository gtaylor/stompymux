+++
title = "@log"
description = "Append a message to an existing server log"
keywords = ["@log", "logging", "logfiles"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# Logging

`@log <filename>=<message>` appends a message and newline to an existing readable,
writable regular file in `game/logs`. No switches are supported. A successful
write replies **Message logged.** Files are not created automatically; filenames
cannot contain `/` or `..` and must be at most 200 bytes. Messages are truncated
to 4094 UTF-8 bytes before the newline.

`@list logfiles` shows cached handles and their idle timeouts. `@list logging`
requires GOD by default and shows effective topics and decorations.

`@admin all_commands=yes` enables command auditing. `suspect_commands` and
`bad_commands` control their respective diagnostics. Password arguments are
redacted, including aliases, macros and queued commands. SUSPECT commands also go
to an existing SuspectsLog channel, independently of stderr switches.

Lua `mux.log(filename, message)` returns true when a request is staged for commit,
not when written. Failed transactions discard their requests; errors after commit
are reported in server diagnostics. Logging-only operations do not save the world.
