+++
title = "Objects and containers"
keywords = ["objects", "get", "take", "drop", "give", "use", "enter", "leave", "inventory", "look"]
article_tags = ["show_in_index"]
description = "Carry, give and use objects; enter and leave containers"
+++

# Objects and containers

| Command | Action |
| --- | --- |
| `get <object>` or `take <object>` | Pick up a nearby object |
| `get <container>'s <object>` | Take an object from a nearby container |
| `drop <object>` | Put a carried object in your current location |
| `give <recipient>=<object>` | Give a carried object to a nearby recipient |
| `inventory` or `inv` | List your possessions |
| `use <object>` | Invoke an object's scripted use behavior |
| `enter <container>` | Enter a nearby thing or player |
| `leave` | Return to the location containing your current container |

Names are matched within the command's allowed local area. Ambiguous names
must be made more specific. A local dbref may identify an object exactly;
it does not let you take a remote object. Containers travel with their contents.
You cannot put an object inside itself or one of its descendants.

Each action may have a Lua access policy. A failed `MATCH` policy lowers a
candidate's matching preference; it does not by itself prevent the action.
`TAKE`, `DROP`, `USE`, `GIVE` and `RECEIVE` determine whether the selected action
is allowed. Giving checks the item's GIVE policy, then the recipient's RECEIVE
policy against the item. The command transfers objects, not money.

Entering checks the destination's ENTER policy followed by the source's LEAVE
policy. Leaving checks those policies in the opposite order. Rooms marked
AUDITORIUM also apply their SPEAK policy to speech.

These commands accept no switches. An object's scripted messages may explain
why an operation was denied.

A dropped object follows a room's configured dropto after the initial drop.
Ordinary movement and droptos do not apply teleport policies. All changes save
before success is delivered; failed saves restore the previous world state.

## Looking

`look` or `l` displays your immediate location. `look <object>` examines a local
object, an exit alias, `me`, `here`, or a possession such as `Cabinet's Badge`.
Targets must be within the visible local scope, including explicit dbrefs.

`look` accepts no switches.

Objects may supply internal and external Lua appearance callbacks. Otherwise,
look displays descriptions and visible contents; an internal description takes
precedence while inside a non-room object. Transparent exits can show their
destination. Describe callbacks run transactionally, and failed callbacks or
saves discard their changes and pending output.

## Inventory and portable commands

`inventory` lists direct possessions, with object identities available to Wizards.
Names retain their styling. Carried exits appear separately under `Exits:` using
their first alias; they cannot be traversed by typing their name. Large inventories
arrive in bounded chunks, with an explicit notice if the total output limit is reached.

A carried object's Lua commands are available to its carrier. Dropping or giving
it away changes who can use those commands. Contents of carried containers are
not searched recursively. Commands try current-location exits, local Lua, local
native, global Lua, then built-in global native commands; the first handled match
wins. Zone commands follow immediate surroundings and inventory in the local Lua
stage. This lets game scripts intentionally override built-ins.
