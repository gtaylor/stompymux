# Computer failure behavior

The Rust selection core is `src/btech/computer_failure.rs`. It selects a typed
effect from current target/display availability and the caller's random stream.
The live host action in `src/btech/computer_runtime.rs` now runs from the server
heartbeat and applies target loss, shutdown and sensor outages through shared
services for Mechs, mobile ground vehicles and VTOLs. Stationary platforms do
not participate.

The reference heartbeat invokes the computer check only at its turn boundary,
for started units whose movement type is not None. Parts-disabled checks consume
no dice. Otherwise the rare gate draws 1–5000 and continues only on 42. The
quality check then draws 1–100. Quality zero means five; the actual catalogue
lookup uses integer `(24 + quality - 1) * 5 / 6`, so nominal qualities 1–5 have
success thresholds 80, 80, 90, 95 and 100. Rust preserves that lookup, including
valid extended catalogue indices, and rejects ratings with no catalogue entry
before consuming dice rather than accessing invalid storage.

A failed quality check draws 1–6, rerolling a six exactly once. The final outcomes
are target loss, tactical display loss, long-range display loss, scanner loss,
all displays lost, and shutdown. Target loss requires a positive unit target;
coordinate targeting alone does not qualify. Tactical and long-range failures
require their own nonzero range. Scanner-only and all-display failures both
require every display range to be nonzero. Failed prerequisites discard the
selected failure without rerolling.

Three library tests cover the quality boundaries, every effect, prerequisite
suppression, draw order, disabled parts and invalid ratings. The full 250-test
library suite passes in `target/audit-computer-failure-selection-final.log`.

## Live effects and recovery

The server checks failures at the shared turn phase after periodic piloting and
stagger handling, matching their relative order in the reference.
Running mobile vehicles keep the simulation active even when stopped and without
scanner observers. Existing recovery events advance every committed second,
including while power and parts are off; newly scheduled events retain their
full delay. Target loss clears the shared selection and trajectory correction.
Shutdown invokes the same fall, collision and crew services as ordinary shutdown.
Failure and recovery messages participate in the enclosing output checkpoint.

Each affected display receives an independent nested random delay, in tactical,
long-range, scanner order. The ordered event queue persists in the native
`btech_sensor_recovery` database extension. Administrative restoration followed
by another fault retains both events; simultaneous recoveries apply in insertion
order. Object deletion and wreck retirement discard the corresponding events.
A failed host action or database commit restores ranges, timers, random streams
and notifications together.

`tests/btech_computer_failures.rs` exercises real selected failures across the
supported chassis, stationary exclusion, shutdown, target loss, all-display
recovery, overlapping timers, database restart and later-unit action rollback.
The server scenario injects a database error before accepting a retry.

## Display and lifecycle acceptance

Live outage tests cover each display independently on Mechs, ground vehicles and
VTOLs. Tactical and long-range centering and native/Lua scanning reject failed
hardware with the reference reply. Cached contacts and target selections survive
both the outage and read-only requests; recovery re-enables the corresponding
queries. Destroyed units receive the hardware restoration without recovery
messages. Database cleanup removes pending events and preserves the source
snapshot. Forced shutdown while moving or airborne produces the same material,
crew, object and notification consequences as ordinary shutdown from the same
post-selection random stream, including the subsequent VTOL descent and impact.

## Radio failure inventory

A complete source search finds only one caller of `mech_generic_failure_check`:
`movement/mech_update_piloting.c`, passing `FAILURE_SYSTEM_COMPUTER`.
`FAILURE_SYSTEM_RADIO` appears only in its enum declaration. The radio static,
range-loss and short-circuit handlers are private to `character/failures.c` and
are reachable only through that generic selector's radio branch. Therefore the
reference has no active radio-failure trigger to reproduce. The unused radio
handlers and `mech_rrec_event` remain unimplemented inventory; adding a new live
trigger would change user-visible behavior and requires a separate feature
decision. Existing radio range and interference behavior is unaffected.

The implemented recovery value behavior follows this reference detail: the reference
`mech_srec_event` distinguishes displays with encoded values containing offsets
256 and 512, but passes the complete encoded value to range setters. Those
setters clamp to signed-char range. Thus long-range and scanner recovery reach
127 rather than restoring their original values. This is observable executable
behavior; do not silently treat the encoding as decoded restoration. Tactical
recovery uses its captured value directly. Destroyed units suppress recovery
messages but still receive the range write.

Read-only reference sources: `character/failures.c`,
`movement/mech_update_piloting.c`, and `unit/mech_electronics_state.c`.
