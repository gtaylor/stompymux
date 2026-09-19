# Lua parity ledger

The machine-readable ledger is
[`tests/fixtures/lua-api-contracts.json`](../tests/fixtures/lua-api-contracts.json).
It pins the C reference at `2bbbe6fcdbabe69e229d73089f44bf38f91c0591`
and the initial Rust audit baseline at
`7917a95642e2fe690bf0ac03a018a0e2b7342b21`. The older
[`lua-api.json`](../tests/fixtures/lua-api.json) remains the compatible,
callable-only MUX fixture; the companion ledger is the complete contract source.

## Inventory and evidence

The ledger contains 671 entries: 201 declared native callables, 9 directly
accessible registered metamethods, 265 typed constants, 29 stable errors, 42 package or catalog namespaces, 18 exports from
the three shipped Lua packages (including 12 methods on the table returned by
`testing.suite`), and 107 sandbox globals and standard-library
members. Native callable descriptions, parameters and return annotations come
from the pinned generated declarations and point back to their registering C
source. The generator also cross-checks all 112 BattleTech native-array and 89
MUX installer registrations and fails if one is absent from the callable inventory. Reviewed
contracts and test evidence live in `lua-api-contract-overrides.json`, so a
regeneration cannot erase their status. `tools/update_lua_contract_inventory.py`
regenerates the fixtures from that revision and deliberately requires the
sibling `btmux-khi` checkout.

`tests/lua_contract_inventory.rs` freezes revisions, entry kinds, uniqueness,
metadata requirements, and accepted-difference IDs. It resolves every entry
marked verified in an initialized Rust VM. Actionable entries may be absent or
contract-incompatible; presence alone never promotes them to verified. A status
should change to verified only when its `rust_evidence` names an executable test
covering parameters, returns, errors, and effects described by the entry.

The C reference's existing nine Lua and autopilot integration tests pass at the
pinned revision. Those tests establish a healthy reference build, but are not a
differential oracle for every ledger row. Shared probes must use temporary game
directories and compare exact return counts and shapes as well as state/output
effects. When the C runner cannot host a probe, source-backed expectations plus
an executable Rust regression are required and the evidence limitation remains
recorded.

## Initial supported gaps

All 45 MUX world/Object/State/Flags/Powers callables are verified by focused
persistence tests and an exact paired probe. The remaining MUX callables remain
actionable until their individual contracts have precise test evidence. BattleTech
callable work is grouped as follows:

**Final state (2026-09-19): every actionable entry is verified — 662 verified,
0 actionable, 9 prerequisite-blocked.** The MUX services (comsys, error, text,
session, telnet, config, top-level), all nine BattleTech subpackages including
the full 48-callable `btech.unit` surface, the 265 constants, the sandbox
inventory, and the shipped Lua packages (`testing`, `access_policy`,
`object_appearances`) carry paired differential-probe evidence plus focused
contract suites. Key C facts established during the port: the pinned part
registry rejects brand-zero names, so shipped templates are
`btech.template.invalid` through the template API and saved templates are
unloadable on both sides; `save_template` reproduces C's brand-zero artifact;
LuaJIT's shared-metatable `__eq` dispatch governs cross-family constant
comparisons; C's UTF-8 handling replaces each invalid byte with one U+FFFD
(width/strip) and truncates at the first invalid byte. Rust facade extension
groups beyond C's nine subpackages are classified in
`tests/fixtures/lua-probes/runtime_surface.allow`; any unclassified addition
fails the probe. `require('mux')` returning the global remains the documented
Rust package extension.

The nine prerequisite-blocked callables are unchanged and freshly re-reviewed
with source evidence: seven `btech.autopilot` order-queue callables (absent
BtechAutopilot runtime; special_registration.rs defers the type) and two
`btech.repair` scheduler-dependent callables (`needs`, `is_under_repair`;
absent FIRST_TECH_EVENT..LAST_TECH_EVENT queue and damage-table derivation).

| Package | Actionable | Blocked | Initial disposition |
|---|---:|---:|---|
| `btech.autopilot` | 0 | 7 | Needs the absent autopilot queue, association, modes, and event runtime. |
| `btech.character` | 0 | 0 | All 7 callables are implemented and verified against exact catalog, value, mutation, XP, error, and persistence contracts. |
| `btech.map` | 0 | 0 | All 16 callables are verified by the focused map contract suites. |
| `btech.parts` | 0 | 0 | All 8 callables have complete paired C/Rust catalogue and mutation evidence, plus persistence and checking-mode regressions. |
| `btech.player` | 6 | 0 | All 6 are implemented with persistence coverage; shared error-format acceptance remains open. |
| `btech.repair` | 1 | 2 | `is_fixable` and `technician_available_in` are verified. Immediate `apply` remains actionable for raw class sections and complete component restoration; `needs` and `is_under_repair` require the absent technician event queue. |
| `btech.system` | 2 | 0 | Both callables are implemented; explicit Lua-reload telemetry continuity acceptance remains open. |
| `btech.template` | 13 | 0 | Asset inspection and construction data exist; each C projection and zero-return display path needs an adapter. |
| `btech.unit` | 48 | 0 | Inspection and scalar editing largely have domain support; equipment rebuilding remains subject to implementation audit. |

The 265 constants are actionable independently of their parent operations.
All 45 object-type, command-access, flag, power, and lock constants are verified
for value, identity, equality, string, immutability, invalid lookup, and protected
metatable behavior. The remaining constant catalogs retain actionable status.

The sandbox inventory is pinned to the reference LuaJIT surface: it includes
`gcinfo`, `newproxy`, and `table.move`, and excludes the unavailable
`math.mod`. Error namespace packages are inventoried separately from their
plain-table dotted code nodes; existing fields are writable while `__newindex`
rejects new fields. `require('mux')` remains the documented baseline
Rust package extension; this is recorded as implementation evidence rather than
used to waive any C package mismatch.

## Frozen accepted differences

The ledger freezes eight pre-existing decisions and permits no implicit
exceptions: transaction-wide rollback (`LE06`/`LE08`), retained movement causes,
bounded atomic flow state (`LE09`), captured schedule registrations (`LE10`),
configured resource limits, session ownership of `CONNECTED`, text/configuration
extensions, and host/filesystem isolation. Every future mismatch must either be
removed or explicitly map to one of these IDs; adding another ID requires review.

An entry may be prerequisite-blocked only when its behavior depends on an absent
gameplay subsystem. Missing bindings, aliases, projections, persistence fields,
validation, computations, or instrumentation are actionable implementation
work. This rule is why constants, player configuration, event-lag measurement,
and scalar unit administration are not classified as blocked.
