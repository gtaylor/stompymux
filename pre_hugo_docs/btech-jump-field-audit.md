# Jump heading and length audit

`jumpheading` and `jumplength` now edit both retained course values and active
Mech flight. The shared setter is `src/btech/jump_fields.rs`. The source audit
and deliberate redirection difference are recorded below.

The reference script-value writer directly changes `rd.jumpheading` and
`rd.jumplength`. The Mech movement integrator reads both on every airborne step
(`movement/mech_motion_integration.c`): heading controls horizontal travel, and
length determines progress and the altitude interpolation. A nonpositive length
ends the jump. Landing still checks the separately stored destination hex;
changing heading does not itself move that destination. Consequently arbitrary
independent edits can leave a reference jump travelling away from its landing
location. A Rust implementation must not silently pretend this is just history.

Rust owns validated `BattleJumpPath` geometry and a saved `BattleJumpFlight`
cursor. Distance, endpoint, progress, wrapping and sampled altitude agree by
construction. `LastJump` is a separate report retained after flight retirement.
The implementation uses coherent remaining-route edits, preserving the committed
position and cumulative progress rather than rebuilding from the launch point.

Required acceptance includes an edit before takeoff, an edit during flight,
unchanged immediate position/time/dice, subsequent horizontal and vertical
progress, landing and collision behavior, DFA targets, wrapping/reassignment,
invalid edits, callback rollback and restart at an edited airborne cursor.
These are Mech flight mechanics; vehicle jump thrust used by orbital
compensation is already a separate supported operation. Vehicles reject these
conventional course edits without changing their orbital compensation thrust.

The September 13 follow-up inspected the integrator directly: progress is radial
distance from the launch coordinates after each translated step, not cumulative
path length. The raw heading affects translation, and raw length affects both
that progress ratio and altitude. The catalog exposes both as signed shorts.
Landing tests the separately stored destination hex and launch-to-destination
radius; a nonpositive length instead invokes landing immediately before travel.
These details rule out treating either field as a historical report edit.

An optional user question offered coherent redirection of the remaining route
(recommended) versus independent steering with the old destination. After more
than a minute of independent audit and full-suite work without a reply, coherent
redirection is the stated working assumption, not user approval. A redirected
segment begins at the exact committed point and fractional altitude, preserves
flight progress/dice/DFA intent, and uses the shared collision and landing services.

## Shared continuation geometry and cursor

`BattleJumpPath::continuation` now constructs remaining-route geometry from an
exact airborne point and fractional altitude. The saved route distinguishes a
continuation from ordinary takeoff admission. Descent from an already airborne
sample can exceed the original takeoff elevation difference; horizontal range
and additional climb still use the supplied capacity. Normal launch constructors
and saved launches retain integral takeoff and their existing admission checks.

`BattleJumpFlight::redirect` requires the replacement path to begin at the exact
committed sample. It retains the last thrust sample, DFA target, boundary policy
and cumulative completed distance while replacing the remaining segment. Status
reports total progress; altitude rounding continues to recognize a flight that
has already departed. The existing integrator performs subsequent samples and
landing outcomes. A mismatched replacement rejects before mutating the cursor.

Five new unit tests cover fractional samples and saved replay, normal versus
continuation admission, corrupt saved origins/progress, two successive redirects,
retained DFA intent, subsequent progression and eventual arrival. These are
geometry/cursor acceptance; field-level acceptance is described below.

## Field behavior and acceptance

Heading accepts 0–359 degrees; length accepts signed 16-bit values in field units
(322.5 per hex). Grounded writes retain values without launching. A normal launch
sets the course from its admitted path. During flight, heading retains remaining
distance; length means the new total distance, including completed segments.
The remaining endpoint is projected from the current sample and its terrain
height is resolved on the current map, including wrapping. Range and further
climb use launch-time capacity; current surviving thrust still controls updates.

Nonpositive or already-completed lengths request landing at the current sample
on the next tick, through existing terrain/DFA/landing handling. Heading edits
retain a pending landing. A valid extension before the next tick cancels it.
Same-value edits are idempotent. Same-hex continuation endpoints are valid, while
new takeoffs still reject same-hex destinations. An admitted replacement clears
the old map-reassignment marker so future validation checks its new route.

The Mech matrix covers both limb layouts, grounded and airborne writes, exact
state preservation except course/cursor, native/Lua agreement, repeated edits,
invalid integers/range, callback/outbox rollback, fractional sample retention,
restart, subsequent horizontal/vertical progression and arrival. Additional
cases cover nonpositive, already-completed and short same-hex lengths, extending
a pending landing, retained DFA intent with a relocated landing, wrapping seams,
editing after scenario map reassignment, and terrain collision with deterministic
restart. All five vehicle chassis reject conventional course writes atomically.
