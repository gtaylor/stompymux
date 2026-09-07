+++
title = "Object and channel locks"
keywords = ["locks", "object locks", "channel locks", "lock_passes"]
wizard_only = true
article_tags = ["show_in_wiz_index"]
description = "Define typed Lua access policies and action messages"
+++

# Object and channel locks

Object modules may declare a `locks` table with lowercase keys and function
values. Global modules cannot declare locks. Unknown keys and non-function
values are rejected at startup. Existing module tables remain live at runtime.

The non-BattleTech catalog is:

```text
MATCH TRAVERSE TAKE USE DROP GIVE RECEIVE ENTER LEAVE
TELEPORT TELEPORT_OUT LINK SET_HOME SPEAK
CHANNEL_JOIN CHANNEL_TRANSMIT CHANNEL_RECEIVE
```

These names are immutable typed constants in `mux.world.locks`. Use lowercase
keys such as `set_home` and `channel_receive` in module declarations.

```lua
return {
  locks = {
    use = function(ctx)
      return {
        passes = mux.world.object(ctx.subject):flags():has(mux.world.flags.WIZARD),
        enactor_message = "Only staff may use this.",
        other_message = "cannot activate it.",
      }
    end,
  },
  messages = {
    use = function(ctx)
      return { enactor_message = "The device activates." }
    end,
  },
  events = {
    on_use = function(ctx)
      mux.world.object(ctx.object):state("device"):set("used", true)
    end,
  },
}
```

Locks return a boolean or a table containing boolean `passes` and optional
`enactor_message` and `other_message` strings. Empty messages suppress default
text. Unknown result fields and malformed values fail the check. Messages must
be shorter than 8192 bytes. Errors discard the callback's changes and output.
A missing attachment or handler allows the check; a missing configured module
or malformed handler does not.

Contexts contain `object`, `enactor`, `cause`, `subject`, lowercase `lock`,
`silent`, an empty `args` table and an applicable `descriptor`. Movement checks
may also include `source` and `destination`. RECEIVE uses the transferred object
as subject. Teleport destination checks use the initiating Wizard as subject
and cause, while the moved object remains enactor.

To test a policy without performing its associated action:

```lua
local allowed = mux.world.lock_passes {
  object = mux.world.object(123),
  enactor = ctx.enactor,
  lock = mux.world.locks.ENTER,
}
```

Only `object`, `enactor`, `lock`, `cause` and `subject` are accepted. Cause and
subject default to enactor. String lock names are invalid. This API always sets
`silent=true`, returns false for callback failures, and does not automatically
emit denial messages or failure events. Explicit handler output still follows
normal callback staging.

Native actions select their own failure events: TAKE/TRAVERSE use `on_fail`,
while USE, DROP, GIVE, RECEIVE, ENTER/LEAVE and teleportation have their specific
failure events. SPEAK and LINK do not automatically invoke `on_fail`.

Channel access is granted by Wizard status, a passing channel-object lock, or
the corresponding player/object access bit. Locks do not override an explicit
bit grant. Channel checks are silent. A missing handler on an attached channel
object grants access, so remove the access bit and define a denying handler
when restricting a channel.

State namespaces have no automatic meaning. The bundled default exit calls
`access_policy` for `locks.traverse`; see [exit access](../exit_access.md).
Other modules must explicitly evaluate their own policy state. No separate lock
storage or database migration is required.
