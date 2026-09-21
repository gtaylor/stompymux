# Artillery forward-observer datalink contract

This records the reference contract and current shared Rust implementation.
`src/btech/spotter_events.rs` owns durable requests and maintenance clocks for
Mechs and vehicles. Selection remains in `src/btech/spotter.rs`; artillery
consumers continue to use `src/btech/artillery_firing.rs`.

## Selection and admission

The reference `sensors/mech_spot.c` requires a friendly target that has selected
itself as a spotter. A visible spotter is selected immediately, even for an
artillery unit. Only a shooter with an eligible artillery weapon and no LOS to
the spotter enters the delayed radio-link branch. A non-artillery shooter still
receives the no-LOS refusal. Artillery detection walks `weapon_number_find` with
its ordinary, non-sighting lookup. A nonfunctional first critical, weapon
recycle/reload or section physical recycle returns a negative code and does not
qualify that weapon as artillery. An empty ammunition supply is not checked by
this lookup. Reusing full firing admission would therefore be too restrictive,
while checking only installed weapon types would be too permissive.

The range ceiling is twice the **spotter's** radio range. The shooter's own
radio range is not an additional limit. The pending branch announces the attempt
to both units before testing range, so an out-of-range attempt still publishes
both attempt notices followed by the shooter's range refusal. This is a reported
unsuccessful attempt, not a mutation error that should erase its messages.

Use the existing radio capability query, shared geometry and pilot admission.
Native selection also needs battlefield-label parsing: the current Rust SPOT
command accepts only dbrefs. The reference takes the first two characters of
its sole argument, with `-` selecting no spotter. Reference wording and native
argument precedence need coverage alongside the typed Lua entry point.

## Delayed completion

The delay in seconds is `2 * (trunc(range) / 10 + 5)`, with integer division.
Examples: ranges below 10 take 10 seconds, 10 through below 20 take 12 seconds,
and exactly 20 takes 14 seconds. This uses spatial range, not a weapon bracket.
The selected spotter is not changed until completion. An existing selection is
therefore retained while another link is pending.

The request captures both units' real X/Y positions. At completion, the
reference cancels only if **all four** coordinate comparisons differ: shooter X,
shooter Y, spotter X and spotter Y. Each difference must exceed 0.0001 in reference
real-coordinate units. Pure east/west or north/south movement, movement by only
one participant, and changing altitude alone do not meet that conjunction.
Rust's continuous coordinates are normalized; reference real coordinates use
322.5 times those values. Preserve the float comparison semantics when testing
this boundary rather than comparing tactical hexes or ordinary movement flags.

Completion announces the link to both participants, assigns the spotter and
schedules the first maintenance check ten seconds later. The reference's
shooter completion message formats the shooter's own observer-relative identity
in its forward-observer sentence. That naming quirk must be explicitly covered
or changed as a separately identified behavior decision.

## Maintenance and event lifetime

Every ten seconds, the maintenance event checks its retained observer. It stops
silently if the shooter currently has no selected spotter. Otherwise it clears
the selection and reports loss when the observer is absent, on another map, has
no spotter selected, or exceeds twice its current radio range. The periodic
predicate does not itself require the observer to remain self-selected, powered
or on the same team; firing admission is a separate service and may reject a
link that has not yet reached a maintenance boundary.

SPOT does not cancel existing link-completion events or maintenance events.
Multiple requests can therefore coexist, and clearing or replacing a selection
can be followed by an earlier request's completion. A single replaceable pending
slot would not reproduce this behavior. Simultaneous requests run in insertion order. This follows the reference
`mux/server/event_timer.c` wrapper and its vendored
`third_party/libuv/src/timer.c`: equal deadlines compare increasing `start_id`.
The Rust queue persists global order across both unit stores.

Ordinary shutdown cancels many movement events but does not cancel SPOT_LOCK or
SPOT_CHECK. Whole-unit destruction cancels all events. Rust must retain the
appropriate distinction while cleaning up deleted object identities safely.
Persist pending request clocks, captured positions and maintenance clocks so
restart resumes them without offline catch-up. Do not introduce C pointers or a
second mutable spotter-ownership index.

## Integration and acceptance

Use one lifecycle for both Mech and vehicle shooters and observers. The domain
selection result must carry all participant notices, including unsuccessful
radio attempts, so native and Lua actions can publish them transactionally.
The selection result now carries a vector of participant notices. Both adapters
publish them inside their existing transactions.
Reuse existing artillery correction/reset rules and distinguish direct selection
from delayed completion; the reference only resets the shooter's adjustment on
the immediate visible-target branch.

Required checks include all supported chassis, visible versus radio selection,
artillery versus ordinary launchers, spotter-radio asymmetry, exact range and
delay boundaries, the four-coordinate movement conjunction, multiple requests,
clear/reselect during a pending request, shutdown/destruction, ten-second link
loss, radio changes after establishment, native/Lua output, callback rollback,
save failure and restart. Artillery firing must work after completed selection
and reject unavailable observers through its existing admission path. The
simulation's pending-work predicate must continue ticking a link even when
ordinary movement and weapon work are idle.

## Current verification (2026-09-14)

`tests/btech_spotter_links.rs` verifies all seven supported chassis with empty
Arrow IV launchers, mixed source/observer families, recycling refusal, exact
observer-radio boundaries, ten-hex delay boundaries, all sixteen combinations
of coordinate changes, insertion order, multiple requests, clear/shutdown,
immediate destruction cleanup, correction-reset scope, native/Lua output,
callback rollback, detached inspection and restart. A live server test shuts
both engines down, rejects a database update, verifies no premature completion
message or selection, then permits the save and observes the completed link.

The focused link suite passes 11 tests in
`target/audit-spotter-links-acceptance.log`. Artillery acceptance and regressions
pass 13 tests in `target/audit-spotter-links-artillery.log`, including actual
biped and quad launches after radio completion. Existing mixed indirect-fire
and C3 suites pass 7 and 26 tests in
`target/audit-spotter-links-regressions.log`. All-target Clippy passes in
`target/audit-spotter-links-clippy.log`. These results do not constitute a new
full-suite pass.
The five existing Mech spotter scenarios also pass in
`target/audit-spotter-links-motion.log`, bringing this focused verification to
62 distinct tests.

First-critical loss and same-section physical recycling now have explicit
acceptance tests. A maintenance-order regression is fixed: clearing selection
stops a check silently before it inspects a deleted observer. All 14 link tests
pass in `target/audit-spotter-link-edges.log`.

Remaining acceptance includes float epsilon/altitude boundaries, observer map removal and reassignment during
pending setup, and vehicle artillery launches after radio completion. Unplaced
or deleted participants are retired safely rather than retaining invalid
references or repeatedly failing the server clock; that defensive behavior
still needs its dedicated scenario tests. The broader audit remains open.
