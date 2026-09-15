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
Changing horizontal speed requires fuel unless fusion engines are exempt;
heading commands and speed readouts do not require fuel.

After liftoff, `heading` and `speed` control horizontal travel. Use
`vertical <kph>` to climb with a positive speed, descend with a negative speed,
or stop changing altitude with zero. `vertical` without an argument shows the
current vertical speed. Horizontal and vertical commands share the aircraft's
available speed: a faster climb leaves less speed for forward travel.

Keep at least two elevation levels above the surface when entering a forest
hex. Below that clearance, an active pilot must pass a piloting check with a
+5 penalty to stop before entry. Failure crashes the VTOL into the forest.
Stopping rolls you back to the previous hex at that terrain's elevation.

Flying into a higher obstacle also rolls you back. Your pilot can make an
emergency landing over grass, road or a building after a successful control
check; otherwise the aircraft crashes. This emergency landing stops actual
horizontal and vertical speed but retains your selected horizontal speed.

Use `land` near a suitable surface, with low horizontal speed and a gentle
descent. Landing requires working lift and fuel unless the server exempts
fusion engines. Landing activates applicable mines. Shutting down in flight
starts a fall; it does not perform a controlled landing.
