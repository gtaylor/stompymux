# First BattleTech implementation

This is the current milestone summary and follow-up backlog. It supersedes the
open-ended completion gates and historical “remaining work” lists in the older
BattleTech audit documents. The goal of this milestone is a playable Rust
implementation for the supported units, with explicit limits; it is not complete
behavioral equivalence with the reference server.

## Delivered scope

The BattleTech runtime is native Rust 2024, with no C BattleTech bridge. It uses
the game assets in this repository. LuaJIT and SQLite remain dependencies. The
`btmux-khi` reference tree is unchanged.

Supported units are biped and quad Mechs, tracked, wheeled, hover and stationary
ground vehicles, and VTOLs. They have construction, placement, cockpit controls,
power, movement, sensors, targeting, live combat, damage and crew handling, and
saved runtime state. Maps, environmental hazards, artillery, mines, external
towing, native commands and Lua interfaces have implementations and tests.

Launcher admission, ammunition profiles, attack rolls, damage packets, feed
recovery and other anatomy-independent mechanics are shared. New unit support
should add state adapters instead of copying these rules. See
[implementation ownership](btech.md) for the existing boundaries.

MML-3/5/7/9 have functioning SRM and LRM combat, including cluster hits,
ammunition expenditure, interception and restart. The shipped Daishi-H template
has corrected flags. Neither requires a reference-server workaround.

## Boundaries operators should know

- Autopilots, repair/refit systems, naval units, aerospace/dropships, infantry and
  battle armor are deferred. Loading or unloading a unit into another unit and
  container cargo handling are also deferred. External towing is available.
- Template parsing does not imply simulation support. Construction validates the
  installed equipment and supported chassis. The latest recorded corpus audit
  accepted 1,299 of 1,302 biped/quad templates and all 269 ground/VTOL templates;
  PHX-HK2, STG-A5 and WSP-105 contain unsupported LAM conversion equipment.
  Those counts are historical construction evidence, not a fresh closeout audit
  or proof of every equipment combination's combat behavior.
- `status`, `critstatus` and `mechtype` can be inspected but cannot be written
  directly. Use implemented cockpit and administrative services for their
  supported transitions. The setter now explicitly reports this boundary.
  It does not silently accept a mask or partially change a unit.
- `mechdamage` replaces compact material state on all supported chassis. It is
  not a full combat snapshot or an undo operation: omitted material entries are
  restored. Interactions with towing, crew recovery and full reconstruction
  ordering still need dedicated acceptance. Prefer normal damage/gameplay
  services for live scenarios until that follow-up is complete.
- MML LRM ammunition combined with special-ammunition flags is unsupported.
- Compatibility and readiness markers that describe full parity remain
  conservative; do not infer that a parsed or registered object is simulated,
  or flip a marker merely because one scenario passes.

The exact 67-field inventory is in [unit-field writes](btech-field-writes.md).
The historical audits retain detailed findings and focused test evidence.

## Validation and running

Closeout baseline, September 13, 2026: `cargo test` exited successfully with
2,431 tests passed, zero failures and zero ignored tests across 312 result
targets (309 integration executables, library, binary and documentation).
The dedicated MML tests and the two-client duel are included.

After that baseline compiled, the final change made three already-rejected
administrative fields return an explicit unsupported-write error, added one
cross-chassis regression test and updated documentation/help. The library,
unit-field and help targets were rerun against that change: all 251 tests
passed. Thus 2,432 distinct tests have passing coverage across the baseline and
final focused run; this is not a claim of one uninterrupted full-suite run of
the final checkout. Local logs are `target/current-acceptance.log` and
`target/first-implementation-final-tests.log`.

Formatting and diff checks passed, and the production and fixture Lua
declarations match. `cargo clippy --all-targets -- -D warnings` passed, recorded in
`target/first-implementation-clippy.log`.

For a local game, follow the [README startup instructions](../README.md).
Use a separate game-directory copy for experiments and one server writer per
database. Wizard creation and placement commands and their Lua equivalents are
documented in [the BattleTech guide](btech.md) and in-game `help @btech`.

The automated two-client scenario exercises login, cockpit control, acquisition,
firing, destruction and restart:

```sh
cargo test --test btech_06 btech_duel::
cargo test --test btech_05 btech_mml::
```

These are regression checks, not a claim that every mixed-unit battle or
production workload has been tested. The remaining first-implementation blocker
policy is concrete: fix failures in supported construction, ordinary gameplay,
transaction rollback or persistence; schedule additional parity work below.

## Follow-up work

Priorities below order the next phase. An unverified interaction is an acceptance
gap, not an assertion that it is broken.

| Priority | Work | Completion evidence |
| --- | --- | --- |
| 1 | Finish `mechdamage` live-state acceptance: tow/carrier mass reconciliation, recovering crews, destruction/restoration and section-order reconstruction, including hardened-gyro piloting effects. | Mech/vehicle cases retain the correct independent state and timers, consume no premature dice, reject invalid edits atomically, and replay identically after restart. Start with [the damage audit](btech-damage-field-audit.md). |
| 1 | Define direct `status`, `critstatus` and `mechtype` transitions before adding setters. | Explicit supported transition matrix, shared validation, no masks that contradict material state, and native/Lua authorization, rollback and restart tests on every affected chassis. Keep current rejection until then. |
| 1 | Broaden mixed combat and lifecycle acceptance. | End-to-end Mech/ground/VTOL battles cover casualties, observer messages, tow partners, cleanup, interrupted actions and failed persistence commits. Extend existing duel/scenario tests instead of creating another combat harness. |
| 2 | Complete construction/equipment/configuration and command-argument parity audits. | Refresh the shipped-template audit; verify configuration consumers, equipment combinations and command admission against the intended behavior. Record deliberate differences and retain explicit rejection of unsupported equipment. |
| 2 | Add combined MML LRM special ammunition if desired. | One shared ammunition representation drives selection, matching bins, range, damage, guidance, interception, hazards, mass, reports and persistence across chassis. |
| 2 | Consolidate historical documentation and improve compatibility reporting. | Replace chronological claims with subsystem contracts and distinguish parsing, construction, playable simulation and full parity without changing runtime admission accidentally. |
| 3 | Measure sustained server load and large battles. | Reproducible workloads report tick latency, persistence cost and memory use; optimize measured problems without splitting shared mechanics or weakening transactions. |

The excluded systems and unit classes above need separate scope decisions.
They are not unfinished blockers for this milestone. Keep unit loading deferred
and keep the reference tree read-only in subsequent work.
