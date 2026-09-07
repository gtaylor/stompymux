+++
title = "Objects and containers"
keywords = ["objects", "get", "take", "drop", "give", "use", "enter", "leave", "inventory", "look", "look/outside"]
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

`get/quiet`, `drop/quiet`, `enter/quiet` and `leave/quiet` honor quiet operation
when you control the relevant object. `give/quiet` requires Wizard permission.
These switches do not bypass locks. An object's scripted messages may explain
why an operation was denied.

A dropped object follows a room's configured dropto after the initial drop.
Ordinary movement and droptos do not apply teleport policies. All changes save
before success is delivered; failed saves restore the previous world state.

## Looking

`look` or `l` displays your immediate location. `look <object>` examines a local
object, an exit alias, `me`, `here`, or a possession such as `Cabinet's Badge`.
Targets must be within the visible local scope, including explicit dbrefs.

From inside a player or thing, `look/outside` shows the enclosing location;
`look/outside <object>` matches from that container's perspective. You cannot
look outside a room. `/o` abbreviates `/outside`.

Objects may supply internal and external Lua appearance callbacks. Otherwise,
look displays descriptions and visible contents; an internal description takes
precedence while inside a non-room object. Transparent exits can show their
destination. Describe callbacks run transactionally, and failed callbacks or
saves discard their changes and pending output.
