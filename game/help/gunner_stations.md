+++
title = "Gunner Stations"
description = "Claim or release a separate gunner station"
keywords = ["initialize", "deinitialize", "gunner stations", "@setturret", "@viewturret"]
article_tags = ["show_in_index"]
+++

# Gunner Stations

Inside a gunner station, use `initialize` to claim its controls and `deinitialize`
to release them. A connected gunner must leave the station or disconnect before
someone else can take over. Claiming a station again keeps the same assignment.

The station must have a supported parent unit. Its assignment and independent
saved aiming fields survive server restarts.

Use `lock #<unit>` to select one of the parent's acquired contacts, `lock -` to
clear selection, or `lock <x> <y> [H|B|I|C]` for coordinates. Each station keeps
its own target. Stations with an explicit arc mask lock immediately; stations
using the parent's ordinary arcs take eight seconds to settle. Pending locks
retain their remaining time across restarts.

Use `weaponspecs` to inspect the parent’s weapon specifications, `weaponstatus`
for weapon condition, and `critstatus <section>` for installed critical slots.
These reports require a claimed station. Weapon condition requires a placed
parent; condition and critical reports require conscious, unblinded observation.

Use `weapons` to see weapon numbers, ammunition and recycle times.

Use `fire <weapon>` to attack your selected unit, `fire <weapon> #<unit>` for an
explicit target, or `fire <weapon> <x> <y>` for coordinates. Your selected
coordinate purpose controls terrain attacks, including ignition and clearing. Firing uses
your skill and station lock, the parent's ammunition and heat, and your assigned
firing arcs. The parent must be running and off weapons hold. You and the parent
cockpit receive firing feedback.

Artillery uses your artillery skill and targets coordinates. Observed misses build
trajectory correction for your station. Changing your target or the parent’s
observer link clears that correction. Correction and shells in flight survive
restarts; removing the station does not cancel shells already launched.

Use `sight <weapon> [#<unit>|<x> <y>]` to check an attack with your own targeting,
skill and assigned arcs. This also works for artillery. Sighting rolls aim dice
but does not launch, expend ammunition, change heat or recycle, or reveal cover.
You can sight while weapons are on hold or recycling, or ammunition is empty.

`bearing`, `range`, and `vector` use your station’s selected target when you omit
coordinates. `eta [x y]` uses the parent’s current speed and your selected ordinary
hex; it does not plan a route. `findcenter` measures the parent’s exact position
within its current hex. These commands require a running parent.

Use `tactical`, `lrsmap <mode>`, and `navigate` to view the parent’s battlefield.
Their ordinary mode and centering arguments apply. With no center specified,
they follow the parent; viewing a map does not change either operator’s target.
Maps use the parent’s sensors and your saved display size and color preference.
Local navigation can still show the current hex with failed scanner hardware.

`scan [target] [A|I|W|AIW]` and `report [target]` use your station’s selected
target when omitted. Coordinate forms also work: `scan x y [B|H]` inspects an
occupant, building, or full hex, and `report x y` reports an occupant. These use
the parent’s operational scanners and visibility. SCAN can warn the scanned
unit; REPORT is silent. Building and mine results come to your station, and
any perception experience is awarded to you.

`status [armor|info|weapons|heat|short|AIWHSNR]` shows the parent's condition,
equipment and sensors, with your station's selected target and lock countdown.
Status remains available while the parent is shut down.

`contacts [+|options|#unit]` lists contacts seen by the parent. Your own selected
target controls target-only inclusion and highlighting; `+` uses your saved
contact preferences. Building identification checks run as you, with the parent
as the scanner. Losing station access during identification cancels the report.

Wizards inside a station can use `@setturret <field> <value>` and
`@viewturret [1|4][field-prefix]`. Fields are `arcs`, `parent`, `gunner`,
`target`, `targx`, `targy`, `targz`, and `lockmode`. Object references are decimal
numbers; coordinates are independent signed 16-bit values. Edits reset lock
settling and artillery correction. Unknown parents can be stored for setup,
but station combat requires an available supported parent.

Station `addtic`, `deltic`, `cleartic`, `listtic`, and `firetic` are reserved and
do nothing. The parent's pilot retains control of its weapon groups. Use `fire`
for station attacks.
