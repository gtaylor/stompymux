+++
title = "Runtime controls and connection messages"
description = "Manage logins, queueing, idle checks and cached text"
keywords = ["runtime controls", "@readcache", "readcache"]
article_tags = ["wizard_commands"]
wizard_only = true
+++

# Runtime controls

Wizards use `@enable <control>` and `@disable <control>`. `@list globals` shows
current status. Controls start enabled after restart and survive Lua reload.

| Control | Abbreviation | Effect when disabled |
| --- | --- | --- |
| cleaning | cl | Pause automatic database checks |
| idlechecking | id | Suspend login and inactivity timeouts |
| queueing | qu | Reject new queued work; existing work continues |
| logins | log | Reject ordinary login and public registration |

Wizard/GOD logins bypass disabled logins and the configured session limit.
Existing connections remain online. IDLE power and Wizard status exempt
 authenticated players from inactivity timeout. Re-enabling idle checks does not
reset activity timestamps. Socket failure cleanup and resource limits always apply.
`@halt` works while queueing is disabled. Checkpointing is unsupported.

# Connection messages

`@readcache` refreshes connection, down, full, bad-site and quit text plus optional
welcome banners. It takes no arguments or switches. File sizes and diagnostics
are reported privately; failed replacements retain last-good content. UTF-8 cache
contents share the configured Lua output-byte budget. An over-budget candidate
leaves the current cache unchanged. Help reload is separate.

New connections choose among available cached banners, or use the connect file.
Refused admissions receive the down/full file and optional configured message.
Bad-site text is cached for future site enforcement. Edits appear after reload or
restart, and current sessions are not rebannered. Controls and cache reload do
not write configuration or the world database.
