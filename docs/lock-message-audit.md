# Lock and messaging comparison

This audit found **four groups of non-BattleTech discrepancies** in lock/message behavior. All four are now corrected with C-grounded live-runtime regressions and a separate [corrected paired TCP transcript](../tests/fixtures/lock-message-wire-corrected.json). The [original transcript](../tests/fixtures/lock-message-wire.json) remains unchanged as historical evidence of the pre-fix behavior. The seven D06–D12 reproductions remain corrected; LM03 exercises a different forwarding path than D09's contained-player case.

## Evidence and boundaries

Both servers were exercised over real TCP in separate temporary schema-32 worlds. The [probe](../tests/tools/lock_message_probe.py) installs allow, deny, bare-boolean-denial and erroring policies, captures GOD and Wizard independently, and records every setup command. The [transcript](../tests/fixtures/lock-message-wire.json) contains 60 commands per server. GOD removes Wizard's WIZARD flag before the channel-policy tests. Temporary paths and extra callback context fields are retained rather than silently normalized. The [audit fixture](../tests/fixtures/lock-message-audit.json) records revisions, the current Rust source fingerprint, findings and coverage.

The comparison preserves approved transactions and rollback, staged output/logs/flows, resource limits, safe descriptors and original movement causes. It does not restore removed switches, exit destinations for teleport, `#dbref command`, or `@dump`. BattleTech locks are excluded. Additional event `source`/`destination`/`operation` fields and same-location no-ops remain the documented Rust behavior. These exclusions do **not** cover missing ordinary messages or expanded channel recipients.

Tests in [lock_message_audit.rs](../tests/lock_message_audit.rs) assert the corrected C behavior. They use `Scripts::new` in **Live** mode with copied modules, not the testing-only parent globals. Normal Cargo testing needs no C executable.

## Corrected discrepancies

### LM01 — Channel traffic leaks through subscribed things (high)

The following is the historical pre-fix observation retained for comparison.

Create LockBox, create channel Probe, force LockBox to `addcom box=Probe`, then teleport Wizard inside LockBox. Wizard has no membership in Probe. Execute `@chan/emit Probe=CHANNEL LEAK`.

- C: Wizard receives nothing.
- Rust: Wizard receives `[Probe] CHANNEL LEAK`.

[C channel delivery](../../btmux-khi/src/mux/communication/channel_delivery.c) uses `notify_checked(MSG_ME_ALL | MSG_F_DOWN)` for non-player members. In this fork, `MSG_F_DOWN` is the inactive `MSG_INV_L` branch; it does not recursively notify contents. [Rust `Service::deliver`](../src/communication/delivery.rs) explicitly descends through non-player recipients. A player can therefore receive a channel without their own membership, listening preference or receive policy being checked. If independently subscribed while inside, the first exploratory probe also observed duplicate messages; the checked-in final fixture isolates the nonmember case.

**Correction:** use the C-supported notification policy for object recipients, preserving AUDIBLE exit behavior and direct delivery to subscribed connected players. Do not replace all object notifications with either recursive contents delivery or unconditional suppression. Test both unauthorized occupants and independently subscribed occupants.

**Corrected evidence:** object members now route through the `DIRECT` notification policy, while connected player members retain direct delivery. The regression excludes unnamed occupants, verifies an AUDIBLE exit, and verifies one copy for an independently subscribed occupant.

Evidence: `channel_thing_delivers_to_unnamed_occupant`; transcript command `@chan/emit Probe=CHANNEL LEAK`.

### LM02 — Channel leave callbacks fail in the live runtime (high)

The following is the historical pre-fix observation retained for comparison.

Keep LockBox subscribed with an `events.on_leave` callback. Join Wizard using `addcom p=Probe`, then issue `p off` or `delcom p`.

- C: invokes the thing's `on_leave`, reports departure and changes membership/listening state.
- Rust: errors with `attempt to index global '_parents' (a nil value)`; the transaction leaves membership unchanged. A subsequent `p on` reports that Wizard is already on the channel.

[Rust `leave_callbacks`](../src/communication/membership.rs) compiles a closure accessing `_parents`. That global is only installed in [Testing mode](../src/lua/testing.rs), while [live module loading](../src/lua/loading.rs) stores parents in a private registry. The existing callback-error regression supplies the test globals, masking this live failure. The callback need not itself be broken: the lookup fails before it runs.

**Correction:** invoke the captured live module through the runtime's event machinery, without exposing testing globals. Preserve [C channel membership ordering](../../btmux-khi/src/mux/communication/channel_aliases.c), applicable event eligibility, callback context and transactional rollback. Cover off, alias deletion, allcom-off and administrative removal with real loaded thing modules. The first two operations are executed here; the others share relevant code and need dedicated regressions.

**Corrected evidence:** leave hooks use the captured runtime service and normal event dispatcher. Live regressions cover alias `off`, `delcom`, `allcom off`, and administrative boot, with C's reverse membership order and index-zero omission preserved.

Evidence: `channel_leave_fails_in_live_runtime`; transcript `p off`, `p on`, `delcom p`.

### LM03 — Direct action/denial messages and pages skip AUDIBLE exits (medium)

The following is the historical pre-fix observation retained for comparison.

Attach an AUDIBLE exit named ear to GOD, linked to room #4. Put Wizard in #4. LockBox's USE policy denies with `enactor_message="DENY use"`.

- `@pemit GOD=CONTROL`: both engines deliver `From a distance, CONTROL` to Wizard.
- `use LockBox`: C forwards `From a distance, DENY use`; Rust omits it. Explicit `pemit` tracing from the provider/event still arrives in both engines, proving the exit path works.
- `page GOD=ROUTED PAGE`: C forwards `From a distance, GOD pages: ROUTED PAGE`; Rust delivers only to GOD.

[C action and denial messaging](../../btmux-khi/src/mux/commands/action_messages.c) uses `MSG_ME_ALL`, whose `MSG_INV_EXITS` branch is active in [the notifier](../../btmux-khi/src/mux/server/game.c). [Rust action_text](../src/lua/actions.rs) pushes direct messages straight into the outbox; [page](../src/communication/page.rs) uses direct `notify`. The same shared direct-action path also serves successful action messages, although the paired case tests denial specifically.

**Correction:** route applicable enactor messages through the existing DIRECT notification policy. Keep the earlier D09 correction's exclusion of contained non-recipients; C's AUDIBLE exits are a separate, explicitly configured route. No prior decision to disable this route was found. If intentionally stricter page privacy is desired, record that decision separately rather than calling direct-only delivery C parity.

**Corrected evidence:** action/denial and page recipient messages now use `DIRECT`, which includes the addressed player and configured AUDIBLE exits without descending into contained nonrecipients.

Evidence: `direct_lock_messages_and_pages_bypass_audible_exits`; transcript `@pemit GOD=CONTROL`, the later `use LockBox`, and `page GOD=ROUTED PAGE`.

### LM04 — Native default messages differ (low)

The following is the historical pre-fix observation retained for comparison.

These are normal booleans and successful operations, not callback-error/rollback differences:

| Case | C | Rust |
|---|---|---|
| TAKE returns false without custom text | `You can't pick that up.` | `You can't take that.` |
| GIVE returns false for LockBox | `You can't give LockBox away.` | `You can't give that away.` |
| RECEIVE returns false on Wizard | `Wizard doesn't want LockBox.` | `That recipient won't accept the object.` |
| Successful drop, neighbor text | `GOD dropped LockBox.` | `GOD drops LockBox.` |
| Successful `addcom p=Probe` | Final `Channel Probe added with alias p.` | Final confirmation absent |
| `@chan/object Probe=#16` | Identifies channel and LockBox | Generic `@chan/object: Set.` |

Compare [C inventory](../../btmux-khi/src/mux/world/inventory_commands.c), [give/receive](../../btmux-khi/src/mux/commands/rob.c), [channel aliases](../../btmux-khi/src/mux/communication/channel_aliases.c) and [channel administration](../../btmux-khi/src/mux/communication/channel_administration.c) against [Rust inventory](../src/commands/objects/inventory.rs), [membership](../src/communication/membership.rs) and [channel commands](../src/communication/commands.rs).

**Correction:** restore caller-specific defaults and success confirmations, while retaining explicit empty-message suppression and provider overrides. The fixture includes all six observations; the runtime characterization directly covers give/receive.

**Corrected evidence:** the positive regression covers TAKE, GIVE and RECEIVE defaults, drop neighbor tense, `addcom` completion, and the object-specific `@chan/object` response including standard dbref flag suffixes.

## Lock coverage

Catalog presence alone is not a parity claim. All 17 non-BattleTech lock identities were compared with the C catalog; the following narrows the tested contracts.

| Lock(s) | Evidence and result |
|---|---|
| MATCH | TCP take matching invokes it silently before TAKE; enactor/subject/object agree. Existing ambiguity/preference tests retained. Exhaustive ambiguous scope combinations remain unverified. |
| TAKE, USE, DROP, GIVE | TCP explicit denials agree on invocation identity, direct custom text, prefixed nearby text, exclusions and selected failure event. LM03/LM04 qualify forwarding/default parity. |
| RECEIVE | TCP verifies subject is the given object, enactor is the giver, and GIVE precedes RECEIVE. The corrected default now matches C (LM04). |
| ENTER | TCP denial reaches the target policy and fires on_enter_fail. Source review and existing tests cover destination ENTER then source LEAVE. |
| LEAVE | Native leave's source LEAVE then destination ENTER is source-reviewed and covered by existing Rust tests; no new paired native-leave denial scenario here. Channel on_leave is an event, not this lock (LM02). |
| TRAVERSE | Existing copied-exit, forced-executor, DARK-silence and failure regressions retained. The new probe does not repeat the full traversal matrix. |
| TELEPORT, TELEPORT_OUT | TCP traces match moved-object enactor and initiating/moved subject identities. Source review retains nested source-chain checks and home bypass. Full nested-denial text variants were not paired. |
| LINK, SET_HOME | Source-reviewed native destination checks and existing builder tests retained. No new paired live permission-change/link-failure run. |
| SPEAK | Auditorium say, pose and @emit all invoke SPEAK and deliver the same custom denial to GOD and prefixed neighbor message to Wizard. GAGGED/Wizard gates source-reviewed; existing speech tests retained. |
| CHANNEL_JOIN, CHANNEL_TRANSMIT, CHANNEL_RECEIVE | With an ordinary player, TCP confirms silent contexts and matching subject/enactor. False locks still allow open access bits; clearing receive/transmit bits blocks delivery/transmission. Successful join invokes JOIN then recipient RECEIVE policies in the same order. See caveat below. |

The denied transmit callback's explicit trace is discarded by Rust when the command returns an error; C retains it. This is covered by approved transactional output rollback, not a missing lock evaluation. With LM02 corrected, the final re-add reaches the closed JOIN policy in both engines. The subsequent LM05 correction restores C's exact `Sorry, this channel type does not allow you to join.` denial, preserving native access checks and the trusted Lua add-player contract.

## Messaging controls and unresolved boundaries

- Explicit lock `other_message` text agrees in the tested room, including the actor prefix and actor/target exclusions. LM03's direct AUDIBLE route is now covered separately from neighbor routing.
- Player channel delivery, ordinary-player lock traces and denied-lock/open-flag behavior match on TCP. LM01 and LM02 now cover non-player channel routing and leave callbacks.
- Rust rolls back erroring policies and their output; C emits diagnostics and continues denial processing. Preserve approved callback-failure transactions; do not restore side effects merely to match C error handling. The raw traces retain both results.
- LM06 resolves the extra failure-event `subject` field: native failure events now omit it as C does. Lock checks still receive their correct subject, including the item for RECEIVE. Approved operation/source/destination extensions, movement causes and descriptor rules are unchanged; ordinary events are outside this narrowly scoped change.
- Exact public-Lua error-code identities, all missing/malformed return permutations, channel ordering with multiple/offline/HALTED thing members, and all nested audible containment combinations are not exhaustively paired here. Existing tests are evidence for selected contracts, not full C equivalence.

## Reproduction and validation

From the Rust directory, with the sibling C server built:

```sh
cargo build
python3 tests/tools/lock_message_probe.py > /tmp/lock-message-wire.json
cargo test --test lock_message_audit
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The optional probe reuses the existing incremental Telnet client, refuses optional negotiation, authenticates with the public fixture password and terminates/removes its temporary worlds. It never opens operator data. Current C tests run for this audit: lua_lock_checks, lua_command_access, speech_format, page_recipients, commac, lua_pcall_checked, lua_module_smoke and flag_privileges.

The validation record in [lock-message-audit.json](../tests/fixtures/lock-message-audit.json) links both the corrected and historical TCP transcripts so the fixed behavior and the evidence that motivated it remain reviewable.

Broader unverified cases remain explicit. No production edits, restart, or commit were performed by this correction.


## Follow-up corrections LM05–LM06

The user requested correction of the remaining differences. Two Sol Medium agents
handled the independent changes; the primary reviewed and validated their combined result.

- **LM05 — closed-channel JOIN denial:** native `addcom` now uses the exact C sentence.
  `tests/channel_join_parity.rs` checks the message, absence of new aliases/membership
  on denial, Wizard bypass and grant-lock behavior with closed access bits.
  Reference: C `communication/channel_aliases.c`, Rust `communication/membership.rs`.
- **LM06 — failure-event context:** `deny_action` and traversal/teleport failure
  dispatch remove `subject` from the event context only. Lock evaluation retains it.
  `tests/failure_event_parity.rs` covers ordinary and movement failures and the
  retained silence/transaction contracts. Reference: C `commands/action_messages.c`
  builds `LuaEventInvocation` without a subject; Rust `lua/actions.rs` and
  `lua/callbacks.rs` now preserve that distinction.

The original audit and LM01–LM04 corrected transcripts remain historical artifacts.
The [follow-up transcript](../tests/fixtures/lock-message-wire-followup.json) confirms
the closed JOIN wording and absent failure-event subjects in both engines, while
RECEIVE lock traces retain the item subject. Final validation: **326 Rust tests**,
eight C tests, formatting and Clippy passed. No general callback-context rewrite, production edits, migration, restart
or commit accompanies these corrections.
