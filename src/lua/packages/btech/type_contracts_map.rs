//! LuaLS contract blocks for the btech map surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00024
//|---Read the detached saved cargo location, or nil when the map has no location restriction.
//|---@param map integer
//|---@return BattleCargoTransferPoint|nil
//|function btech_map.cargo_point(map) end
// lua-types-end

// lua-types-begin btech 00025
//|---Wizard-only transfer-point configuration; nil clears the point. Coordinates must lie inside the map.
//|---@param actor integer
//|---@param map integer
//|---@param point BattleCargoTransferPoint|nil
//|function btech_map.set_cargo_point(actor, map, point) end
// lua-types-end

// lua-types-begin btech 00026
//|---Read source-map metadata without decoding saved terrain or applying overlays.
//|---@param name string Relative name under database.map_database.
//|---@return BattleMapAssetSummary
//|function btech_map.inspect_file(name) end
// lua-types-end

// lua-types-begin btech 00027
//|---Inspect a saved map identity without activating simulation.
//|---@param dbref integer
//|---@return StoredBattleMap
//|function btech_map.inspect(dbref) end
// lua-types-end

// lua-types-begin btech 00028
//|---Read a decoded tile; ambiguous maps and invalid coordinates raise an error.
//|---@param dbref integer
//|---@param x integer Zero-based column.
//|---@param y integer Zero-based row.
//|---@return BattleHex
//|function btech_map.hex(dbref, x, y) end
// lua-types-end

// lua-types-begin btech 00029
//|---Register an existing room or thing using a source asset. Transactional.
//|---@param dbref integer
//|---@param name string Relative name under database.map_database.
//|---@return boolean
//|function btech_map.create(dbref, name) end
// lua-types-end

// lua-types-begin btech 00030
//|---Explicitly replace terrain on an unoccupied map of the same dimensions.
//|---@param dbref integer
//|---@param name string Relative name under database.map_database.
//|---@return boolean
//|function btech_map.reload(dbref, name) end
// lua-types-end

// lua-types-begin btech 00099
//|---Change saved light/weather conditions without reloading occupied terrain.
//|---Perception follows the new light and visibility on the next scan; contacts and locks remain until then.
//|---@param dbref integer Map object dbref.
//|---@param light BattleLightLevel Typed constant from btech.map.light_levels.
//|---@param visibility integer Weather range from 0 through 60.
//|---@return boolean
//|function btech_map.conditions(dbref, light, visibility) end
// lua-types-end

// lua-types-begin btech 00100
//|---Wizard-only persisted cloud boundary. Zero disables it; accepts signed 16-bit elevation levels.
//|---@param actor integer
//|---@param dbref integer
//|---@param altitude integer
//|---@return integer altitude
//|function btech_map.cloud_base(actor, dbref, altitude) end
// lua-types-end

// lua-types-begin btech 00103
//|---Wizard broadcast to occupants of running, conscious units in map-slot order.
//|---Does not require sensor contacts; all notices and the private confirmation roll back together.
//|---Rust extension retained under its descriptive name; the canonical emit follows the C contract.
//|---@param actor integer
//|---@param dbref integer
//|---@param text string Leading spaces are removed; empty messages are rejected.
//|---@return integer[] Eligible unit dbrefs, including units with empty cockpits.
//|function btech_map.emit_as(actor, dbref, text) end
// lua-types-end

// lua-types-begin btech 00105
//|---Deliver a cockpit message to occupants of running units using the shared transactional emitter.
//|---@param map DbRef|Object
//|---@param message string One through 8191 bytes; leading spaces are removed.
//|---@param options? BattleMapEmitOptions
//|function btech_map.emit(map, message, options) end
// lua-types-end

// lua-types-begin btech 00106
//|---Wizard-only shutdown and removal in map-slot order; game objects stay in the map room.
//|---@param actor integer
//|---@param dbref integer
//|---@return integer[] Removed unit dbrefs.
//|function btech_map.clear_units(actor, dbref) end
// lua-types-end

// lua-types-begin btech 00107
//|---Wizard-only resize; copies overlapping visible tiles, clears map objects and rejects clipped units.
//|---@param actor integer
//|---@param dbref integer
//|---@param width integer 1 through 1000
//|---@param height integer 1 through 1000
//|---@return boolean
//|function btech_map.resize(actor, dbref, width, height) end
// lua-types-end

// lua-types-begin btech 00108
//|---Stage an atomic asset replacement after world commit; true means queued, not written.
//|---@param actor integer Wizard receiving the completion or failure notice.
//|---@param dbref integer
//|---@param name string Relative name inside the configured map directory.
//|---@return boolean
//|function btech_map.save(actor, dbref, name) end
// lua-types-end

// lua-types-begin btech 00109
//|---Load an asset; GOD keeps membership, while other wizards shut down and clear units.
//|---Rust extension retained under its descriptive name; the canonical loader follows the C contract.
//|---@param actor integer
//|---@param dbref integer
//|---@param name string Relative asset name.
//|---@return boolean
//|function btech_map.load_as(actor, dbref, name) end
// lua-types-end

// lua-types-begin btech 00110
//|---Replace map terrain from a saved asset using the strict C contract.
//|---@param map DbRef|Object
//|---@param name string Relative name under database.map_database.
//|function btech_map.load(map, name) end
// lua-types-end

// lua-types-begin btech 00111
//|---Install wizard fire; zero duration is permanent. Off-map coordinates leave the map unchanged.
//|---@param actor integer
//|---@param dbref integer
//|---@param x integer
//|---@param y integer
//|---@param duration integer Signed seconds; fire keeps a signed-short spread budget, smoke uses at least one tick.
//|---@return boolean
//|function btech_map.add_fire(actor, dbref, x, y, duration) end
// lua-types-end

// lua-types-begin btech 00112
//|---Add a newest-first mine owned by the wizard; return its persistent record slot.
//|---@param actor integer
//|---@param map integer
//|---@param x integer
//|---@param y integer
//|---@param kind 'standard'|'inferno'|'command'|'vibra'|'trigger'
//|---@param strength integer
//|---@param extra? integer
//|---@return integer
//|function btech_map.add_mine(actor, map, x, y, kind, strength, extra) end
// lua-types-end

// lua-types-begin btech 00113
//|---Publish a labelled terrain-only map using the wizard's saved display preferences.
//|---@param actor integer
//|---@param map integer
//|---@param x integer
//|---@param y integer
//|---@return table report Clipped viewport and styled text; maximum_range is zero without a scanner.
//|function btech_map.view(actor, map, x, y) end
// lua-types-end

// lua-types-begin btech 00114
//|---Edit an exact named map field as a wizard; shared controls publish any cockpit consequences.
//|---@param actor integer
//|---@param map integer
//|---@param field string
//|---@param value string
//|function btech_map.set_field(actor, map, field, value) end
// lua-types-end

// lua-types-begin btech 00115
//|---Install wizard smoke; zero duration is permanent. Off-map coordinates leave the map unchanged.
//|---@param actor integer
//|---@param dbref integer
//|---@param x integer
//|---@param y integer
//|---@param duration integer Signed seconds; fire keeps a signed-short spread budget, smoke uses at least one tick.
//|---@return boolean
//|function btech_map.add_smoke(actor, dbref, x, y, duration) end
// lua-types-end

// lua-types-begin btech 00117
//|---Read the authored link configuration saved by the wizard editor. Rust extension retained
//|---under its descriptive name; the canonical link follows the C contract.
//|---@param child integer
//|---@return BattleAuthoredMapLink|nil
//|function btech_map.authored_link(child) end
// lua-types-end

// lua-types-begin btech 00118
//|---Configure an authored link without rebuilding live routes; nil removes the configuration.
//|---Rust extension retained under its descriptive name; the canonical setter follows the C contract.
//|---@param child integer
//|---@param link BattleAuthoredMapLink|nil
//|---@return boolean
//|function btech_map.set_authored_link(child, link) end
// lua-types-end

// lua-types-begin btech 00122
//|---Read the strict C-contract link configuration of a child map, or nil when none is authored.
//|---@param child DbRef|Object
//|---@return BattleMapLink|nil
//|function btech_map.link(child) end
// lua-types-end

// lua-types-begin btech 00123
//|---Replace the C-contract link configuration of a child map; nil removes it.
//|---@param child DbRef|Object
//|---@param link BattleMapLink|nil
//|function btech_map.set_link(child, link) end
// lua-types-end

// lua-types-begin btech 00124
//|---Rebuild reachable map routes with cycle/depth protection and atomic publication.
//|---Rust extension retained under its descriptive name; the canonical rebuild follows the C contract.
//|---@param actor integer
//|---@param map integer
//|---@return {buildings:integer,leaves:integer,entrances:integer,skipped:integer}
//|function btech_map.update_links_as(actor, map) end
// lua-types-end

// lua-types-begin btech 00125
//|---Rebuild reachable map routes with cycle/depth protection and atomic publication.
//|---@param map DbRef|Object
//|function btech_map.update_links(map) end
// lua-types-end

// lua-types-begin btech 00126
//|---Publish a wizard map listing without advancing simulation or changing contacts.
//|---@param actor integer
//|---@param dbref integer
//|---@param target string MECHS or OBJS; complete case-insensitive name required.
//|---@return boolean
//|function btech_map.list(actor, dbref, target) end
// lua-types-end

// lua-types-begin btech 00127
//|---Delete map objects by type, coordinate, or both. At least one selector is required.
//|---@param actor integer
//|---@param dbref integer
//|---@param kind? string FIRE, SMOKE, DECO, MINE, BUILDING, LEAVE, ENTRA, LINKED, or BLZ; prefixes accepted.
//|---@param x? integer Must be paired with y.
//|---@param y? integer Must be paired with x.
//|---@return integer Number of selected records deleted; reciprocal cleanup is not counted.
//|function btech_map.delete_objects(actor, dbref, kind, x, y) end
// lua-types-end

// lua-types-begin btech 00128
//|---Add a wizard-owned circular landing restriction; negative radii block nothing.
//|---@param actor integer
//|---@param dbref integer
//|---@param x integer
//|---@param y integer
//|---@param radius integer
//|---@param team? integer Zero means no exemption.
//|---@return integer Restriction slot.
//|function btech_map.add_block(actor, dbref, x, y, radius, team) end
// lua-types-end

// lua-types-begin btech 00130
//|---Wizard live base-terrain edit. Retains unit altitude and overlays; does not cause combat falls.
//|---@param actor integer
//|---@param dbref integer
//|---@param x integer
//|---@param y integer
//|---@param terrain string Canonical terrain symbol; a leading dot selects grassland.
//|---@param elevation integer Absolute magnitude capped at nine.
//|---@return BattleMapHexChange
//|function btech_map.set_hex(actor, dbref, x, y, terrain, elevation) end
// lua-types-end

// lua-types-begin btech 00132
//|---Wizard seasonal growth. Only water can freeze; new ice does not extend this pass's shoreline.
//|---@param actor integer
//|---@param dbref integer
//|---@param percentage integer Signed percentage threshold; outside 0–100 means never/always.
//|---@return BattleMapIceReport
//|function btech_map.add_ice(actor, dbref, percentage) end
// lua-types-end

// lua-types-begin btech 00133
//|---Wizard seasonal melting. Falls, flooding, casualties and notices commit with the terrain.
//|---@param actor integer
//|---@param dbref integer
//|---@param percentage integer
//|---@return BattleMapIceReport
//|function btech_map.remove_ice(actor, dbref, percentage) end
// lua-types-end

// lua-types-begin btech 00135
//|---Wizard SETCOND action; updates live map rules without advancing time or resetting units.
//|---@param actor integer
//|---@param dbref integer
//|---@param conditions BattleMapEnvironment
//|---@return BattleMapEnvironment Actual resulting state, including retained underground status.
//|function btech_map.environment(actor, dbref, conditions) end
// lua-types-end

// lua-types-begin btech 00136
//|---Enable or disable saved opposite-edge wrapping.
//|---@param dbref integer
//|---@param enabled boolean
//|---@return boolean
//|function btech_map.wrapping(dbref, enabled) end
// lua-types-end

// lua-types-begin btech 00138
//|---List saved artillery blast zones in saved order.
//|---@param map DbRef|Object
//|---@return BattleBlastZone[]
//|function btech_map.blast_zones(map) end
// lua-types-end

// lua-types-begin btech 00139
//|---Read the saved cargo transfer point, or nil when the map has no location restriction.
//|---@param map DbRef|Object
//|---@return BattleCargoTransferPoint|nil
//|function btech_map.cargo_transfer_point(map) end
// lua-types-end

// lua-types-begin btech 00140
//|---Replace the saved cargo transfer point; nil clears the restriction.
//|---@param map DbRef|Object
//|---@param point BattleCargoTransferPoint|nil
//|function btech_map.set_cargo_transfer_point(map, point) end
// lua-types-end

// lua-types-begin btech 00141
//|---Read one tile elevation; water and ice report depth.
//|---@param map DbRef|Object
//|---@param hex BattleHexCoordinate
//|---@return integer elevation
//|function btech_map.elevation(map, hex) end
// lua-types-end

// lua-types-begin btech 00143
//|---Read one decoded terrain kind.
//|---@param map DbRef|Object
//|---@param hex BattleHexCoordinate
//|---@return BattleTerrainName terrain
//|function btech_map.terrain(map, hex) end
// lua-types-end

// lua-types-begin btech 00144
//|---Report whether a coordinate lies inside a saved blast zone.
//|---@param map DbRef|Object
//|---@param hex BattleHexCoordinate
//|---@return boolean inside
//|function btech_map.in_blast_zone(map, hex) end
// lua-types-end

// lua-types-begin btech 00146
//|---Report line of sight from one placed unit toward a unit or hex.
//|---@param observer DbRef|Object
//|---@param target DbRef|Object|BattleHexCoordinate
//|---@return BattleLineOfSight state
//|function btech_map.line_of_sight(observer, target) end
// lua-types-end

// lua-types-begin btech 00148
//|---Place a unit on decoded terrain using the shared placement rules.
//|---@param unit DbRef|Object
//|---@param map DbRef|Object
//|---@param position BattlePlacement
//|function btech_map.place_unit(unit, map, position) end
// lua-types-end

// lua-types-begin btech 00149
//|---Measure the spatial range between two units or positions on one map.
//|---@param map DbRef|Object
//|---@param from DbRef|Object|BattlePlacement
//|---@param to DbRef|Object|BattlePlacement
//|---@return number range
//|function btech_map.range(map, from, to) end
// lua-types-end

// lua-types-begin btech 00150
//|---Resolve the first unit matching a two-character battlefield ID from a unit or map origin.
//|---@param origin DbRef|Object Registered unit or map.
//|---@param id string Exactly two ASCII characters.
//|---@return Object|nil unit
//|function btech_map.unit_by_id(origin, id) end
// lua-types-end

// lua-types-begin btech 00152
//|---List units placed on a map in saved slot order; an optional filter omits distant units.
//|---@param map DbRef|Object
//|---@param filter? BattleMapUnitFilter
//|---@return Object[] units
//|function btech_map.units(map, filter) end
// lua-types-end

// lua-types-begin btech 00412
//|---Check map membership and world invariants without changing placements or unit state.
//|---@param actor integer Wizard performing the check.
//|---@param map integer
//|---@return table report Map ID and units in persisted slot order.
//|function btech_map.check(actor, map) end
// lua-types-end

// lua-types-begin btech 00413
//|---Publish and return a wizard's map field report in catalogue order.
//|---@param actor integer
//|---@param map integer
//|---@param arguments? string Optional leading 1 or 4 selects columns, followed by a field prefix.
//|---@return table report Map, columns, fields (name/value) and literal text. firstfree has no value.
//|function btech_map.fields(actor, map, arguments) end
// lua-types-end
