//! LuaLS contract blocks for the btech gunner surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00421
//|---Register a new station for a supported constructed parent. Wizard only.
//|---@param actor integer
//|---@param station integer
//|---@param parent integer
//|---@param arcs integer
//|---@return boolean
//|function btech_gunner.register(actor, station, parent, arcs) end
// lua-types-end

// lua-types-begin btech 00422
//|---Claim an available station while physically inside it.
//|---@param actor integer
//|---@param station integer
//|---@return boolean
//|function btech_gunner.initialize(actor, station) end
// lua-types-end

// lua-types-begin btech 00423
//|---Release the current actor's station assignment.
//|---@param actor integer
//|---@param station integer
//|---@return boolean
//|function btech_gunner.deinitialize(actor, station) end
// lua-types-end

// lua-types-begin btech 00424
//|---Return detached saved station fields, or nil when absent. Does not grant combat access.
//|---@param station integer
//|---@return table|nil
//|function btech_gunner.state(station) end
// lua-types-end

// lua-types-begin btech 00425
//|---Select an acquired parent contact, or clear with nil. Requires the present registered gunner.
//|---@param station integer
//|---@param gunner integer
//|---@param target integer|nil
//|---@return boolean
//|function btech_gunner.lock(station, gunner, target) end
// lua-types-end

// lua-types-begin btech 00426
//|---Select coordinates using shared sensor-lock rules and H/B/I/C purpose modes.
//|---@param station integer
//|---@param gunner integer
//|---@param x integer
//|---@param y integer
//|---@param mode string|nil
//|---@return boolean
//|function btech_gunner.lock_hex(station, gunner, x, y, mode) end
// lua-types-end

// lua-types-begin btech 00427
//|---Read the current registered gunner's conventional skill target for a parent weapon.
//|---Uses configured extended gunnery; does not check weapon readiness or fire.
//|---@param station integer
//|---@param gunner integer
//|---@param weapon integer Zero-based parent weapon index.
//|---@return integer
//|function btech_gunner.gunnery(station, gunner, weapon) end
// lua-types-end

// lua-types-begin btech 00428
//|---Read the registered gunner's dedicated artillery skill target without firing.
//|---@param station integer
//|---@param gunner integer
//|---@return integer
//|function btech_gunner.artillery_gunnery(station, gunner) end
// lua-types-end

// lua-types-begin btech 00429
//|---Preview conventional aim using independent station targeting and the registered gunner's skill.
//|---Read-only; does not check launch authority, weapon readiness or station arc eligibility.
//|---@param station integer
//|---@param gunner integer
//|---@param weapon integer Zero-based parent weapon index.
//|---@param target integer|table|nil Unit ID, coordinates {x,y}, or the station's selection.
//|---@return table report Parent, optional target/coordinate, and shared aim breakdown.
//|function btech_gunner.aim(station, gunner, weapon, target) end
// lua-types-end

// lua-types-begin btech 00430
//|---Fire a parent weapon at a unit or coordinate using the registered station operator and independent selection.
//|---Uses shared launch/damage and transactional publication, including delayed artillery and station-owned correction.
//|---@param station integer
//|---@param gunner integer
//|---@param weapon integer Zero-based parent weapon index.
//|---@param target integer|table|nil Unit ID, coordinates {x,y}, or station selection.
//|---@return table report Shared physical-unit firing report.
//|function btech_gunner.fire(station, gunner, weapon, target) end
// lua-types-end

// lua-types-begin btech 00431
//|---Sight a parent weapon using the registered station gunner's selection, skill and arcs.
//|---Supports conventional, terrain and artillery aim; consumes only preparation/attack dice.
//|---Requires running controls but permits weapons hold, recycling and empty ammunition.
//|---@param station integer
//|---@param gunner integer
//|---@param weapon integer Zero-based parent weapon index.
//|---@param target integer|BattleHexCoordinate|nil Omitted target uses station selection and parent observer link.
//|---@return BattleSightReport
//|function btech_gunner.sight(station, gunner, weapon, target) end
// lua-types-end

// lua-types-begin btech 00432
//|---Read compass bearing using the station selection and parent geometry.
//|---@param station integer
//|---@param gunner integer
//|---@param coordinates string? Same coordinate grammar as the corresponding unit report.
//|---@return BattleBearingReport
//|function btech_gunner.bearing(station, gunner, coordinates) end
// lua-types-end

// lua-types-begin btech 00433
//|---Read spatial range using the station selection, parent altitude and map darkness.
//|---@param station integer
//|---@param gunner integer
//|---@param coordinates string? Same coordinate grammar as the corresponding unit report.
//|---@return BattleRangeReport
//|function btech_gunner.range_report(station, gunner, coordinates) end
// lua-types-end

// lua-types-begin btech 00434
//|---Read range, bearing and vertical angle using station targeting.
//|---@param station integer
//|---@param gunner integer
//|---@param coordinates string? Same coordinate grammar as the corresponding unit report.
//|---@return BattleVectorReport
//|function btech_gunner.vector(station, gunner, coordinates) end
// lua-types-end

// lua-types-begin btech 00435
//|---Estimate travel using parent speed and the station hex selection; notify station occupants.
//|---@param station integer
//|---@param gunner integer
//|---@param coordinates string? Same coordinate grammar as the corresponding unit report.
//|---@return BattleEtaReport
//|function btech_gunner.eta(station, gunner, coordinates) end
// lua-types-end

// lua-types-begin btech 00436
//|---Measure the parent position relative to its current hex center.
//|---@param station integer
//|---@param gunner integer
//|---@return BattleHexCenterReport
//|function btech_gunner.findcenter(station, gunner) end
// lua-types-end

// lua-types-begin btech 00437
//|---Render the parent's tactical map using the registered gunner's display preferences.
//|---@param station integer
//|---@param gunner integer
//|---@param arguments string? Tactical mode and center grammar shared with unit.tactical.
//|---@return BattleTacticalMap
//|function btech_gunner.tactical(station, gunner, arguments) end
// lua-types-end

// lua-types-begin btech 00438
//|---Render the parent's long-range map using the registered gunner's display preferences.
//|---@param station integer
//|---@param gunner integer
//|---@param mode string Terrain/elevation/unit/visibility mode shared with unit.lrsmap.
//|---@param arguments string? Contact or bearing/range center; omitted center follows the parent.
//|---@return BattleLongRangeMap
//|function btech_gunner.lrsmap(station, gunner, mode, arguments) end
// lua-types-end

// lua-types-begin btech 00439
//|---Render local parent navigation, retaining the own-hex exception for failed scanner hardware.
//|---@param station integer
//|---@param gunner integer
//|---@param arguments string? Contact or bearing/range center; omitted center follows the parent.
//|---@return BattleNavigationReport
//|function btech_gunner.navigate(station, gunner, arguments) end
// lua-types-end

// lua-types-begin btech 00440
//|---Inspect a visible unit with parent sensors; scan warnings identify the physical parent.
//|---@param station integer
//|---@param gunner integer
//|---@param target integer
//|---@param options string? A/I/W report sections.
//|---@return string
//|function btech_gunner.scan(station, gunner, target, options) end
// lua-types-end

// lua-types-begin btech 00441
//|---Inspect parent state with the registered gunner's independent target selection.
//|---@param station integer
//|---@param gunner integer
//|---@param options? string
//|---@return string
//|function btech_gunner.status(station, gunner, options) end
// lua-types-end

// lua-types-begin btech 00442
//|---Render contacts using parent visibility, gunner preferences and independent selection.
//|---@param station integer
//|---@param gunner integer
//|---@param options? string Contact options, + for saved preferences, or #unit.
//|---@return string
//|function btech_gunner.contacts(station, gunner, options) end
// lua-types-end

// lua-types-begin btech 00443
//|---Wizard-only atomic station field edit; references use decimal numbers, coordinates clamp to signed shorts.
//|---@param actor integer
//|---@param station integer
//|---@param field string arcs/parent/gunner/target/targx/targy/targz/lockmode
//|---@param value string
//|function btech_gunner.set_field(actor, station, field, value) end
// lua-types-end

// lua-types-begin btech 00444
//|---Wizard-only field report; also publishes lines privately to the actor.
//|---@param actor integer
//|---@param station integer
//|---@param options? string Optional 1/4 column selector followed by field prefix.
//|---@return string
//|function btech_gunner.view_fields(actor, station, options) end
// lua-types-end

// lua-types-begin btech 00445
//|---Return a silent brief unit report with the parent sensor admission and ordinary disclosure.
//|---@param station integer
//|---@param gunner integer
//|---@param target integer
//|---@return string
//|function btech_gunner.report(station, gunner, target) end
// lua-types-end

// lua-types-begin btech 00446
//|---Inspect the first acquired occupant at a coordinate using parent map order.
//|---@param station integer
//|---@param gunner integer
//|---@param x integer
//|---@param y integer
//|---@param options string? A/I/W report sections.
//|---@return string
//|function btech_gunner.scan_hex(station, gunner, x, y, options) end
// lua-types-end

// lua-types-begin btech 00447
//|---Inspect a building using cached parent perception; publish to the station and award gunner experience.
//|---@param station integer
//|---@param gunner integer
//|---@param x integer
//|---@param y integer
//|---@return BattleBuildingScan
//|function btech_gunner.scan_building(station, gunner, x, y) end
// lua-types-end

// lua-types-begin btech 00448
//|---Scan buildings and mines atomically; dice, experience and station output roll back together.
//|---@param station integer
//|---@param gunner integer
//|---@param x integer
//|---@param y integer
//|---@return BattleHexScan
//|function btech_gunner.scan_terrain(station, gunner, x, y) end
// lua-types-end

// lua-types-begin btech 00449
//|---Scan the station selection without altering either lock or its settling countdown.
//|---@param station integer
//|---@param gunner integer
//|---@param options string? A/I/W report sections.
//|---@return BattleSelectedScan
//|function btech_gunner.scan_selected(station, gunner, options) end
// lua-types-end
