+++
title = "Vehicle Turrets"
description = "Turn the turret of a ground vehicle"
keywords = ["autoturret", "turret", "fixturret"]
article_tags = ["show_in_index"]
+++

# Vehicle Turrets

After taking the cockpit and starting a ground vehicle, use `turret` to read its
current facing or `turret <degrees>` to turn it. Headings are whole degrees;
values outside 0–359 wrap around. For example, `turret -90` faces 270 degrees.

The facing is relative to the hull: turning the hull also turns the turret.
An immobilized vehicle can still turn an undamaged turret. A damage-locked turret
cannot turn, and a destroyed turret cannot be controlled. Facing and locks survive
server restarts.


Use `fixturret` to clear a jam. The attempt takes 60 seconds and prevents weapon
fire while it is pending. Your vehicle must be running and your pilot conscious
when it finishes. A second hit on a jammed turret locks it permanently; crew
repairs cannot clear that damage. A jam alone does not stop turret weapons firing
along their existing facing.


## Automatic tracking

`autoturret` toggles automatic tracking of the selected unit or hex. You may select
the mode with the reactor off, provided you are the conscious assigned pilot,
the vehicle is on a map and its turret survives.

While running, tracking updates once per second without waiting for the target
lock to settle. Hull turns and target movement change the required facing.
Unconsciousness, turret jams and turret locks pause rotation. Crew stun still
permits turning. The mode survives shutdown and restart. A manual `turret` setting is replaced on the next tracking update while
the mode is enabled. `status` shows whether automatic tracking is on.
