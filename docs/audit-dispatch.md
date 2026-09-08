# Command dispatch, matching, macros and queues audit

This tranche compares the C command path against the Rust rewrite and corrects
five unapproved differences: native/Lua precedence and local accumulation,
runtime source traversal, interactive-only macro expansion, room-zone exit
fallback, and executor lifecycle preflight. Tests and probes
use copied fixture worlds; no production game or database is opened.

## Evidence boundaries

Source tracing establishes control flow and selected matching policies. Rust
regressions establish the corrected implementation. The isolated TCP probe is the
cross-engine evidence for room-zone exits. Existing Rust tests alone are not
claimed as complete C parity.

The C dispatcher is `process_command` in
`btmux-khi/src/mux/commands/command_dispatch.c`. It performs prefix and interactive
macro handling, communication/BattleTech hooks, local exits, global native lookup,
all eligible local Lua scopes, zone fallback, zone exits, global Lua and the unknown
response. `lua_module_command_match` in `src/mux/lua/lua_commands.c` receives
`stop_on_handled = 0` for object modules and `1` for global modules. The Rust paths
are `stompymux-rs/src/commands/mod.rs`, `src/lua/dispatch.rs`,
`src/commands/sources.rs` and `src/commands/exits.rs`.

The reviewed source anchors are C `command_dispatch.c:531` (interactive macros),
`:578` (bare-exit selection), `:650-741` (local, zone and global dispatch),
`lua_commands.c:165-238` (attachment and list matching), and
`match.c:501-512,542-573` (zone roots and result selection). The corresponding
Rust anchors are `commands/mod.rs:123,201`, `commands/sources.rs:101`,
`lua/dispatch.rs:76`, and `commands/exits.rs:60,85`. Queue parsing and cancellation
were checked at C `command_parser.c:51`, `command_queue.c:276-396,593` and Rust
`commands/queue.rs:134-262,320-384`.

## Corrected findings

**D13 — native and scoped Lua precedence.** Rust previously tried local exits,
local Lua, local native, global Lua, then global native and stopped at the first
handled local source. A Lua pattern could shadow a built-in, including a configured
alias resolving to a native command. Rust now tries local exits, global natives,
direct local Lua, local native extensions, zone fallback, zone exits and global
Lua. Permission or switch denial on a selected native consumes the command. Every
eligible direct local Lua source runs; any true return prevents zone and global
fallback after direct accumulation completes. Global Lua still stops at the first
handled module. `lua_scopes_permissions_captures_and_frozen_registration`,
`native_aliases_precede_lua_and_restricted_lua_falls_through_to_exits` and
`portable_dispatch_stages_and_inventory` cover native collisions, aliases,
restricted declarations, multiple local instances and fallback suppression.

Rust retains its transactional callback boundary: a Lua error restores the command
world/output snapshot instead of being logged and counted as handled as C does.
That is part of the approved transactional-effects difference.

**D14 — queued dot macros.** C calls `do_macro` only when `INTERACTIVE` is true.
Rust previously expanded a player's dot macro, and admitted direct `.create`-style
macro administration, during descriptor-free queued execution. `run_inner` now
gates both paths on `InputOrigin::Interactive`. Queued and nested forced commands
still resolve configured aliases, sample current executor authority and attachments,
and retain their executor/cause split. `queued_context_locks_callbacks_and_session_rejection`
proves a queued dot alias remains unmatched and does not execute its expansion.

**D15 — executor lifecycle preflight.** C rejects invalid and GOING executors, and
rejects HALTED executors except interactive Players, before entering dispatch.
Direct Rust library calls previously bypassed the server queue's partial checks and
could reach global native commands. `commands::executable` now defines the shared
preflight and `run_inner` enforces it. `execution_lifecycle_guard_is_origin_and_type_aware`
covers queued HALTED things plus interactive HALTED and GOING players. The server
uses the same predicate before auditing work, keeping rejected input out of command
audit logs as in C.

**D16 / P02 — room-zone exits.** C reaches `match_zone_exit` only after direct and
zone Lua fallback. The outer condition requires the immediate location's zone to
be a Room and compares the immediate location against the player's zone. The match
helper then searches exits rooted at the player's zone; it does not require that
root to equal the location-zone Room. The earlier probe configured only half of
that state and was not a working reproduction. Rust now applies these independent
preconditions after scoped fallback and before global Lua. The positive and negative
Rust cases, including distinct Room and Thing zone roots, are in
`zone_exit_fallback_matches_working_c_preconditions`. The optional paired probe in
`tests/tools/behavioral_probe.py` records the working precondition explicitly; its
focused matching output is checked in as `tests/fixtures/dispatch-audit.json`.

**D17 — local source traversal and fallback stages.** Rust previously deduplicated
all object sources and excluded GOING and NO_COMMAND objects everywhere. C invokes
the caller directly and then walks the location contents without deduplication, so
an attached caller can run twice. Its direct caller, location and non-room-zone
calls honor NO_COMMAND; its nearby, inventory and room-zone contents walks do not.
Every path skips HALTED attachments, while GOING is not tested by
`lua_command_match`. Rust now preserves these route-specific rules. Location-zone
and player-zone fallbacks are separate stages: a true result in the former prevents
the latter from running. `lua_scopes_permissions_captures_and_frozen_registration`
and `portable_dispatch_stages_and_inventory` cover the duplicate caller, list-stage
flags and fallback short circuit. Catalog inspection retains a read-only,
deduplicated source view.

## Matching and access coverage

Object matching was traced through C `src/mux/world/match.c` and Rust
`src/commands/objects/mod.rs` plus `src/commands/target.rs`. Both give dbref/token
and exact-name matches priority over prefixes. For ordinary inventory matching,
passing MATCH locks outrank preferred object type, while exact names outrank both;
equal surviving object matches remain ambiguous. Later-word prefix, possessive,
visibility and inventory cases remain covered by `ordinary_word_prefix_matches_c`
and the object command suites. Builder targets accept `me`, `here`, dbrefs, global
`*player` names and visible local word prefixes before applying control checks.

Exit matching has two distinct C contracts. Explicit `goto` uses `match_result` and
reports equal-confidence ambiguity. Bare and zone-exit dispatch use
`last_match_result`, randomly selecting one equally confident exit. Rust now uses
the same split after MATCH-lock preference. The regression checks that bare choices
remain inside the preferred candidate set without requiring one random outcome.

Configured command aliases expand once: a full switched token wins over a base
alias, and the original argument tail is retained. Effective access edits apply to
all registrations with the canonical name. Native and Lua handlers test current
executor flags, type prerequisites and the live queue control at invocation, so a
queued command does not retain admission-time authority. The causal actor never
grants command permission.

Macro slot order, case-insensitive alias lookup, one-pass `*` substitution and `%*`
escaping match C `src/mux/commands/macro.c`; existing `tests/macros.rs` coverage was
retained. Queue list splitting follows C `parse_to` nesting and escape rules.
Cancellation removes ready, delayed and partially consumed lists for the exact
executor; generation, GOING, HALTED and Garbage checks prevent stale execution.
The approved Rust bounds, monotonic deadlines, fair rotation, transaction staging
and descriptor ownership remain unchanged.

## Remaining questions

No confirmed dispatch, target, macro or queue defect remains from the reviewed
paths. Object-local native handlers are a Rust extension and retain their existing
position after direct local Lua because C has no corresponding registration class.
The audit does not claim BattleTech command catalog parity or recursive zone and
inventory discovery, which remain outside this tranche.

## Validation

Focused validation covers `cargo test --test commands`, `cargo test --test macros`,
`cargo test commands::queue::tests` and `cargo test server::queue::tests`. The C TCP
probe ran against `../btmux-khi/build/stompymux` with a temporary copied world;
the distinct zone setup and `zonegate2` established the working C fallback described
above.
