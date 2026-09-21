# Map lookup object behavior

TBITS now has an owned sparse-row implementation, persistence and startup mine
reconciliation. This inventory records the reference contract and the remaining
acceptance work; it does not claim complete caller coverage.

## Representation and lifetime

`map/map_bits.c` stores two bits per hex, packed four hexes per byte: mine=1,
hangar=2. Rows are allocated independently. The information object can exist
without any allocated rows, and allocated rows remain present after their last
bit clears. The Rust representation uses an optional sparse row collection:
absence represents no object, an empty collection represents an allocated object
with no rows, and zero-filled rows preserve their allocation. Byte packing is
needed at the database boundary; raw pointers and a C template interpreter are
not needed.

| Operation | Reference effect |
| --- | --- |
| Set mine/hangar bit | Allocate object and row if absent, then set bit. |
| Unset individual bit | Allocate object if absent, clear only if row exists. |
| Clear mine/hangar bits | Do nothing if object absent; preserve existing rows and object. |
| Read a bit | Return false for absent object or row; never allocate. |
| Delete TBITS | Remove object and rows, retaining mine/building definitions. |
| LIST OBJS | Emit `--- MAP/HANGAR INFORMATION OBJECT ---` once for the object. |
| SETMAPSIZE | Reject an allocated object with `Invalid map for size change, sorry.` before parsing dimensions. |

Generic object removal is gated by `MAPFLAG_MAPO`. Both ordinary object insertion
and TBITS allocation set this flag. `map/map_obj.c` removes reciprocal interior
return links when deleting a building object but does not clear its hangar bit.
The declared `unset_hex_enterable` has no callers in the inspected reference.

## Rebuild callers and consumers

`combat/mine.c::mine_fields_recalculate` clears mine bits, preserving rows and
hangar bits, then rebuilds mine coverage from definitions. Standard/command mines
set their coordinate; vibrating and trigger mines use their existing distinct
coverage geometry. Rust shares the geometry in `mine_activation.rs` between rebuild and activation.

Recalculation occurs after adding mines, after completed explosive mine effects,
and after relevant DELOBJ operations. Type-only `DELOBJ MINE` requests it even
when no mines were found; coordinate deletion requests it when a mine matched.
Consequently, deleting TBITS disables mine triggering only until a subsequent
rebuild. Bulk map-object deletion can remove the cache entirely.

The adding-mine case above is the operator `mine_command_add` path, not every
insertion. `artillery.c` calls `mine_field_add`, which inserts a standard mine
without recalculating or setting cache bits. It skips an existing coordinate
only if that hex's mine bit was already set. Thus artillery deposition on an
uncached hex can leave a field inactive until later recalculation, and repeated
deposition can add duplicate fields while that bit remains clear. Rust's `artillery_impact.rs::deposit_mine` now uses that cache-dependent duplicate
gate and shared insertion without rebuilding. Operator creation explicitly
requests rebuilding. Integration tests cover repeated uncached deposition,
persistence, later rebuild and duplicate suppression once coverage exists.

`map/map_links.c::recursively_update_links` clears hangar bits, then marks valid authored
building links while rebuilding entrance objects. Clearing does not create an
absent cache; a valid marked link does. Deleting a building by itself does not
perform this rebuild.

The production `is_hangar_hex` consumer is
`map/map_buildings.c::steppable_base_check`. It gates the structure-name/CF
notice before looking up the entrance, checking DropShip structure type and
hidden-building perception. It does **not** gate every entrance lookup or
building interaction. Putting this test into Rust's general `building_at`
accessor would suppress unrelated behavior. The corresponding Rust consumer is
the shared structure-entry notice service.

Mine activation uses `is_mine_hex` before altitude and field selection. Other
mine-bit callers include the duplicate-coordinate gate when creating a mine,
and display/removal admission; the terrain-clearing removal
function itself remains intentionally disabled. Each consumer needs its own
call-site comparison rather than a blanket replacement of all mine queries.

## Persistence and startup are separate stages

`persistence/map_restore.c::btech_special_load_map_bits` reads rows in
map/y/byte-index order. It rejects missing maps, invalid coordinates or byte
values, and incomplete/noncontiguous rows. One row contains exactly
`ceil(width / 4)` bytes, including padding bits. Allocation is recreated from
the first stored row. `persistence/snapshot_store.c` writes allocated rows,
including their zero bytes; an object with no rows has no independent saved
record. Ordinary object serialization skips TYPE_BITS.

After special-state loading, `special/registry_loading.c::load_update3` calls
`mine_fields_recalculate` for every map. Thus startup clears stale saved mine
bits and recreates coverage from surviving definitions, even when no TBITS rows
were saved. Hangar bits are not rebuilt by that pass. The earlier shorthand that
an empty allocation disappears on restart applies only to allocation restoration;
later mine rebuilding can recreate it. Inspection and server startup must not be
conflated when adapting this behavior to Rust's read-only database loader.

## Implementation acceptance

The model, shared update operations, persistence, startup reconciliation,
deletion, resize gate and exact listing are implemented. Owned byte persistence
replaces blanket cache invalidation. Terrain reload retains the lookup object
and its map-object flag; saves preserve independent lanes and unused padding.

Tests need absent/empty/zero-row states; independent mine/hangar masks; row
packing and malformed imports; cache deletion with surviving definitions;
rebuilding after deletion; startup mine reconciliation with retained hangar bits;
base notices without globally suppressing building lookup; resize refusal
precedence; native/Lua output; restart and callback rollback. Artillery tests also
need clear versus set cache bits, duplicate coordinates and activation after
later rebuilding. Coordinate deletion
of newly allocated TBITS has an additional reference limitation: its scalar
coordinate members are not initialized by `grab_us_an_array`, whereas restored
objects are zero-initialized. Rust uses canonical `(0,0)` coordinates for both new and restored lookup
objects. This defines the uninitialized reference case rather than claiming an
exact match for undefined behavior.


## Current verification

The focused run in `target/audit-map-bits-revised-final.log` passes 30 integration
tests across map bits, listing, object deletion, resizing, terrain persistence and
building step notices. Two lookup unit tests pass in
`target/audit-map-bits-revised.log`; its integration targets were filtered out and
are not counted. Tests retain callback/database rollback checks. Building notices
are suppressed after TBITS deletion for all five mobile ground chassis, while
ordinary entrance lookup survives, including read-only save/load replay.

The next consumer run in `target/audit-map-bits-consumers.log` passes 12 tests.
UPDATE LINKS now explicitly verifies stale-hangar clearing with retained zero
rows, native/Lua agreement, rollback and restart. Coordinate-wide deletion checks
that removing both a mine and TBITS precedes rebuilding the remaining mine's
coverage. Across all seven supported chassis, deletion suppresses mine selection
and scan detection without consuming dice or removing definitions; an explicit
mine edit restores both. Read-only persistence replay agrees.

The broader map/artillery/mine/surface/scan run passed 164 tests and exposed one
stale artillery duplicate expectation (`target/audit-map-bits-broad.log`). After
updating that test to require both uncached duplication and post-rebuild
suppression, all four ADDMINE tests pass in
`target/audit-map-bits-artillery-order.log`. Together these verify the 165 tests
in those twelve targets. The full run in `target/audit-map-bits-full.log` completed with 2,647 passed,
two failed and none ignored across 348 targets. The failures were a building-exit
fixture overwriting the retained map-object flag and a kick counter test omitting
critical-induced falls. Both corrections now pass focused verification: all 15 building-entry action
tests in `target/audit-building-exit-flags.log`, and the kick test's 32-seed,
five-case matrix in `target/audit-kick-counter-cascade.log`. Formatting passes.
The full run itself remains recorded as a two-failure baseline; it was not rerun
after these test-only corrections. The older 2,641-test
baseline predates this implementation.
