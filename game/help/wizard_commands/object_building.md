+++
title = "Exit linking and cloning"
keywords = ["@open", "@link", "@unlink", "@clone", "object building"]
wizard_only = true
article_tags = ["show_in_wiz_index"]
description = "Open and link exits, set homes and droptos, and clone objects"
+++

# Exit linking and cloning

These commands require Wizard access. Normal object-control rules still apply.
Use `me`, `here`, a nearby name or a dbref; destinations also accept numeric dbrefs.

```text
@open north;n=#123,south;s
@open/inventory hatch=#123
@link north=#123
@link Toolbox=#123
@link here=#123
@unlink north
@clone Toolbox=Spare Toolbox
@clone/inventory Toolbox=Portable Copy
```

`@open <name>[=<destination>[,<return exit>]]` creates an exit in your current
location. `/inventory` creates it on you; `/location` selects the default.
The optional return exit is created at the destination and linked back.
A denied optional link leaves the created exit unlinked for later use.

`@link <object>=<destination>` sets an exit destination, a player/thing home, or
a room dropto. Exit and room linking checks LINK on the destination; setting
home checks SET_HOME. You must satisfy the applicable control rules as well.
A room dropto must lead to a room. Homes must be safe container destinations.

`@unlink <object>` removes an exit destination or room dropto. An empty right
side of `@link` has the same effect. It does not erase player or thing homes.

`@clone <object>[=<new name>]` copies a room, thing or exit, never a player.
It copies persistent object state, descriptions and Lua parent. The new object
uses configured creation defaults, clears WIZARD, and receives a fresh dbref.
Contents, attached exits, accounts, channels and player macros are not copied.
`/inventory` and `/location` select placement. Things receive a valid home;
exits and room droptos attempt to reestablish their link through LINK policy.
A denied link leaves the clone unlinked. The clone then receives `on_clone`.

Normal link denials may leave a newly created object unlinked. Callback errors,
invalid final world state and failed saves roll back the entire command.
