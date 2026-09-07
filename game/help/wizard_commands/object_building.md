+++
title = "Building objects and rooms"
keywords = ["@open", "@link", "@unlink", "@clone", "object building", "@create", "@dig", "@alias", "@chzone"]
wizard_only = true
article_tags = ["show_in_wiz_index"]
description = "Open and link exits, set homes and droptos, and clone objects"
+++

# Building objects and rooms

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

## Creation and editing

`@create <name>` creates a Thing in your inventory with the configured flags,
Lua parent and a valid home. `@dig[/teleport] <room>[=<exit>[,<return exit>]]`
creates a room and optionally a pair of connecting exits. `/teleport` moves you
to the new room, subject to teleport policy. `/t` is its short form.

```text
@create Toolbox
@dig Workshop=workshop;ws,out;o
@name Toolbox=Tool Chest
@description Tool Chest=[bold]A sturdy chest.[/]
@internal-description Tool Chest=It is dark inside.
@alias PlayerName=ShortName
@chzone Tool Chest=here
```

A normal failure of an optional exit or teleport step retains the room and any
successful steps. Callback failures or failed saves roll back the whole command.
All successful changes are saved before confirmation.

`@alias <player>=<alias>` sets a single login alias; an empty value clears it.
Only players have account aliases. Exit aliases remain part of the name,
separated by semicolons. Account names and aliases share a case-insensitive
uniqueness check. This command does not edit command aliases or macros.

`@chzone <object>=<zone|none>` assigns or clears a zone. You must control both
objects; a zone must be a room or thing. Zoning a non-player clears its WIZARD
flag and all powers. Player flags and powers are retained.
