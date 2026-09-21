# Object lifecycle and database repair audit

Audited C baseline `2bbbe6fc` against Rust baseline `74246d9` plus the corrections
described below. The review covers non-BattleTech object creation, cloning, linking,
destruction scheduling, GOING cleanup, home replacement, dependent-record cleanup,
database repair, persistence boundaries, and restart validation. It does not claim
complete equivalence for every malformed SQLite graph.

No production game directory or running server was used. Rust execution used copied
`tests/fixtures/game` trees and temporary SQLite databases. C evidence in this audit is
source tracing through working command and maintenance entry points; no new C server
process was started. Existing C integration coverage was inspected but is not counted as
new paired execution evidence.

## Creation, cloning, and links

The working C allocation path is `create_obj` in
`btmux-khi/src/mux/world/object.c:219`, reached by `do_create`, `do_open`, `do_dig`,
and `do_clone` in `builder_create_commands.c`. Rust uses `World::create_with` and the
handlers in `src/commands/objects/builders.rs`.

The selected contracts are equivalent:

- Room, thing, exit, and player defaults come from their configured flag and Lua-parent
  sets. Rust keeps CONNECTED session-owned, an approved difference.
- Non-player objects inherit the creator's zone; players use a positive configured
  player zone. Invalid or GOING zones are rejected before allocation in Rust.
- Created things choose the actor's controlled location, then controlled home, then the
  configured default/start homes. Clone homes first try the source thing's home. Rust
  additionally requires a containment-safe destination.
- Clone requires Wizard command access, copies owned state, descriptions, and Lua parent,
  clears Wizard on the copy, and recreates only the applicable home, dropto, or exit link.
- Exit/dropto/home linking checks object control, destination control/type, and the link or
  set-home lock. Rust keeps callback rollback and final identity revalidation.

`LC01` corrected clone confirmation behavior. C emits the confirmation before placement
and `on_clone`, and uses `SOURCE cloned as NAME, new copy is object #N.` when a new name
is supplied (`builder_create_commands.c:502`). Rust previously emitted a different form
after `on_clone`. `builders.rs:293` now stages the C form before placement callbacks.
`builder_creation_link_home_clone_and_dropto_roundtrip` verifies text ordering against an
`on_clone` message as well as state, home, dropto, link, and restart behavior.

The C allocator reuses clean garbage dbrefs; Rust retains monotonic ids and generational
handles. That existing safety design is intentional and was not changed.

## Destruction and GOING maintenance

The C command path is `do_destroy` in
`btmux-khi/src/mux/commands/builder_destroy_commands.c:39`, followed by
`object_destroy_schedule`, `database_check`, and the type-specific functions in
`object.c:429-606`. Rust separates scheduling (`src/destruction.rs:13`), semantic
planning (`src/dbck.rs:179`), callback replay (`src/lua/maintenance.rs:16`), and one
SQLite transaction (`src/persistence/mod.rs:181`).

SAFE/override, already-GOING, Wizard-player, and foundational-object guards were traced.
Rust protects dbrefs 0 and 1 plus configured start room, start home, and default home,
and clears corrupt GOING flags on those foundations rather than guessing replacements.
The advertised C `@destroy/recursive` bit is unused by its handler; Rust continues to
reject that switch.

`LC02` corrected occupied-container evacuation. C keeps a doomed room, thing, or player
live with GOING set while `move_via_generic` evacuates surviving contents. This runs the
source leave provider/event, moved-object move provider/event, destination enter
provider/event, routed messages, and cause `NOTHING`, before the source becomes Garbage.
Rust previously planned the tombstone first and manually invoked only `on_exit` and
`on_enter`; source providers could not observe the live GOING object.

`apply_relocations` now reconstructs a valid pre-purge graph, including player accounts
and owned state, applies planned reference repairs to survivors, and uses the shared
generic movement engine. Before tombstoning, a GOING player or thing also takes C's
generic departure to `NOTHING`: its source leave provider/event and its own move
provider/event run with a nil destination. Thing departures and occupant evacuations use
cause `NOTHING`. A scheduled player's departure uses the actor captured when destruction
was scheduled; a GOING player loaded without that runtime identity uses `NOTHING`. The
runtime-only value is cloned for rollback and omitted from persistence, matching C's
restart default. A callback that moves a later occupant out of the doomed container is
retained instead of being overwritten by the original repair plan, matching C's live
`SAFE_DOLIST` ownership check. Tombstones and in-memory macro,
channel, alias, and last-page cleanup are applied only after evacuation callbacks. If the
original graph is itself invalid, repair retains the restricted callback path rather than
passing an unsafe graph to normal movement. Transaction failure still restores world
state, output, logs, cleanup effects, and the database.

The new execution regressions are:

- `maintenance_evacuates_occupied_container_before_tombstoning_it`: a GOING thing source
  remains readable during leave, its own departure and the occupant's complete provider/
  event sequence run with cause -1, direct provider text is retained, and the source then
  becomes unavailable.
- `maintenance_does_not_overwrite_callback_relocation`: an earlier occupant's move event
  relocates a later occupant, and maintenance retains that live callback destination.
- `maintenance_replays_live_player_container_state_before_cleanup`: a GOING player
  source retains its account and state through leave callbacks, accepts a callback state
  write, exposes the scheduling actor as its own departure cause, evacuates its occupant,
  and is then tombstoned with its account removed.
- `maintenance_manual_going_player_uses_nothing_cause`: a player found GOING without a
  runtime scheduler identity exposes cause `NOTHING`; the database repair test verifies
  the scheduling identity is absent after save/reload.
- `maintenance_evacuation_provider_failure_rolls_back_every_effect`: a failing leave
  provider restores the original live GOING container and occupant, discards callback
  state writes and staged logs, emits no stale output, and leaves no maintenance flow.

## Repair, dependencies, and restart

The C checker repairs dead zone, affiliation, home, location, dropto, exit, contents, and
next references; moves orphans home; destroys disconnected/invalid exits; reports
floating rooms; and purges GOING objects. Rust covers the same selected graph fields while
building a deterministic plan before writes. It retains existing approved differences:
atomic repair, unknown dependency rejection, containment safety, append ordering for new
list members, bounded diagnostics, and session-owned CONNECTED.

The current schema cleanup catalog deletes owned player/object rows, removes membership
rows where the destroyed id is a recipient or linked object, clears optional sentinel or
nullable references, and refuses an unrecognized foreign-key dependency. Unknown columns
and unrelated tables are preserved. Player sessions detach, queued work reconciles against
garbage generations, and persisted containment heads/next pointers are rebuilt in the
same commit.

Executed Rust evidence:

- `cargo test --test core_03 dbck::`: six repair, preservation, atomic-failure, foundation, bounded
  report, and startup refusal tests passed.
- `cargo test --test core_02 lua_parity::maintenance_`: seven synchronous/TCP maintenance tests,
  including departure, callback relocation, evacuation, and rollback cases, passed.
- `cargo test --test core_03 locks::builder_creation_link_home_clone_and_dropto_roundtrip`: clone,
  linking, permissions, state, callback order, persistence, and reload passed.

## Remaining limits

Malformed legacy list ownership has a large cross-product. The audit did not execute the
same corrupt SQLite image under both servers, so broad O01 repair equivalence remains
unverified. The restricted callback fallback for graphs that cannot pass normal world
validation deliberately provides only safe repair events; it is not evidence of the full
C movement sequence for arbitrary corruption. BattleTech cleanup tables remain supported
by the schema maintenance catalog but their gameplay semantics are outside this audit.

C boots a destroyed player before its departure callbacks. Rust retains CONNECTED until
the database transaction commits because connections are session-owned and detachments
are staged; player departure hearing and routing context can therefore differ. This is
part of the approved CONNECTED ownership boundary and is not claimed as exact player
callback-context parity.

The C allocator's dbref freelist, Rust's monotonic generational allocation, transactional
rollback, unknown-dependency rejection, bounded resources, appended containment order,
CONNECTED ownership, removed command forms/switches, and lack of BattleTech gameplay are
approved differences and were not treated as defects.
