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
Forbidden sites receive cached bad-site text before negotiation or login. Edits appear after reload or
restart, and current sessions are not rebannered. Controls and cache reload do
not write configuration or the world database.

# Site access and monitoring

`@list site_information` (or `@list si`) displays the IPv4 access and suspicion
rules in first-match order. `@telnet <player>` shows captured site status and peer
addresses for each connection. Site rules are loaded from configuration at startup;
`@readcache` changes rejection text, not policy. A site ban applies even to GOD.

Players with MONITOR receive `GAME:` messages for connections, reconnections,
partial disconnects and final disconnects. DARK first connections are labeled
DARK-connected. The existing `Suspect` channel receives notices for SUSPECT players
and suspected-site sessions independently, including each reconnect and partial
disconnect. Trust rules suppress site suspicion only. Missing channels are not
created. Command auditing to `SuspectsLog` remains deferred.
