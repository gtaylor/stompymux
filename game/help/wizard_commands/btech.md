+++
title = "@btech"
description = "Inspect and manage BattleTech object registration"
keywords = ["@btech", "@btech/info", "@btech/register", "@btech/unregister"]
article_tags = ["wizard_commands", "battletech"]
wizard_only = true
+++

# @btech

Inspect or change an object's typed BattleTech registration:

```text
@btech <object>
@btech/info <object>
@btech/register <object>=<type>
@btech/unregister <object>
```

The bare command is identical to `/info`. Valid registration types are
`MECH`, `DEBUG`, `MAP`, `AUTOPILOT`, and `TURRET`.

Registration is independent of object flags. Registering allocates the typed
runtime object atomically; unregistering disposes its runtime state and typed
configuration. The target must be a controlled live thing.
