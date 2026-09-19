# BattleTech Lua constants and parts contract

This note records the contract implemented from the C package sources. Rust tests pin the
source-derived values, and isolated paired probes compare the C and Rust servers against cloned
game state.

## Typed constants

`btech.error.codes` is the canonical error-code namespace. `btech.errors` remains an extension and
refers to the same values. Every BattleTech constant is immutable userdata. Equality requires the
same catalogue and native value, `tostring` returns the declared name, and namespace lookup rejects
unknown or non-string keys with `mux.arg.invalid` and `detail.argument = 2`.

| Namespace | Entries | Native representation |
| --- | ---: | --- |
| `btech.unit.types` | 9 | C unit type ordinal |
| `btech.unit.movement_types` | 11 | C movement ordinal |
| `btech.unit.sections` | 36 | C section ordinal |
| `btech.unit.technology` | 67 | C technology bit value |
| `btech.unit.technology_groups` | 3 | C group ordinal |
| `btech.unit.fire_modes` | 21 | C fire-mode bit value |
| `btech.unit.ammunition_modes` | 23 | C ammunition-mode bit value |
| `btech.autopilot.orders` | 17 | C order ordinal |
| `btech.autopilot.directions` | 4 | C direction ordinal |
| `btech.autopilot.roam_modes` | 2 | C roam-mode ordinal |
| `btech.autopilot.autogun_modes` | 3 | C autogun-mode ordinal |
| `btech.repair.operations` | 21 | C repair-operation ordinal |

The 217 names and values come from `btech_constants.c`, `btech_unit_constants.c`, the autopilot
constant sources, and the repair operation source. `lua_btech_constants.rs` checks the complete
name inventory in live and checking runtimes, catalogue isolation, metatable protection, lookup
errors, and the distinct values of similarly named ammunition modes.

## Part records and references

A returned part is a detached table with `id`, `brand`, `packed_id`, `short_name`, `long_name`,
`very_long_name`, `category`, `weight_tons`, and `cost`. Weapon parts also contain `weapon`, whose
fields are `kind`, `heat`, `damage`, `minimum_range`, `short_range`, `medium_range`, `long_range`,
`critical_slots`, `ammunition_per_ton`, `recycle_time`, and `battle_value`.

Inputs accept a Lua number containing an integral packed ID from zero through `INT_MAX`, an exact
ASCII-case-insensitive short/long/very-long name, or a table with numeric `id` and `brand` fields.
Table projections may contain extra fields. Packed identities use `brand * 1024 + id`. The C
registry exposes 490 weapon forms at brands one through five; brand zero is not registered because
C's `create_brandname` rejects it. Missing registrations resolve to nil. Raw installed-part
projections can still describe a brand-zero identity, omitting unavailable name fields. APIs requiring a part
raise `btech.part.not_found`; a name shared by distinct identities raises `btech.part.ambiguous`.
Both errors include the public argument number. As in the C API, string comparisons stop at the
first NUL byte.

## `btech.parts` symbols

| Symbol | Inputs and defaults | Return | Errors, effects, and prerequisites |
| --- | --- | --- | --- |
| `categories()` | Extra arguments ignored | Six `{code,name}` rows in canonical order | No world mutation |
| `list(category?)` | Nil lists all; category is case-insensitive | Catalogue-ordered part records | Unknown/non-string category is `mux.arg.invalid` at argument 1 |
| `search(query)` | Non-empty byte string; `*`, `?`, and backslash escaping use C quick-wild semantics | Catalogue-ordered matching records | Missing, non-string, or NUL-leading query is `mux.arg.invalid` at argument 1 |
| `resolve(part)` | Packed ID, exact name, or record | Part record or nil | Invalid shape/range is `mux.arg.invalid`; ambiguity is `btech.part.ambiguous` |
| `stores(object)` | Live dbref/Object, excluding GOING objects | Positive registered stock rows `{part,quantity}` | Object errors use `mux.object.invalid` or `mux.object.unavailable` |
| `store_quantity(object, part)` | Live object and registered part | Integer quantity, zero when absent | Missing part is `btech.part.not_found` at argument 2 |
| `adjust_stores(object, part, delta)` | Delta is a nonzero integral Lua number in the C `int` range | No values | Applies one low-level stock edit without load reconciliation or notices. Under/overflow raises `btech.operation.failed` with `detail.reason = store_capacity_exceeded`; edit failure uses `store_commit_failed`. Host callback rollback restores failed transactions. |
| `set_cost(part, cost)` | Cost is an integral Lua number from zero through `2^53-1` | No values | Cost is shared across brands and persisted by canonical C item name. Invalid cost is `mux.arg.invalid` at argument 2. |

All eight functions are installed through the private native dispatch table. Checking mode replaces
that table, so every call raises `mux.unavailable.checking` before reading or changing live state.

## Evidence

The behavior source is `mux/lua/packages/btech/parts/btech_parts_bindings.c`, with catalogue facts
from `unit/weapons_catalogue.c`, `unit/weapons_vrt.h`, equipment category definitions, and part-name
registry ordering. `lua_btech_parts_contracts.rs` audits all 178 C weapon definitions, including
personal-combat and infantry rows, against the returned kind and every numeric weapon field. It
also checks the 490 registered branded projections, ordering, category partition, all accepted
reference forms, ambiguity, first-NUL behavior, packed-brand overflow, store changes, rollback,
safe-integer cost bounds, brand-shared cost, persistence reload, and checking restrictions. The
paired catalogue probe compares complete typed records, while the paired mutation probe covers
store capacity boundaries, zero-row omission, exact no-value returns, structured failures, and
brand-shared costs.
