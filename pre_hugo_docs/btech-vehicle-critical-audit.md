# Vehicle critical-status write contract

`unit/mech_script_value.c` assigns `tankcritstatus` directly. The six admitted
letters come from `unit/mech_status_types.h`: turret lock (`a`), turret jam (`b`),
dug in (`c`), digging (`d`), crew stunned (`e`), tail rotor destroyed (`f`).
The reference tree is read-only. The setter now uses independent conditions and pending events, as described below.

## Established behavior

| Condition | Existing Rust owner | Requirement before raw writes |
| --- | --- | --- |
| Turret lock/jam | Vehicle lock/jam booleans and repair countdowns | Preserve heading and pending repair work. Respect the existing surviving-turret constraint; do not turn a field write into material damage. Simultaneous lock and jam are accepted; existing rotation and repair gates still apply. |
| Dug in/digging | `DigState` flags and completion deadlines | Both conditions are separate from pending completion events. Reference bits are independent, including simultaneous `c` and `d`. Raw writes do not schedule a completion event. |
| Crew stunned | Recovery countdown plus effective stun condition | Effective stun is separate from its recovery timer. A raw stun can be indefinite; clearing the condition must not manufacture a new timer or silently cancel an existing one. |
| Tail rotor destroyed | Existing tail-rotor condition, separate from main-rotor material | Change the condition without destroying the main rotor or running the damage notification path. Verify new throttle admission and retained current flight controls. |

`movement/mech_boosters.c:mech_dig_event` returns without changing cover if the
digging bit is clear or the vehicle is not started. Otherwise it clears digging
and sets dug-in. The native dig action schedules completion after 20 seconds.
`core/btech_event.c:mech_stop_digging` explicitly cancels the event and clears the
condition; that cancellation is distinct from a raw status-word edit.

`combat/crit_vehicles.c:mech_vehicle_crew_stun_critical_apply` sets the stun bit,
schedules recovery through `mech_stun_crew`, and limits speed to cruise. The
scheduler uses 60 seconds. `unit/mech_events.c:unstun_crew_event` eventually clears
the bit and announces recovery, even if a raw edit cleared the bit earlier; a
newer scheduled stun delays recovery. A raw flag edit does not call this damage
handler and must not reset the timer or synthesize the throttle transition.

## Implemented representation

The implementation uses explicit conditions and pending countdowns in the existing vehicle owners.
Do not add a second raw status word, a fake one-second remaining value, or a new
20/60-second action when the requested operation only changes a bit. Gameplay
readers ask whether digging, dug-in, or stunned applies; scheduler readers
ask whether a timer exists. Reporting distinguishes an indefinite
condition from a countdown. Native commands retain their normal timed behavior.

Audit driving, weapon readiness, sight, radio, pod removal, cover, landing/drop
admission, destruction cleanup, and snapshot validation when separating those
owners. The shared radio interface now exposes the boolean used by transmission.

Acceptance must exercise raw set/clear without a timer, edits during a timer,
clear-and-set before the original deadline, normal command/damage refresh,
shutdown/destruction, both dig flags together, native/Lua rollback, and restart.
Use tracked and wheeled digging fixtures and all supported vehicle chassis for
crew controls, with Mech rejection and VTOL-only tail-rotor checks.

## Scheduler correction

The shared pending-work predicate omitted digging countdowns. A lone vehicle has
no optical scanner peer, so its stationary dig could cease advancing when other
work expired. `simulation_pending.rs` now includes the digging countdown owner.
The regression creates a single tracked or wheeled vehicle, exhausts the reactor
startup window, proves the baseline is idle, and requires pending work for all
20 dig ticks, including restart at seven seconds. Completed cover becomes idle.
This fix is required independently of the remaining field setter.


`DigState` now stores both flags and distinct completion deadlines. Native
preparation adds a 20-second deadline; raw edits leave all deadlines intact.
Expiry checks current preparation and power, just as the reference callback does.
Crew stun uses an optional explicit condition over the ordinary timed effect;
raw edits retain the timer, fresh stun reinstates the normal effect, and expiry
retires the explicit condition. Consumers and diagnostics distinguish conditions
from scheduled work. The shared radio adapter exposes a boolean stun query.
Turret lock and jam are independently writable; their normal command gates remain.
