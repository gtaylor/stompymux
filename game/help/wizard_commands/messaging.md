+++
title = "Message commands"
keywords = ["message commands", "@emit", "@pemit", "@npemit", "@oemit", "@fsay", "@fpose", "@femit", "@wall"]
article_tags = ["wizard_commands"]
description = "Targeted, impersonated and server-wide messages"
wizard_only = true
+++

# Message commands

| Command | Effect |
| --- | --- |
| `@emit <message>` | Emit in your location |
| `@pemit <target>=<message>` | Send to one object |
| `@npemit <target>=<message>` | Same message delivery as @pemit; no expression evaluation |
| `@oemit <target>=<message>` | Emit in the target's location, excluding the target |
| `@fsay <target>=<message>` | Format a controlled object's speech |
| `@fpose <target>=<message>` | Format a controlled object's pose |
| `@femit <target>=<message>` | Emit in a controlled object's location |
| `@wall <message>` | Broadcast to authenticated players |

Targets use local object matching, `me`, `here`, dbrefs and `*player` lookup.
Targeted messages require proximity or control; impersonated messages require
control and do not execute commands or invoke the speaker's SPEAK lock.

`@emit` and `@femit` support `/here` and `/room`, individually or together. The
former means the immediate location; the latter finds the enclosing room through
at most twenty container steps. Selecting both does not duplicate a root that is
already a room. Audible forwarding can still produce distinct delivery paths.

`@pemit` and `@npemit` support `/contents`, `/list`, `/object` and `/silent`.
Contents delivery requires control. Lists are space-separated; repeated targets
receive repeated messages, and an invalid entry does not cancel valid entries.
`/list` takes precedence over `/contents`. `/object` and `/silent` are accepted
compatibility switches and add no behavior.

`@wall/pose` broadcasts a pose; `/emit` omits the speaker name. `/wizard` selects
Wizards with a `Broadcast:` tag. `/admin` also selects Wizards in this fork, using
`Admin:`. Default broadcasts use `Announcement:`. `/no_prefix` suppresses the tag.
Without `/pose` or `/emit`, leading `:` and `;` select spaced and unspaced poses.
Broadcasts bypass object forwarding and local speech restrictions.

Switches accept unambiguous abbreviations. `@fpose/default` and `/nospace` are
accepted; this fork's C bit values make `/nospace` retain the default spacing.

Speech, ordinary poses, `@emit` and wall message bodies have styling stripped.
Targeted and impersonated message bodies retain legacy styling. All recipients
render independently using their session's capabilities. Output is bounded;
callback or persistence failures discard staged messages and mutations.
