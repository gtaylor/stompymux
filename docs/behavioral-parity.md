# Non-BattleTech behavioral parity audit

The latest [six-area follow-up](behavioral-audit-six.md) records additional
corrections and evidence against Rust `74246d9`. Historical baseline rows below
retain their original scope; the machine-readable matrix also includes the new
focused contracts.

Baseline: C `2bbbe6fc`, Rust `3a1c559`, followed by this movement correction.
Evidence consists of C source inspection and the named Rust regression tests.
The initial movement audit did not start C. The [second comparison](behavioral-audit-round2.md)
adds isolated paired C/Rust TCP evidence and seven new discrepancies (D06–D12). D12 now restores plain `goto` and aliases through shared exit travel.
D09 has since been corrected with direct
player page delivery. D06–D12 are now resolved, with positive regressions and corrected paired transcripts linked in the second comparison.
No production world was exercised. A passing
Rust test alone is not proof of C parity. “Verified equivalent” below describes
only the selected contract stated in that row, subject to separately documented
intentional differences. Broad behavioral cross-products remain unverified.

The machine-readable [matrix](../tests/fixtures/behavioral-parity.json) records
C/Rust references, evidence and status for every row. Existing command/switch,
access, Lua callable, lock, configuration, speech and text catalogs remain the
sources for individual entries; this audit does not duplicate or replace them.

## Approved movement corrections

For container teleportation, the normal sequence is:

1. Traveler `messages.teleport_source` (other-message only).
2. Source `messages.leave`, `events.on_leave`; destination `messages.enter_source`.
3. Relocation and appearance.
4. Traveler `messages.teleport`, `events.on_teleport`, `messages.move`, `events.on_move`.
5. Destination `messages.enter`, `events.on_enter`; source `messages.leave_destination`.

Home omits teleport-specific actions, retains its three messages and visible
home announcement, and bypasses locks. Location transitions use cause `-1`;
home object actions also use `-1`. Teleport object actions retain the causal actor.
Message contexts retain source/destination/operation/silent; Rust event contexts
continue to expose source/destination/operation as useful existing extensions.
Descriptors never refer to another player's session. Appearance and all output
remain staged until commit, even though their evaluation order matches C.

Hearing state is captured before action callbacks. DARK teleportation skips the
teleport providers/events and suppresses location events/neighbor text. Silent
location message providers still execute with `silent=true` and can return direct
text; move providers/events still run. Ordinary location visibility rules remain
applicable. Callback-modified containment is rechecked before relocation, and
final validation protects caught nested calls. Failures discard rich output,
logs, state and maintenance effects. Exit relocation has no occupant actions.

## Resolved audit findings D01–D05

The following reproductions identified five discrepancies. All five are now
corrected against `world/move.c` and `commands/action_messages.c`; their IDs
remain stable in the matrix. Transactional failure behavior remains intentional.

| ID | Minimal reproduction and previous effect | Corrected behavior and evidence |
|---|---|---|
| D01 | A connected DARK Wizard leaves a container whose leave provider returns private text. Rust skipped the provider, text and its mutations. | Silent providers execute and deliver enactor text, suppressing neighbor text and associated events. `generic_actions_match_c_silence_order_and_routing` and `exit_contexts_suppression_and_callback_rollback`. |
| D02 | A thing's success provider returns neighbor text in a room with an AUDIBLE exit to an observer. Rust omitted the remote observer. | Shared action text uses the notification router and C exclusions. `generic_actions_match_c_silence_order_and_routing` and `generic_container_contexts_and_notification_limits`. |
| D03 | Enter a container whose arrival callback changes its description. Rust rendered the changed description rather than the value at relocation. | Appearance runs immediately after relocation and before move/arrival actions. `generic_actions_match_c_silence_order_and_routing`. |
| D04 | GOD issues `@force #2=parityexit`. Rust checked #2's lock but moved GOD instead. | Typed movement requests retain executor, moved object, cause and matched exit separately. `exit_executor_and_action_sequence_match_c`, context assertions and nested-queue TCP coverage. |
| D05 | Attach success/drop handlers to an exit, leave/enter handlers to locations and move to its traveler. Rust only invoked source on_exit and destination on_enter. | Complete C provider/event sequence and immediate appearance; no extra on_exit. `exit_contexts_suppression_and_callback_rollback` and TCP coverage. |

Exit travel now runs success/on_success on the exit, leave/on_leave on the source,
enter_source on the destination, relocation/appearance, drop/on_drop on the exit,
move/on_move on the traveler, enter/on_enter on the destination, then
leave_destination on the source. Exit actions use operation `traverse`;
location/traveler actions use `move`. Rust retains the command cause, as approved;
this is distinct from C's command-entry defaults (M05 below). DARK Wizards
suppress exit events and neighbor messages, but exit providers still run with
`silent=true`. Location visibility is calculated independently. Generic movement
uses the same leave/relocation/appearance/move/enter path without exit actions.

The TCP regression `tcp_state_default_exit_policy_and_failed_write_rollback`
uses the copied default-exit access policy, nested forced execution, two sessions,
persistent state and restart. Injected relational-write failures discard location
changes, rich output and logs; successful logs drain on graceful shutdown.
Per-phase failures and malformed callback containment are covered without
changing production files. Remaining unverified matrix rows are still open.

## Intentional differences and exclusions

**M05 — movement cause retention:** the shared C `move_via_exit` and
`move_via_generic` functions honor an explicit request cause, but their native
callers do not always forward the command cause. `move_exit` and native
enter/leave pass `NOTHING` (`-1`); get/drop pass the executing player.
`move_exit` also uses the player as traversal-lock cause. The approved Rust
request retains the original command cause for ordinary/queued movement,
including forced travel. Tests assert that retained identity; the C-grounded
sequence fixtures must not be read as proof of identical native caller defaults.
Home/teleport special causes remain as described above.

- Preserve successful same-location no-ops, session-owned CONNECTED, generational
  handles, containment safety, callback rollback, staged logs and bounded resources.
- Keep Markdown/HTML and Unicode layout, effective/defaulted configuration lookup,
  safe checking VMs, password redaction and stricter input/path validation.
- Keep stable session IDs and corrected process metric units; keep queue safety
  fixes and atomic relational updates. C prepends containment members; Rust retains
  existing order and appends new members as previously approved.
- Keep `@teleport` destinations restricted to rooms/players/things. Reject
  `goto/quiet` and `@teleport/quiet|loud`. Do not restore `get/drop/give/enter/leave`
  quiet switches, `look/outside`, `#dbref command`, `@find/next` or `@dump`.
- C advertises `@destroy/recursive`, but its handler never uses that bit. Rust
  continues rejecting it; no recursive destruction feature is inferred.
- BattleTech, shutdown reasons, extra protocol options and new web services are
  outside this work. Dormant C configuration is not automatically missing work.

Unverified rows are a remaining review backlog, not known bugs. Full equivalence
is not asserted; resolving these five findings does not verify the remaining backlog.

## Matrix

| ID | Area / selected behavior | Status | Evidence |
|---|---|---|---|
| M01 | movement/locks: Home and container teleport action order, causes and DARK suppression | verified equivalent | [C](../../btmux-khi/src/mux/world/move.c), [Rust](../src/movement.rs), [tests](../tests/movement_parity.rs) |
| M02 | movement/locks: Same-location movement has no transition side effects | intentional difference | [C](../../btmux-khi/src/mux/world/move.c), [Rust](../src/movement.rs), [tests](../tests/movement_parity.rs) |
| M03 | movement/locks: Exit teleport destinations and movement switches | intentional difference | [C](../../btmux-khi/src/mux/commands/wiz.c), [Rust](../src/commands/native.rs), [tests](../tests/movement_parity.rs) |
| M04 | movement/locks: Lock catalog and fail-closed checks | verified equivalent | [C](../../btmux-khi/src/mux/lua/lua_lock_catalog.c), [Rust](../src/locks.rs), [tests](../tests/locks.rs) |
| D01 | movement/locks: Silent generic movement message providers | verified equivalent | [C](../../btmux-khi/src/mux/commands/action_messages.c), [Rust](../src/lua/actions.rs), [tests](../tests/movement_parity.rs) |
| D02 | communication: Ordinary action other_message routing | verified equivalent | [C](../../btmux-khi/src/mux/commands/action_messages.c), [Rust](../src/lua/actions.rs), [tests](../tests/movement_parity.rs) |
| D03 | movement/locks: Generic movement appearance ordering | verified equivalent | [C](../../btmux-khi/src/mux/world/move.c), [Rust](../src/movement.rs), [tests](../tests/movement_parity.rs) |
| C01 | commands/switches: Catalog, scoped matching and access inventory | unverified | [C](../../btmux-khi/src/mux/commands/command_dispatch.c), [Rust](../src/commands/registry.rs), [tests](../tests/commands.rs) |
| C02 | commands/switches: Removed command forms | intentional difference | [C](../../btmux-khi/src/mux/commands/command_table.c), [Rust](../src/commands/registry.rs), [tests](../tests/commands.rs) |
| C03 | commands/switches: @destroy/recursive spelling | intentional difference | [C](../../btmux-khi/src/mux/commands/builder_destroy_commands.c), [Rust](../src/commands/objects/destruction.rs), [tests](../tests/foundation.rs) |
| T01 | targeting/permissions: Control rules and ambiguous target combinations | unverified | [C](../../btmux-khi/src/mux/world/match.c), [Rust](../src/commands/target.rs), [tests](../tests/building.rs) |
| O01 | object lifecycle: GOING, protected objects, purge and relational cleanup | unverified | [C](../../btmux-khi/src/mux/world/database_check.c), [Rust](../src/dbck.rs), [tests](../tests/dbck.rs) |
| O02 | object lifecycle: Containment list insertion order | intentional difference | [C](../../btmux-khi/src/mux/world/move.c), [Rust](../src/persistence/write.rs), [tests](../tests/persistence.rs) |
| S01 | communication: Speech formats and notification routing | verified equivalent | [C](../../btmux-khi/src/mux/communication/speech_format.c), [Rust](../src/communication/speech.rs), [tests](../tests/speech.rs) |
| S02 | communication: Channel/page edge combinations | unverified | [C](../../btmux-khi/src/mux/communication/comsys_context.c), [Rust](../src/communication/mod.rs), [tests](../tests/communication.rs) |
| Q01 | macros/queues: Macro slot priority and single-pass expansion | verified equivalent | [C](../../btmux-khi/src/mux/commands/macro.c), [Rust](../src/macros/mod.rs), [tests](../tests/macros.rs) |
| Q02 | macros/queues: Queue cancellation and resource safety | intentional difference | [C](../../btmux-khi/src/mux/commands/command_dispatch.c), [Rust](../src/commands/queue.rs), [tests](../tests/foundation.rs) |
| Q03 | macros/queues: Nested expansion and execution cross-product | unverified | [C](../../btmux-khi/src/mux/commands/command_dispatch.c), [Rust](../src/server/queue.rs), [tests](../tests/macros.rs) |
| L01 | Lua APIs/callbacks: Callable names and immutable identity contracts | verified equivalent | [C](../../btmux-khi/src/mux/lua/packages/mux/mux_package.c), [Rust](../src/lua/packages/world/handles.rs), [tests](../tests/lua_parity.rs) |
| L02 | Lua APIs/callbacks: All argument coercions and malformed callback return values | unverified | [C](../../btmux-khi/src/mux/lua/lua_callbacks.c), [Rust](../src/lua/actions.rs), [tests](../tests/lua_parity.rs) |
| L03 | Lua APIs/callbacks: Captured schedules and UTC cron timing | verified equivalent | [C](../../btmux-khi/src/mux/lua/lua_schedule.c), [Rust](../src/lua/schedules/mod.rs), [tests](../tests/schedules.rs) |
| L04 | Lua APIs/callbacks: Atomic world and staged side effects | intentional difference | [C](../../btmux-khi/src/mux/commands/action_messages.c), [Rust](../src/lua/transactions.rs), [tests](../tests/lua_parity.rs) |
| A01 | accounts/admission: Retry counts and creation zones | verified equivalent | [C](../../btmux-khi/src/mux/network/connect_flow.c), [Rust](../src/server.rs), [tests](../tests/policies.rs) |
| A02 | accounts/admission: CONNECTED and stale authentication safety | intentional difference | [C](../../btmux-khi/src/mux/network/connect_flow.c), [Rust](../src/server.rs), [tests](../tests/foundation.rs) |
| F01 | configuration: Includes, defaults and typed validation | verified equivalent | [C](../../btmux-khi/src/mux/server/configuration_toml_load.c), [Rust](../src/config/loader.rs), [tests](../tests/configuration.rs) |
| F02 | configuration: All runtime directive interactions | unverified | [C](../../btmux-khi/src/mux/server/configuration_interpreter.c), [Rust](../src/config/administration.rs), [tests](../tests/administration.rs) |
| N01 | Telnet/rendering: Q transitions and implemented option directions | verified equivalent | [C](../../btmux-khi/src/mux/network/telnet_handler.c), [Rust](../src/telnet.rs), [tests](../tests/telnet.rs) |
| N02 | Telnet/rendering: All markup/OSC capability combinations | unverified | [C](../../btmux-khi/src/mux/support/styled_text/markup.h), [Rust](../src/text.rs), [tests](../tests/text.rs) |
| N03 | Telnet/rendering: Markdown, Unicode layout and bounded rich output | intentional difference | [C](../../btmux-khi/src/mux/help/help_render.c), [Rust](../src/text.rs), [tests](../tests/text.rs) |
| H01 | help: Lookup, metadata refresh and invalid-file handling | verified equivalent | [C](../../btmux-khi/src/mux/help/help_index.c), [Rust](../src/help.rs), [tests](../tests/help.rs) |
| G01 | logging: Audit privacy, staged file logs and path confinement | intentional difference | [C](../../btmux-khi/src/mux/server/log.c), [Rust](../src/logging/mod.rs), [tests](../tests/logging.rs) |
| G02 | logging: Complete event-category/format coverage | unverified | [C](../../btmux-khi/src/mux/server/log.c), [Rust](../src/logging/mod.rs), [tests](../tests/logging.rs) |
| R01 | operational reports: WHO and process-report selected contracts | verified equivalent | [C](../../btmux-khi/src/mux/network/connection_commands.c), [Rust](../src/operations/mod.rs), [tests](../tests/operations.rs) |
| R02 | operational reports: Session identifiers and metric units | intentional difference | [C](../../btmux-khi/src/mux/commands/command_list.c), [Rust](../src/operations/process.rs), [tests](../tests/operations.rs) |
| D04 | macros/queues: Forced exit movement moves executor and retains cause | verified equivalent | [C](../../btmux-khi/src/mux/world/movement_commands.c), [Rust](../src/commands/mod.rs), [tests](../tests/movement_parity.rs) |
| D05 | movement/locks: Exit action sequence | verified equivalent | [C](../../btmux-khi/src/mux/world/move.c), [Rust](../src/movement.rs), [tests](../tests/movement_parity.rs) |
| M05 | movement/locks: Original cause retained in ordinary movement callbacks | intentional difference | [C](../../btmux-khi/src/mux/world/movement_commands.c), [C exit entry](../../btmux-khi/src/mux/world/move.c), [Rust](../src/movement.rs), [tests](../tests/movement_parity.rs) |

D07/D08 are resolved: description providers execute before live stored-content
selection, and rooms use internal appearance even when viewed remotely. See the
[correction evidence](behavioral-audit-round2.md#d07d08-correction-evidence).
Other unverified rows retain their status.

D10/D11 are resolved: partial-target pages retain group recipient formatting, and
`mux.text.is_printable_ascii` requires an actual Lua string. See the
[round-two correction evidence](behavioral-audit-round2.md#d10d11-correction).
