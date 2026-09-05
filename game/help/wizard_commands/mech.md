+++
title = "@mech"
keywords = ["@mech", "unit administration", "setarmor", "restore", "loadnew"]
article_tags = ["wizard_commands", "battletech"]
description = "Administer the BattleTech unit you are inside"
wizard_only = true
+++

# @mech

`@mech` provides wizard-only unit administration. Stand inside the unit you
want to change, then select an operation with a command switch. Enter `@mech`
without a switch to see the complete switch list and a brief description of
each operation.

Common operations include:

```text
@mech/loadnew <template>
@mech/restore
@mech/savenew <template>
@mech/setarmor <location> [front] [internal] [rear]
@mech/repair <location> <type> [value]
@mech/reload <name> <location> <critical-slot> [mode]
@mech/restock <location> <critical-slot>
@mech/display <location>
@mech/showtech
```

Construction and configuration switches are:

```text
@mech/addweap <name> <location> <critical-sections> [R|T|O]
@mech/resetcrits
@mech/firemode <weapon-number> <mode>
@mech/addsp <item> <location> <critical-slot> [data]
@mech/addtech <type>
@mech/deltech <all|type>
@mech/addinftech <type>
@mech/delinftech
@mech/settons <tons>
@mech/settype <type>
@mech/setmove <movement-type>
@mech/setmaxspeed <mp>
@mech/setheatsinks <count>
@mech/setjumpspeed <mp>
@mech/setlrsrange <hexes>
@mech/settacrange <hexes>
@mech/setscanrange <hexes>
@mech/setradio <level>
@mech/setradiorange <hexes>
@mech/setcargospace <space> <maximum-tons>
```

Numeric values and unit locations are validated before a change is applied.
Armor values must be integers from `0` through `255`, and rear armor is valid
only for sections that support it. Speed values must be finite and
non-negative. `RESTOCK` requires a live ammunition critical slot.
