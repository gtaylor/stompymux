+++
title = "Flying VTOLs"
description = "Take off, control vertical speed, and land"
keywords = ["takeoff", "vertical", "flight", "vtol"]
+++

Enter a VTOL, take its cockpit with `pilot`, and complete `startup`.
Use `takeoff` to begin launching. A wizard may use `takeoff <delay>` to add
up to 65535 seconds to the launch countdown. `land` cancels a queued launch.
You can set `speed` while landed or during the countdown, but a landed VTOL
does not travel across the ground. Liftoff clears horizontal speed commands
and begins climbing at 60 KPH.
Heading commands are also accepted while landed or preparing to launch.
Only a VTOL with an internal combustion engine burns fuel, and it needs fuel to
change horizontal speed; a fusion-powered VTOL never runs dry. Heading commands
and speed readouts never require fuel.

After liftoff, `heading` and `speed` control horizontal travel. Use
`vertical <kph>` to climb with a positive speed, descend with a negative speed,
or stop changing altitude with zero. `vertical` without an argument shows the
current vertical speed. Hexes and levels come out of one pool of movement
(Total Warfare p. 54): horizontal and vertical speed add up to the VTOL's
maximum, and 10.75 kph of climb or descent covers one level in the time 10.75
kph of travel covers one hex. A VTOL can climb no higher than 500 levels.

A VTOL whose elevation is the level of its hex is on the ground, and the only
move it can make there is to climb; it must be at least one level above the
terrain before it travels horizontally, and a VTOL that stops climbing at the
terrain's level has landed (or crashed, where it cannot land).

Entering a hex at or below the level of its terrain, a building in it, or the
treetops of its woods (two levels, three for ultra-heavy growth) is an
automatic crash in that hex (Total Warfare p. 54): no roll avoids it. A road
through the woods lets a VTOL fly below the treetops along it. A VTOL flying at
flank speed that changes facing and then enters a new hex rolls against its
driving skill to avoid sideslipping (p. 67); on a failure it drifts along its
old course for as many hexes as it missed the roll by, never more than one less
than the hexes it has entered, and crashes if the drift carries it into terrain
or treetops at its elevation or down onto water.

Crash damage is the hexes the VTOL was covering times its tonnage over ten, in
five-point groups on the VTOL hit table (p. 68); damage that reaches internal
structure makes the wreck explode. A VTOL that can still fly after a crash in a
hex it could normally land in has landed there and may take off again;
anywhere else the crash destroys it. Engine Damage in flight calls for a +4
driving roll to land on clear, paved or rough ground or a building (p. 197).

Use `land` near a suitable surface (open ground or a building roof, clear of fire and
smoke), with low horizontal speed and a gentle descent. Landing requires working lift,
and fuel for an internal combustion VTOL. Landing activates applicable mines. You
cannot `shutdown` in flight: land first ("You must land before you can shut down.").
Only an administrative shutdown can cut a flying VTOL's power, and then it falls.
