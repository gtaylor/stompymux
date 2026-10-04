//! LuaLS contract blocks for the btech unit surface.
// This file is read by lua-type-updater. Keep declarations next to the bindings.

// lua-types-begin btech 00034
//|---Read detached VTOL fuel and current stock-derived capacity in a callback.
//|---@param unit integer
//|---@return VtolFuelStatus
//|function btech_unit.fuel(unit) end
// lua-types-end

// lua-types-begin btech 00035
//|---Wizard fuel correction bounded by current capacity and 4294967295.
//|---Fuel and throttle changes participate in callback rollback; this does not repair lost lift.
//|---@param actor integer
//|---@param unit integer
//|---@param amount integer Nonnegative remaining fuel.
//|---@return VtolFuelStatus
//|function btech_unit.set_fuel(actor, unit, amount) end
//|local btech_player = {}
// lua-types-end

// lua-types-begin btech 00036
//|---Inspect a saved unit identity without activating simulation.
//|---@param dbref integer
//|---@return StoredBattleUnit
//|function btech_unit.inspect(dbref) end
// lua-types-end

// lua-types-begin btech 00058
//|---Construct a persistent Mech or ground vehicle on an unused live thing. Transactional.
//|---@param dbref integer
//|---@param name string Template reference: the file stem of a `.toml` document anywhere under database.mech_database.
//|---@return boolean
//|function btech_unit.create(dbref, name) end
// lua-types-end

// lua-types-begin btech 00059
//|---Inspect detached construction state; deferred saved units raise an error.
//|---@param dbref integer
//|---@return UnitState|VehicleState
//|function btech_unit.state(dbref) end
// lua-types-end

// lua-types-begin btech 00060
//|---Place a unit on decoded terrain and update world containment. Transactional.
//|---@param dbref integer
//|---@param map integer
//|---@param x integer
//|---@param y integer
//|---@return boolean
//|function btech_unit.place(dbref, map, x, y) end
// lua-types-end

// lua-types-begin btech 00061
//|---Clear placement and move a unit into an ordinary container. Transactional.
//|---@param dbref integer
//|---@param destination integer
//|---@return boolean
//|function btech_unit.remove(dbref, destination) end
// lua-types-end

// lua-types-begin btech 00062
//|---Assign a present player to an available cockpit. The caller supplies access policy.
//|---@param dbref integer Unit object.
//|---@param player integer Pilot object.
//|---@return boolean
//|function btech_unit.pilot(dbref, player) end
// lua-types-end

// lua-types-begin btech 00063
//|---Wizard-only team edit for a placed unit; negatives normalize to zero, other signature facts are retained.
//|---@param actor integer
//|---@param unit integer
//|---@param team integer Signed 32-bit team number.
//|---@return integer team Normalized value, also reported privately to the administrator.
//|function btech_unit.set_team(actor, unit, team) end
// lua-types-end

// lua-types-begin btech 00064
//|---Wizard-only literal emote to running units currently seeing the source; source cockpit excluded.
//|---Privately confirms completion. Entire publication rolls back on failure.
//|---@param actor integer
//|---@param unit integer Placed physical unit; need not be running or piloted by actor.
//|---@param message string Empty text and leading apostrophes retain ordinary emote semantics.
//|---@return integer observers Number of addressed observer units.
//|function btech_unit.losemit(actor, unit, message) end
// lua-types-end

// lua-types-begin btech 00065
//|---Wizard-only saved ID preference; does not change the current label or consume dice.
//|---Rust extension retained under its descriptive name; the canonical setter follows the C contract.
//|---@param actor integer Wizard actor.
//|---@param unit integer Constructed unit; no placement or power requirement.
//|---@param value? string Exactly two ASCII letters; nil or empty clears the preference.
//|---@return string? preferred_id Normalized uppercase preference, or nil when cleared.
//|function btech_unit.set_preferred_id_as(actor, unit, value) end
// lua-types-end

// lua-types-begin btech 00066
//|---Set the saved two-letter battlefield ID preference; nil clears it without consuming dice.
//|---@param unit DbRef|Object Constructed unit.
//|---@param id string|nil Exactly two ASCII letters; nil clears the preference.
//|function btech_unit.set_preferred_id(unit, id) end
// lua-types-end

// lua-types-begin btech 00067
//|---Read the saved display-name override, or nil when the template name is in use.
//|---@param unit DbRef|Object
//|---@return string|nil name
//|function btech_unit.display_name(unit) end
// lua-types-end

// lua-types-begin btech 00068
//|---Set a display override of at most 120 bytes; an empty string clears it. Wizard only.
//|---Rust extension retained under its descriptive name; the canonical setter follows the C contract.
//|---@param actor integer
//|---@param unit integer
//|---@param name string
//|---@return boolean success
//|function btech_unit.set_display_name_as(actor, unit, name) end
// lua-types-end

// lua-types-begin btech 00069
//|---Replace the saved display-name override; nil or an empty string clears it.
//|---@param unit DbRef|Object Constructed unit.
//|---@param name string|nil At most 120 bytes.
//|function btech_unit.set_display_name(unit, name) end
// lua-types-end

// lua-types-begin btech 00070
//|---Read the saved markings string, or nil when no markings are configured.
//|---@param unit DbRef|Object
//|---@return string|nil markings
//|function btech_unit.markings(unit) end
// lua-types-end

// lua-types-begin btech 00071
//|---Wizard-only literal markings, at most 16383 bytes; empty clears. Callback failures roll back.
//|---Rust extension retained under its descriptive name; the canonical setter follows the C contract.
//|---@param actor integer
//|---@param unit integer
//|---@param markings string
//|---@return boolean success
//|function btech_unit.set_markings_as(actor, unit, markings) end
// lua-types-end

// lua-types-begin btech 00072
//|---Replace the saved markings, at most 16383 bytes; nil or an empty string clears them.
//|---@param unit DbRef|Object Constructed unit.
//|---@param markings string|nil
//|function btech_unit.set_markings(unit, markings) end
// lua-types-end

// lua-types-begin btech 00073
//|---View escaped markings through running cockpit contact and unblocked-LOS admission.
//|---No scan-range limit; omitted target uses this operator's selected unit.
//|---@param unit integer Cockpit unit.
//|---@param actor integer
//|---@param target? integer
//|---@return string text Styled report safe for normal output.
//|function btech_unit.view(unit, actor, target) end
// lua-types-end

// lua-types-begin btech 00074
//|---Wizard map assignment; -1 removes membership and retains the pose for re-entry.
//|---A removed running unit shuts down on the next simulation update.
//|---@param actor integer Wizard and private confirmation recipient.
//|---@param unit integer Physical constructed unit.
//|---@param map integer Decimal map dbref, or -1 for removal.
//|---@param preferred? string First two bytes override configuration; short/nil values use the saved preference, then random selection.
//|---@return {assignment: table|nil} report Assigned position, label and reset_origin, or nil on removal.
//|function btech_unit.setmapindex(actor, unit, map, preferred) end
// lua-types-end

// lua-types-begin btech 00075
//|---Wizard repositioning within the current battlefield, preserving controls and tow attachment.
//|---@param actor integer Wizard actor and confirmation recipient.
//|---@param unit integer Placed physical unit.
//|---@param x integer
//|---@param y integer
//|---@param z? integer Signed-short altitude; nil selects the surface and lands VTOLs.
//|---@return {position: table, elevation: integer} report
//|function btech_unit.setxy(actor, unit, x, y, z) end
// lua-types-end

// lua-types-begin btech 00076
//|---Wizard orbital insertion. Detach towing first; reject prone units and active digging.
//|---Ground chassis receive mass-based cocoons. VTOLs enter flight with half-speed requested.
//|---Stopped VTOLs retain the inserted pose and controls until startup finishes.
//|---@param actor integer Wizard actor and confirmation recipient.
//|---@param unit integer Placed physical unit.
//|---@param x integer
//|---@param y integer
//|---@param z? integer Signed-short altitude; nil defaults to 300.
//|---@return {position: table, elevation: integer, drop: table|nil, flight: table|nil} report
//|---An attached on_ood_land event runs before landing dice/damage, with the arriving unit as object/enactor/cause.
//|---Callback errors restore the entire airborne tick and its output.
//|function btech_unit.ood(actor, unit, x, y, z) end
// lua-types-end

// lua-types-begin btech 00077
//|---Wizard-only construction allocation using original components and installed ammunition bins.
//|---@param actor integer Recipient; requires wizard authority.
//|---@param unit integer Power, pilot assignment and placement are not required.
//|---@return string report Publishes privately and returns formatted weight rows and total.
//|function btech_unit.weight(actor, unit) end
// lua-types-end

// lua-types-begin btech 00078
//|---Wizard-only random damage packets through shared combat and casualty rules.
//|---@param actor integer
//|---@param unit integer Power, pilot and placement are not required.
//|---@param damage integer 1 through 1000.
//|---@param clusters integer Packet count from 1 through damage; integer division discards the remainder.
//|---@param rear boolean Select rear armor, also enabled by a rear incoming arc.
//|---@param critical boolean Accepted flag; random location routing chooses critical eligibility.
//|---@return {packet_damage: integer, discarded_damage: integer, impacts: table[]} report
//|function btech_unit.damage(actor, unit, damage, clusters, rear, critical) end
// lua-types-end

// lua-types-begin btech 00079
//|---Wizard-only located damage through shared critical, crew and evacuation rules.
//|---@param actor integer
//|---@param unit integer
//|---@param section string Chassis-specific location or abbreviation.
//|---@param damage integer 1 through 1000.
//|---@param rear boolean Rear armor selection; vehicle front hits redirect to rear.
//|---@param critical boolean Through-armor critical candidate.
//|---@return {kind: "mech"|"vehicle", impact: table} report
//|function btech_unit.damage_section(actor, unit, section, damage, rear, critical) end
// lua-types-end

// lua-types-begin btech 00080
//|---Release this player's cockpit assignment without moving the player.
//|---@param dbref integer
//|---@param player integer
//|---@return boolean
//|function btech_unit.release(dbref, player) end
// lua-types-end

// lua-types-begin btech 00081
//|---Start the assigned pilot's unit. Trusted scripts authorize the optional fast override.
//|---@param dbref integer
//|---@param player integer
//|---@param fast boolean|nil Five-second override; otherwise 30 seconds.
//|---@return boolean
//|function btech_unit.start(dbref, player, fast) end
// lua-types-end

// lua-types-begin btech 00082
//|---Abort startup or shut down the assigned pilot's unit, releasing the cockpit.
//|---@param dbref integer
//|---@param player integer
//|---@return boolean
//|function btech_unit.stop(dbref, player) end
// lua-types-end

// lua-types-begin btech 00084
//|---Measure placed units on the same map. Does not check visibility or weapon eligibility.
//|---@param first integer
//|---@param second integer
//|---@return Range
//|function btech_unit.range(first, second) end
// lua-types-end

// lua-types-begin btech 00085
//|---Set desired heading on the current pilot's running unit and stage a cockpit confirmation.
//|---@param dbref integer
//|---@param player integer
//|---@param degrees number? Omit to read actual heading without notification.
//|---@return boolean|number
//|function btech_unit.heading(dbref, player, degrees) end
// lua-types-end

// lua-types-begin btech 00086
//|---Set desired speed within the running unit's forward/reverse limits and stage a cockpit confirmation.
//|---@param dbref integer
//|---@param player integer
//|---@param kph number|string? Omit to read actual speed; names include walk/cruise, run/flank, stop and back. Numeric requests clamp to throttle limits.
//|---@return boolean|number
//|function btech_unit.speed(dbref, player, kph) end
// lua-types-end

// lua-types-begin btech 00087
//|---Evacuate non-wizard contents of an in-character unit to the configured afterlife.
//|---Requires a wizard actor and callback. Ordinary teleport hooks run; failures roll back moves and XP.
//|---Tactical units do nothing. Configured XP retention applies only when in-character rules are enabled.
//|---@param dbref integer Unit object.
//|---@param player integer Wizard actor.
//|---@return integer Number of occupants moved.
//|function btech_unit.evacuate(dbref, player) end
// lua-types-end

// lua-types-begin btech 00152
//|---Read-only summary of a placed unit's automatic perception: sensor band, sight, probe and radar.
//|---@param dbref integer
//|---@return PerceptionReport
//|function btech_unit.perception(dbref) end
// lua-types-end

// lua-types-begin btech 00153
//|---@class PerceptionReport
//|---@field light "night"|"twilight"|"day" Current battlefield light.
//|---@field sight_range integer Weather visibility in hexes, capped by the map ceiling.
//|---@field lit_sight_range integer Reach to illuminated targets; triple sight at night.
//|---@field sensor_range integer Effective all-conditions sensor band; zero while unavailable.
//|---@field sensors PerceptionStatus Condition of the sensor band.
//|---@field probe {kind: ProbeKind, range: integer, status: PerceptionStatus}|nil Best installed active probe.
//|---@field radar {range: integer, status: PerceptionStatus}|nil Anti-aircraft radar, if installed.
//|---@field running boolean Stopped units perceive nothing.
//|---@field text string The report printed by the sensor command.
// lua-types-end

// lua-types-begin btech 00157
//|---Read acquired contacts the unit still perceives; no acquisition rolls.
//|---@param dbref integer Running observer unit dbref.
//|---@param preferences ContactPreferences? Optional inclusion filter; omitted lists all acquired contacts.
//|---@return ContactView[]
//|function btech_unit.contacts(dbref, preferences) end
// lua-types-end

// lua-types-begin btech 00158
//|---Select a current acquired target or clear selection with nil; requires the conscious assigned pilot.
//|---@param dbref integer
//|---@param pilot integer
//|---@param target integer?
//|---@return boolean
//|function btech_unit.lock(dbref, pilot, target) end
// lua-types-end

// lua-types-begin btech 00162
//|---Illuminate an enemy within fifteen hexes, or stop with nil; requires working TAG and a ready timer.
//|---@param dbref integer
//|---@param pilot integer
//|---@param target integer?
//|---@return boolean
//|function btech_unit.tag(dbref, pilot, target) end
// lua-types-end

// lua-types-begin btech 00163
//|---Read current connected pilot gunnery under configured weapon-family rules; default six without one.
//|---@param dbref integer Unit dbref.
//|---@param weapon integer Zero-based weapon index.
//|---@return integer Signed skill target; no XP award or firing permission.
//|function btech_unit.gunnery(dbref, weapon) end
// lua-types-end

// lua-types-begin btech 00167
//|---Inspect mounted weapons without acquiring targets or consuming dice; calling scripts own access policy.
//|---Rust extension retained under its descriptive name; the canonical weapons list follows the C contract.
//|---@param dbref integer
//|---@return WeaponInspection[] Lua array positions start at one; use each entry's index to fire.
//|function btech_unit.weapon_states(dbref) end
// lua-types-end

// lua-types-begin btech 00168
//|---List mounted weapons in mounting order; an optional section restricts the result.
//|---@param unit DbRef|Object
//|---@param section? MechSection Typed section constant from btech.unit.sections.
//|---@return MountedWeapon[]
//|function btech_unit.weapons(unit, section) end
// lua-types-end

// lua-types-begin btech 00169
//|---Read a TIC's ordered weapon numbers; requires the conscious assigned pilot.
//|---@param dbref integer
//|---@param pilot integer
//|---@param group integer Zero-based group, 0 through 3.
//|---@return integer[]
//|function btech_unit.tic(dbref, pilot, group) end
// lua-types-end

// lua-types-begin btech 00170
//|---Edit persistent membership; add/remove require weapon numbers, clear omits them.
//|---@param dbref integer
//|---@param pilot integer
//|---@param group integer
//|---@param operation "add"|"remove"|"clear"
//|---@param weapons integer[]|nil
//|function btech_unit.tic_edit(dbref, pilot, group, operation, weapons) end
// lua-types-end

// lua-types-begin btech 00171
//|---Fire groups in ascending order using ordinary firing rules. Shot rejection continues;
//|---fall or shutdown stops firing. Callback failure restores the whole batch.
//|---@param dbref integer
//|---@param pilot integer
//|---@param groups integer[] Zero-based group numbers, 0 through 3.
//|---@param target integer|{x: integer, y: integer}|nil Explicit unit/coordinates, or cockpit selection when omitted.
//|---@return {group: integer, weapon: integer, report: table|nil, rejection: string|nil}[]
//|function btech_unit.tic_fire(dbref, pilot, groups, target) end
// lua-types-end

// lua-types-begin btech 00172
//|---Begin a four-second Mech heat cutoff toggle; requires the assigned conscious pilot.
//|---The configuration gate applies to new toggles; admitted transitions survive shutdown.
//|---@param dbref integer
//|---@param pilot integer
//|---@return boolean
//|function btech_unit.heatcutoff(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00173
//|---Toggle an MML between SRM and LRM ammunition; requires dedicated matching bins.
//|---SRMs use 3/6/9 range and two-point hits; LRMs use 7/14/21, minimum six, and five-point groups.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon number.
//|---@return AmmunitionMode The selected supply: SRM rounds, or mml_lrm and its mml_lrm_* special rounds for LRM.
//|function btech_unit.mml(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00174
//|---Toggle Extended Range ammunition on an ATM launcher; requires matching bins.
//|---Extended Range missiles deal one damage each at 9/18/27 hexes with a four-hex minimum.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.atmrange(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00175
//|---Toggle High Explosive ammunition using the same eligibility and saved selection rules.
//|---High Explosive missiles deal three damage each at 3/6/9 hexes with no minimum range.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.atmexplosive(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00176
//|---Power down a Gauss mount after recharge. Requires a running, mapped unit and conscious pilot.
//|---Persists across shutdown/restart, prevents firing and suppresses Gauss critical explosions.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon number.
//|---@return boolean
//|function btech_unit.disable(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00177
//|---Select an ammunition section, or clear with nil or '-'. Requires a conscious assigned
//|---pilot, map placement and an intact non-recycling ammunition weapon, but no reactor power.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon number.
//|---@param section string|nil Cockpit section name or abbreviation.
//|---@return boolean
//|function btech_unit.usebin(dbref, pilot, weapon, section) end
// lua-types-end

// lua-types-begin btech 00178
//|---Toggle automatic turret tracking; requires a surviving turret, map and conscious assigned pilot.
//|---The mode may be selected while stopped; actual tracking requires a running vehicle.
//|---@param dbref integer
//|---@param pilot integer
//|---@return boolean
//|function btech_unit.autoturret(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00202
//|---Fire under configured tactical rules and stage cockpit notices in the current callback transaction.
//|---Requires the conscious assigned pilot; the calling script owns authority to act as that pilot.
//|---An omitted target uses cockpit selection, including automatic coolant self-selection.
//|---An IDF observer takes precedence when the firer has no unit lock, even with explicit arguments.
//|---Explicit coordinates select an occupant for conventional weapons or use terrain effects when empty.
//|---Artillery always uses coordinates and queues its impact. Explicit requests do not change saved locks.
//|---Missile effects require the base target number even with near-miss glancing enabled.
//|---Such a near miss spends its launch and aimed preparation, but starts no AMS, pod effect or Swarm flight.
//|---Results are detached; optional fields use nil.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon number from unit.weapons.
//|---@param target integer|{x: integer, y: integer}|nil Explicit target leaves the selected lock unchanged.
//|---@return MechShotReport|VehicleShotReport|HexShotReport|ArtilleryLaunchReport
//|function btech_unit.fire(dbref, pilot, weapon, target) end
// lua-types-end

// lua-types-begin btech 00204
//|---Select a section of the current unit target using its anatomical aliases.
//|---Requires a running unit and its conscious assigned pilot. Nil or "-" clears without a lock.
//|---The saved class and section persist across lock changes and shutdown. Returned values are detached.
//|---@param dbref integer
//|---@param pilot integer
//|---@param section string|nil
//|---@return AimSelection|nil
//|function btech_unit.target(dbref, pilot, section) end
// lua-types-end

// lua-types-begin btech 00205
//|---Read the saved anatomical preference without requiring a running unit or a current target.
//|---@param dbref integer
//|---@return AimSelection|nil
//|function btech_unit.aimed_section(dbref) end
// lua-types-end

// lua-types-begin btech 00207
//|---Sight a weapon using ordinary target selection and aim, without firing or revealing cover.
//|---Consumes preparation and attack dice; ignores ammunition, recycling and feed jams.
//|---Requires an intact offensive mount and the conscious assigned pilot of a running unit.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@param target integer|HexCoordinate|nil Omitted target uses cockpit selection.
//|---@return SightReport
//|function btech_unit.sight(dbref, pilot, weapon, target) end
// lua-types-end

// lua-types-begin btech 00208
//|---Rotate one step left/right or center the torso; stages the native cockpit message.
//|---@param dbref integer
//|---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
//|---@param direction 'left'|'right'|'center' Case-insensitive; l/r/c aliases are accepted.
//|---@return boolean
//|function btech_unit.rottorso(dbref, pilot, direction) end
// lua-types-end

// lua-types-begin btech 00209
//|---Without a mode, schedule a five-second manual toggle; repeated calls preserve the pending switch.
//|---With a mode, select it and steer the lamp toward it. AUTO lights the lamp at night and
//|---extinguishes it otherwise, re-evaluated when map light changes, the unit changes maps or
//|---finishes starting up.
//|---@param dbref integer
//|---@param pilot integer
//|---@param mode? SearchlightMode Typed constant from btech.unit.searchlight_modes.
//|---@return boolean
//|function btech_unit.slite(dbref, pilot, mode) end
// lua-types-end

// lua-types-begin btech 00210
//|---Toggle forward/backward arms on a capable standing, running chassis.
//|---@param dbref integer
//|---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
//|---@return boolean
//|function btech_unit.fliparms(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00211
//|---Attempt a jump; a failed stagger check falls instead of launching, within the callback transaction.
//|---A completed check survives later destination rejection; an enclosing callback failure still rolls back.
//|---@param dbref integer
//|---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
//|---@param bearing integer Compass degrees.
//|---@param range number Positive range in hex heights, snapped to a destination hex center.
//|---@return boolean Accepted attempt; inspect flight state to distinguish launch from a stagger fall.
//|function btech_unit.jump(dbref, pilot, bearing, range) end
// lua-types-end

// lua-types-begin btech 00212
//|---Attempt early jump landing or VTOL touchdown at the current point; cancel a queued VTOL launch.
//|--- Character XP, injuries and crew evacuation commit together; failures restore the whole landing.
//|---@param dbref integer
//|---@param pilot integer
//|---@return boolean
//|function btech_unit.land(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00213
//|---Begin an eighteen-second hangar entry; current route, eligibility and locks are rechecked at expiry.
//|---@param dbref integer
//|---@param pilot integer
//|---@param direction string? One-byte direction; omitted or longer selector uses the first entrance.
//|---@return boolean admitted False when the enter lock denies entry.
//|function btech_unit.enterbase(dbref, pilot, direction) end
// lua-types-end

// lua-types-begin btech 00214
//|---Queue VTOL takeoff using configured fuel rules and stage a cockpit confirmation.
//|---@param dbref integer
//|---@param pilot integer
//|---@param delay integer? Extra launch seconds, 0..65535; nonzero requires a wizard.
//|---@return boolean
//|function btech_unit.takeoff(dbref, pilot, delay) end
// lua-types-end

// lua-types-begin btech 00215
//|---Read or set VTOL vertical speed using configured fuel rules and the shared velocity budget.
//|---@param dbref integer
//|---@param pilot integer
//|---@param kph number? Omit for current speed; positive climbs, negative descends.
//|---@return boolean|number
//|function btech_unit.vertical(dbref, pilot, kph) end
// lua-types-end

// lua-types-begin btech 00216
//|---Attempt to stand, staging fall and terrain-break notices in the callback transaction.
//|---@param dbref integer
//|---@param pilot integer
//|---@param mode? 'normal'|'anyway'|'careful'
//|---@return table attempt Contains check, optional fall, rise/retry timer and ordered notices.
//|function btech_unit.stand(dbref, pilot, mode) end
// lua-types-end

// lua-types-begin btech 00217
//|---Drop prone; fast travel can require a control roll and cause ordinary fall damage.
//|---@param dbref integer
//|---@param pilot integer
//|---@return table report Contains optional check/fall, flooding, stepping mines and ordered notices.
//|function btech_unit.prone(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00218
//|---Begin hiding in forest, mountains or rough terrain; requires camouflage equipment or a wizard pilot.
//|---Hostile acquired contacts or leaving the ground stop preparation. Hex crossings and firing cancel it.
//|---@param dbref integer Supported unit dbref.
//|---@param pilot integer Assigned pilot.
//|---@return boolean accepted Timer and cockpit feedback commit with the callback.
//|function btech_unit.hide(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00247
//|---Set the unit's downhill cliff preference; requires its present pilot.
//|---@param dbref integer
//|---@param player integer
//|---@param enabled boolean
//|---@return boolean
//|function btech_unit.auto_fall(dbref, player, enabled) end
// lua-types-end

// lua-types-begin btech 00248
//|---Toggle an intact, recycled flamer between damage and heat-transfer modes; stages cockpit notices.
//|---@param dbref integer
//|---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
//|---@param weapon integer Zero-based weapon index.
//|---@return FireMode
//|function btech_unit.flamerheat(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00249
//|---Toggle an intact, recycled LB-X autocannon between slug and cluster ammunition; stages cockpit notices.
//|---@param dbref integer
//|---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
//|---@param weapon integer Zero-based weapon index.
//|---@return AmmunitionMode
//|function btech_unit.lbx(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00250
//|---Toggle an intact recycled artillery launcher between normal and cluster rounds; rejects smoke/mine selection.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return AmmunitionMode
//|function btech_unit.cluster(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00255
//|---Toggle Artemis-compatible ammunition; requires a live linked controller and a recycled missile launcher.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return AmmunitionMode
//|function btech_unit.artemis(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00257
//|---Begin timed feed recovery; stages cockpit feedback and participates in callback rollback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return boolean
//|function btech_unit.unjam(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00258
//|---Toggle hotloading on a supported recycled indirect-fire launcher; stages cockpit feedback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return FireMode
//|function btech_unit.hotload(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00259
//|---Toggle one- or two-round Ultra autocannon firing; stages cockpit feedback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return FireMode
//|function btech_unit.ultra(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00260
//|---Toggle rapid two-round conventional or light autocannon firing.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return FireMode
//|function btech_unit.rapidfire(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00261
//|---Set rotary burst length; repeated selection stays enabled and returns false.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@param rounds? 1|2|3|4|5|6 Defaults to one.
//|---@return boolean changed
//|function btech_unit.rac(dbref, pilot, weapon, rounds) end
// lua-types-end

// lua-types-begin btech 00262
//|---Toggle gatling machine-gun fire, with cockpit feedback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return FireMode
//|function btech_unit.gattling(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00263
//|---Toggle armor-piercing autocannon ammunition with cockpit feedback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return AmmunitionMode
//|function btech_unit.armorpiercing(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00264
//|---Toggle caseless autocannon ammunition with cockpit feedback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return AmmunitionMode
//|function btech_unit.caseless(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00265
//|---Toggle incendiary autocannon ammunition with cockpit feedback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return AmmunitionMode
//|function btech_unit.incendiary(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00266
//|---Toggle inferno missile ammunition with cockpit feedback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return AmmunitionMode
//|function btech_unit.inferno(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00267
//|---Toggle Precision autocannon ammunition with cockpit feedback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return AmmunitionMode
//|function btech_unit.precision(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00268
//|---Toggle Flechette autocannon ammunition with cockpit feedback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer Zero-based weapon index.
//|---@return AmmunitionMode
//|function btech_unit.flechette(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00269
//|---Set the assigned pilot's unit illumination-warning preference.
//|---@param dbref integer
//|---@param player integer
//|---@param enabled boolean
//|---@return boolean
//|function btech_unit.searchlight_warning(dbref, player, enabled) end
// lua-types-end

// lua-types-begin btech 00270
//|---Set the assigned pilot's armor warning preference.
//|---@param dbref integer
//|---@param player integer
//|---@param enabled boolean
//|---@return boolean
//|function btech_unit.armor_warning(dbref, player, enabled) end
// lua-types-end

// lua-types-begin btech 00271
//|---Set the assigned pilot's ammunition warning preference.
//|---@param dbref integer
//|---@param player integer
//|---@param enabled boolean
//|---@return boolean
//|function btech_unit.ammunition_warning(dbref, player, enabled) end
// lua-types-end

// lua-types-begin btech 00272
//|---Set MechWarrior safety; requires the assigned pilot in the cockpit.
//|---@param dbref integer
//|---@param player integer
//|---@param enabled boolean
//|---@return boolean success
//|function btech_unit.mw_safety(dbref, player, enabled) end
// lua-types-end

// lua-types-begin btech 00273
//|---Set the retained BTHDebug preference; requires the assigned pilot in the cockpit.
//|---@param dbref integer
//|---@param player integer
//|---@param enabled boolean
//|---@return boolean success
//|function btech_unit.bth_debug(dbref, player, enabled) end
// lua-types-end

// lua-types-begin btech 00274
//|---Set the assigned pilot's friendly-fire safety. Coolant guns are exempt.
//|---@param dbref integer
//|---@param player integer
//|---@param enabled boolean
//|---@return boolean
//|function btech_unit.friendly_fire_safety(dbref, player, enabled) end
// lua-types-end

// lua-types-begin btech 00275
//|---Read-only cockpit status as styled-text source with escaped literal fields. Select armor, info, weapons, heat, short or AIWHS. N/NW select the compact export.
//|---@param dbref integer
//|---@param options string?
//|---@return string
//|function btech_unit.status(dbref, options) end
// lua-types-end

// lua-types-begin btech 00279
//|---Inspect durable equipment condition; empty ammunition, shutdown and recycle do not imply damage.
//|---Requires a trusted callback transaction. Returned rows are detached from saved unit state.
//|---@param dbref integer
//|---@return WeaponDiagnostic[]
//|function btech_unit.weapon_diagnostics(dbref) end
// lua-types-end

// lua-types-begin btech 00281
//|---Inspect distinct installed weapon types in first-installation order, including destroyed mounts.
//|---Requires a trusted callback transaction. Range columns follow the server's extended-range setting.
//|---@param dbref integer
//|---@return WeaponSpecification[]
//|function btech_unit.weapon_specifications(dbref) end
// lua-types-end

// lua-types-begin btech 00284
//|---Inspect equipment by cockpit section alias. The trusted query requires a callback transaction.
//|---Rows are detached from saved state; native CRITSTATUS separately requires a conscious assigned pilot.
//|---@param dbref integer
//|---@param section string
//|---@return CriticalReport
//|function btech_unit.criticals(dbref, section) end
// lua-types-end

// lua-types-begin btech 00285
//|---Attempt a biped kick; rolls back damage, falls, recovery and notices with the callback.
//|---@param dbref integer
//|---@param pilot integer
//|---@param leg? 'left'|'right' Defaults to right.
//|---@param target? integer Defaults to the selected target; explicit targets require acquisition.
//|---@return table report Attack profile, roll, hit, glancing, impact, balance, fall and notices.
//|function btech_unit.kick(dbref, pilot, leg, target) end
// lua-types-end

// lua-types-begin btech 00286
//|---Attempt one or both arms in left-to-right order. Default selection is both.
//|---Unavailable arms are reported separately when another arm attacks; an entirely rejected action raises an error.
//|---Impact failures and callback aborts roll back the complete action and staged messages.
//|---@param dbref integer
//|---@param pilot integer
//|---@param arms? 'left'|'right'|'both'
//|---@param target? integer Defaults to the selected target; explicit targets require acquisition.
//|---@return table report Ordered attacks, per-arm rejections and notices. Each attack includes its profile and optional impact.
//|function btech_unit.punch(dbref, pilot, arms, target) end
// lua-types-end

// lua-types-begin btech 00287
//|---Attempt a leg trip. A hit forces target balance; a miss has no balance check. No direct impact damage.
//|---Both legs and hips must be usable; the target must be standing and not rising.
//|---@param dbref integer
//|---@param pilot integer
//|---@param leg? 'left'|'right' Defaults to right.
//|---@param target? integer Defaults to selected target; explicit targets require acquisition.
//|---@return table report Attack profile, roll, hit, glancing, optional balance/fall, and notices.
//|function btech_unit.trip(dbref, pilot, leg, target) end
// lua-types-end

// lua-types-begin btech 00288
//|---Attempt an axe swing. Default selection tries equipped arms left first; an accepted swing blocks the other arm through recovery.
//|---@param dbref integer
//|---@param pilot integer
//|---@param arms? 'left'|'right'|'both'
//|---@param target? integer Defaults to selected target; explicit targets require acquisition.
//|---@return table report Ordered attacks, per-arm rejections and transactional notices.
//|function btech_unit.axe(dbref, pilot, arms, target) end
// lua-types-end

// lua-types-begin btech 00289
//|---Attempt a sword swing with the same selection and transaction rules as axe.
//|---@param dbref integer
//|---@param pilot integer
//|---@param arms? 'left'|'right'|'both'
//|---@param target? integer
//|---@return table report
//|function btech_unit.sword(dbref, pilot, arms, target) end
// lua-types-end

// lua-types-begin btech 00290
//|---Attempt a mace swing. A missed swing requires an attacker piloting check with a +2 modifier.
//|---@param dbref integer
//|---@param pilot integer
//|---@param arms? 'left'|'right'|'both'
//|---@param target? integer
//|---@return table report Ordered attacks, per-arm rejections and transactional notices.
//|function btech_unit.mace(dbref, pilot, arms, target) end
// lua-types-end

// lua-types-begin btech 00291
//|---Attempt a dual-saw attack; seven operational parts required, fixed seven base damage without TSM boost.
//|---@param dbref integer
//|---@param pilot integer
//|---@param arms? 'left'|'right'|'both'
//|---@param target? integer
//|---@return table report Ordered attacks, per-arm rejections and transactional notices.
//|function btech_unit.saw(dbref, pilot, arms, target) end
// lua-types-end

// lua-types-begin btech 00292
//|---Attempt claw attacks, left then right by default; each accepted arm starts its own recovery.
//|---@param dbref integer
//|---@param pilot integer
//|---@param arms? 'left'|'right'|'both'
//|---@param target? integer
//|---@return table report Ordered attacks, per-arm rejections and transactional notices.
//|function btech_unit.claw(dbref, pilot, arms, target) end
// lua-types-end

// lua-types-begin btech 00293
//|---Swing the physical weapon installed in each selected arm (axe, sword, mace, dual saw, claw,
//|---retractable blade, lance, flail, wrecking ball, chain whip or vibroblade). Both arms by
//|---default, skipping arms without a weapon; only claws let the second arm follow a completed swing.
//|---@param dbref integer
//|---@param pilot integer
//|---@param arms? 'left'|'right'|'both'
//|---@param target? integer Defaults to selected target; explicit targets require acquisition.
//|---@return table report Ordered attacks, per-arm rejections and transactional notices.
//|function btech_unit.melee(dbref, pilot, arms, target) end
// lua-types-end

// lua-types-begin btech 00294
//|---Grab a tree in a selected arm (left first by default), or drop it with '-'.
//|---@param dbref integer
//|---@param pilot integer
//|---@param arm? 'left'|'right'|'-'
//|---@return table notices
//|function btech_unit.grabclub(dbref, pilot, arm) end
// lua-types-end

// lua-types-begin btech 00295
//|---Swing a club using both arms; a carried tree shatters on a hit. Forest terrain supplies an immediate tree.
//|---@param dbref integer
//|---@param pilot integer
//|---@param target? integer
//|---@return table report Physical attack, damage, recovery and notices.
//|function btech_unit.club(dbref, pilot, target) end
// lua-types-end

// lua-types-begin btech 00296
//|---Select a charge target without starting movement. Nil uses the current target; '-' cancels.
//|---@param dbref integer
//|---@param pilot integer
//|---@param target integer|"-"|nil
//|---@return table[] notices
//|function btech_unit.charge(dbref, pilot, target) end
// lua-types-end

// lua-types-begin btech 00297
//|---Attempt a DFA jump using the shared pre-launch stagger check. Nil uses the current target lock.
//|---@param dbref integer
//|---@param pilot integer
//|---@param target? integer
//|---@return boolean Accepted attempt; a stagger failure can prevent launch.
//|function btech_unit.dfa(dbref, pilot, target) end
// lua-types-end

// lua-types-begin btech 00298
//|---Toggle automatic anti-missile defense, or set an explicit enabled state.
//|---@param dbref integer
//|---@param pilot integer
//|---@param enabled boolean|nil
//|---@return boolean enabled
//|function btech_unit.ams(dbref, pilot, enabled) end
// lua-types-end

// lua-types-begin btech 00299
//|---Toggle Narc-compatible ammunition on a missile weapon.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.narc(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00300
//|---Toggle explosive ammunition on a Narc launcher.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.explosive(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00302
//|---Toggle the corresponding suite mode inside the callback transaction.
//|---@param dbref integer
//|---@param pilot integer
//|---@return ElectronicMode
//|function btech_unit.ecm(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00303
//|---Toggle the corresponding suite mode inside the callback transaction.
//|---@param dbref integer
//|---@param pilot integer
//|---@return ElectronicMode
//|function btech_unit.eccm(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00304
//|---Toggle the corresponding suite mode inside the callback transaction.
//|---@param dbref integer
//|---@param pilot integer
//|---@return ElectronicMode
//|function btech_unit.angelecm(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00305
//|---Toggle the corresponding suite mode inside the callback transaction.
//|---@param dbref integer
//|---@param pilot integer
//|---@return ElectronicMode
//|function btech_unit.angeleccm(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00306
//|---Select iNarc homing (-), explosive (X), haywire (Y), ECM (E), or Nemesis (Z) ammunition.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@param selector? string
//|---@return AmmunitionMode
//|function btech_unit.inarc(dbref, pilot, weapon, selector) end
// lua-types-end

// lua-types-begin btech 00308
//|---Inspect pod effects on all sections, or return an empty list when none are attached.
//|---@param dbref integer
//|---@param pilot integer
//|---@return PodRow[]
//|function btech_unit.pods(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00309
//|---Swat one iNarc pod; a failed attempt deals self-damage. H selects homing, Y haywire, E ECM.
//|---@param dbref integer
//|---@param pilot integer
//|---@param section string
//|---@param kind string
//|---@return PodRemoval
//|function btech_unit.removepod(dbref, pilot, section, kind) end
// lua-types-end

// lua-types-begin btech 00310
//|---Begin the vehicle crew's saved 60-second action; ordinary Narc pods remain attached.
//|---The assigned conscious pilot must be running and placed, with no forward motion or conflicting crew action. VTOLs must be landed (launch preparation is still landed).
//|---@param dbref integer
//|---@param pilot integer
//|---@return boolean started
//|function btech_unit.removepods(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00312
//|---Request a thirty-second stealth armor switch inside the callback transaction.
//|---@param dbref integer
//|---@param pilot integer
//|---@return boolean
//|function btech_unit.stealth(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00313
//|---Request a thirty-second null signature system switch inside the callback transaction.
//|---@param dbref integer
//|---@param pilot integer
//|---@return boolean
//|function btech_unit.nss(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00314
//|---Toggle semi-guided ammunition on a supported recycled missile launcher.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.sguided(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00315
//|---Toggle Stinger ammunition on a supported recycled missile launcher.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.stinger(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00316
//|---Select an acquired friendly spotter; use own dbref to declare spotting, or nil to stop.
//|---@param dbref integer
//|---@param pilot integer
//|---@param spotter integer?
//|---@return boolean
//|function btech_unit.spot(dbref, pilot, spotter) end
// lua-types-end

// lua-types-begin btech 00318
//|---Select valid map coordinates without requiring visibility. Unit-at-hex fire uses its current occupant; empty-hex and terrain attacks remain unimplemented.
//|---@param dbref integer
//|---@param pilot integer
//|---@param x integer
//|---@param y integer
//|---@param mode? string H/hex, B/building, I/ignite, C/clear; omitted means unit at hex.
//|---@return boolean
//|function btech_unit.lock_hex(dbref, pilot, x, y, mode) end
// lua-types-end

// lua-types-begin btech 00320
//|---Inspect empty-terrain aim without dice, expenditure, or a fictitious target unit.
//|---@param dbref integer
//|---@param weapon integer
//|---@param x integer
//|---@param y integer
//|---@return HexAimModifiers
//|function btech_unit.aim_hex(dbref, weapon, x, y) end
// lua-types-end

// lua-types-begin btech 00321
//|---Set a channel frequency without transmitting. Transactional; assigned conscious pilot required.
//|---@param dbref integer
//|---@param pilot integer
//|---@param channel integer Zero-based channel (A is 0).
//|---@param frequency integer From 0 through 999999.
//|---@return boolean
//|function btech_unit.radio_frequency(dbref, pilot, channel, frequency) end
// lua-types-end

// lua-types-begin btech 00322
//|---Save a title, truncated to fifteen bytes at a UTF-8 boundary. Transactional.
//|---@param dbref integer
//|---@param pilot integer
//|---@param channel integer Zero-based channel (A is 0).
//|---@param title string Empty clears the title.
//|---@return boolean
//|function btech_unit.radio_title(dbref, pilot, channel, title) end
// lua-types-end

// lua-types-begin btech 00323
//|---Replace channel mode: D digital, U muted, E relay; optional color letter. Transactional.
//|---Relay requires digital mode and capable hardware. Empty selects analog with no flags.
//|---@param dbref integer
//|---@param pilot integer
//|---@param channel integer Zero-based channel (A is 0).
//|---@param mode string
//|---@return boolean
//|function btech_unit.radio_mode(dbref, pilot, channel, mode) end
// lua-types-end

// lua-types-begin btech 00327
//|---Transmit using the selected channel; delivery and command mines commit together.
//|---Requires a conscious assigned cockpit pilot and no stun. Shutdown radios remain usable.
//|---@param dbref integer
//|---@param pilot integer
//|---@param channel integer Zero-based channel (A is 0).
//|---@param message string Nonempty text without control characters.
//|---@return RadioTransmission
//|function btech_unit.radio_send(dbref, pilot, channel, message) end
// lua-types-end

// lua-types-begin btech 00329
//|---Send to an acquired visible target; the source must be running and not an observer.
//|---A shutdown target receives no message. This does not use channel frequencies or detonate mines.
//|---@param dbref integer
//|---@param pilot integer
//|---@param target integer Recipient dbref.
//|---@param message string Nonempty text without control characters.
//|---@return TargetedRadioReport
//|function btech_unit.radio_target(dbref, pilot, target, message) end
// lua-types-end

// lua-types-begin btech 00330
//|---Inspect an acquired visible target without changing contacts or consuming dice.
//|---Stages a warning to running targets unless the scanning unit is an observer.
//|---Requires the conscious assigned pilot, a running unit and operational scanners.
//|---Observers bypass distance and receive exact status; ordinary scans disclose condition bands.
//|---@param dbref integer Scanner unit dbref.
//|---@param pilot integer
//|---@param target integer Target unit dbref.
//|---@param options string? A (armor), I (info), W (weapons), or a combination; omitted means all.
//|---@return string Styled scan report.
//|function btech_unit.scan(dbref, pilot, target, options) end
// lua-types-end

// lua-types-begin btech 00331
//|---Scan the first acquired visible occupant at a coordinate in saved map order.
//|---Uses the unit-scan report and warning path; empty and unacquired hexes share one reply.
//|---@param dbref integer Scanner unit dbref.
//|---@param pilot integer
//|---@param x integer Map column.
//|---@param y integer Map row.
//|---@param options string? A/I/W sections; omitted means all.
//|---@return string Styled report or empty-hex reply.
//|function btech_unit.scan_hex(dbref, pilot, x, y, options) end
// lua-types-end

// lua-types-begin btech 00333
//|---Scan a structure entrance and publish its integrity report to cockpit occupants.
//|---Hidden structures require an active in-character perception roll; invisible ones stay undetected.
//|---Dice, experience and output commit together. Explicit coordinates retain observer range limits.
//|---@param dbref integer Scanner unit dbref.
//|---@param pilot integer
//|---@param x integer Map column.
//|---@param y integer Map row.
//|---@return BuildingScan
//|function btech_unit.scan_building(dbref, pilot, x, y) end
// lua-types-end

// lua-types-begin btech 00336
//|---Scan buildings then mines in one transaction and publish both phases.
//|---Failed mine recognition is private to the pilot; success reaches cockpit occupants.
//|---@param dbref integer Scanner unit dbref.
//|---@param pilot integer
//|---@param x integer
//|---@param y integer
//|---@return HexScan
//|function btech_unit.scan_terrain(dbref, pilot, x, y) end
// lua-types-end

// lua-types-begin btech 00338
//|---Scan the saved unit or coordinate target without advancing its lock countdown.
//|---Unit reports are returned; building/hex reports also publish cockpit output.
//|---Selected coordinates allow observer distance exemptions while retaining visibility checks.
//|---@param dbref integer Scanner unit dbref.
//|---@param pilot integer
//|---@param options string? A/I/W unit report sections.
//|---@return SelectedScan
//|function btech_unit.scan_selected(dbref, pilot, options) end
// lua-types-end

// lua-types-begin btech 00339
//|---Return a silent brief report of an acquired visible unit, with no armor or weapon details.
//|---Requires a conscious assigned pilot, running unit and working scanners. Direct reports
//|---do not impose the detailed scan radius. No dice, contacts or output are changed.
//|---@param dbref integer Scanner unit dbref.
//|---@param pilot integer
//|---@param target integer Target unit dbref.
//|---@return string Styled identity, motion and position summary.
//|function btech_unit.report(dbref, pilot, target) end
// lua-types-end

// lua-types-begin btech 00341
//|---Resolve display centering only; this does not render or disclose terrain or occupants.
//|---Arguments are empty, a contact label/dbref, or integer bearing and signed distance.
//|---@param dbref integer Scanner unit dbref.
//|---@param pilot integer
//|---@param kind 'tactical'|'long_range'
//|---@param arguments string?
//|---@return ViewPosition
//|function btech_unit.view_center(dbref, pilot, kind, arguments) end
// lua-types-end

// lua-types-begin btech 00344
//|---Resolve display bounds only, without rendering or disclosing terrain/occupants.
//|---@param dbref integer Scanner unit dbref.
//|---@param pilot integer
//|---@param kind 'tactical'|'long_range'
//|---@param arguments string? Own unit, target label/dbref, or bearing and distance.
//|---@param dimensions ViewDimensions?
//|---@return Viewport
//|function btech_unit.viewport(dbref, pilot, kind, arguments, dimensions) end
// lua-types-end

// lua-types-begin btech 00346
//|---Render long-range terrain, elevation or currently visible acquired units.
//|---Mode initials match native LRS; descriptive API mode names are also accepted.
//|---Dark maps mask unseen terrain. Rendering consumes no dice and sends no notices.
//|---@param dbref integer Scanner unit dbref.
//|---@param pilot integer
//|---@param mode string First letter T/E/C/M/L/H/S/U (case insensitive), or a descriptive API mode name. U shows the terrain beneath fire and smoke.
//|---@param arguments string? Shared centering arguments.
//|---@return LongRangeMap
//|function btech_unit.lrsmap(dbref, pilot, mode, arguments) end
// lua-types-end

// lua-types-begin btech 00348
//|---Render standard, C/T (mech/tank cliffs), B (landing zones), M (mines) or L (visible) tactical maps. Fire and smoke fill the top of a hex over the terrain beneath.
//|---Uses shared cockpit/display admission; no acquisition rolls, notices or state changes.
//|---@param dbref integer Scanner unit dbref.
//|---@param pilot integer
//|---@param arguments string? Optional C/T/B/M/L flag followed by shared centering arguments.
//|---@return TacticalMap
//|function btech_unit.tactical(dbref, pilot, arguments) end
// lua-types-end

// lua-types-begin btech 00350
//|---Measure from continuous motion to the current hex center without scanner hardware.
//|---@param dbref integer
//|---@param pilot integer Conscious assigned pilot of a running unit.
//|---@return HexCenterReport
//|function btech_unit.findcenter(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00352
//|---Show the radius-two local map and units within the selected center hex.
//|---@param dbref integer
//|---@param pilot integer Conscious assigned pilot of a running unit.
//|---@param arguments string? Own unit, contact label/dbref, or bearing and distance.
//|---@return NavigationReport
//|function btech_unit.navigate(dbref, pilot, arguments) end
// lua-types-end

// lua-types-begin btech 00360
//|---List visible structures using silent identify_building locks. Failed evaluations roll back side effects.
//|---@param unit integer
//|---@param pilot integer Conscious assigned pilot of a running unit.
//|---@return BuildingContact[]
//|function btech_unit.building_contacts(unit, pilot) end
// lua-types-end

// lua-types-begin btech 00363
//|---Query unit display settings or edit A/C independently. Edits notify occupants.
//|---Requires conscious cockpit occupant; shutdown is allowed. Errors roll back state and notices.
//|---@param unit integer
//|---@param pilot integer
//|---@param arguments string? Empty for query, A 0..6 or C 0..3 for edits.
//|---@return BriefReport
//|function btech_unit.brief(unit, pilot, arguments) end
// lua-types-end

// lua-types-begin btech 00364
//|---Set whether routine contact notices include shutdown targets. Acquisition is unchanged.
//|---@param dbref integer
//|---@param player integer Assigned cockpit pilot.
//|---@param enabled boolean
//|---@return boolean
//|function btech_unit.autocon_shutdown(dbref, player, enabled) end
// lua-types-end

// lua-types-begin btech 00365
//|---Request a six-second lateral change; requires an intact quad and its assigned pilot.
//|---@param dbref integer
//|---@param player integer
//|---@param direction string nw/fl, ne/fr, sw/rl, se/rr, or - to travel straight.
//|---@return Notice
//|function btech_unit.lateral(dbref, player, direction) end
// lua-types-end

// lua-types-begin btech 00367
//|---Pivot left/right on a piloting check; a failed attempt falls.
//|---@param dbref integer
//|---@param player integer
//|---@param direction string
//|---@return BootleggerReport
//|function btech_unit.bootlegger(dbref, player, direction) end
// lua-types-end

// lua-types-begin btech 00369
//|---Estimate travel to explicit x y or the selected ordinary hex and notify cockpit occupants.
//|---@param dbref integer
//|---@param player integer
//|---@param coordinates string?
//|---@return EtaReport
//|function btech_unit.eta(dbref, player, coordinates) end
// lua-types-end

// lua-types-begin btech 00371
//|---Read a compass bearing to the default target, x y, or x0 y0 x1 y1.
//|---@param dbref integer
//|---@param player integer
//|---@param coordinates string?
//|---@return BearingReport
//|function btech_unit.bearing(dbref, player, coordinates) end
// lua-types-end

// lua-types-begin btech 00373
//|---Read range to default target, x y, or between x0 y0 and x1 y1.
//|---@param dbref integer
//|---@param player integer
//|---@param coordinates string?
//|---@return RangeReport
//|function btech_unit.range_report(dbref, player, coordinates) end
// lua-types-end

// lua-types-begin btech 00375
//|---Measure a default target, destination x/y[/z], or origin and destination x/y[/z].
//|---@param dbref integer
//|---@param player integer
//|---@param coordinates string?
//|---@return VectorReport
//|function btech_unit.vector(dbref, player, coordinates) end
// lua-types-end

// lua-types-begin btech 00376
//|---Start or stop ammunition dumping; weapon numbers are zero based and slots one based.
//|---@param dbref integer
//|---@param player integer
//|---@param selection string
//|---@return table[] notices
//|function btech_unit.dump(dbref, player, selection) end
// lua-types-end

// lua-types-begin btech 00378
//|---Toggle MASC and adjust the desired throttle proportionally.
//|---@param dbref integer
//|---@param pilot integer
//|---@return table notice
//|function btech_unit.masc(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00379
//|---Toggle the supercharger and adjust the desired throttle proportionally.
//|---@param dbref integer
//|---@param pilot integer
//|---@return table notice
//|function btech_unit.supercharger(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00380
//|---Join a visible friendly unit's C3i network by battlefield ID, or leave with "-".
//|---@param dbref integer
//|---@param pilot integer
//|---@param target string
//|---@return Notice[]|nil
//|---@return table|nil error
//|function btech_unit.c3i(dbref, pilot, target) end
// lua-types-end

// lua-types-begin btech 00381
//|---Send text to available C3i peers and echo it to your cockpit. Requires an active transaction.
//|---@param dbref integer
//|---@param pilot integer
//|---@param message string
//|---@return Notice[]|nil
//|---@return table|nil error
//|function btech_unit.c3i_message(dbref, pilot, message) end
// lua-types-end

// lua-types-begin btech 00383
//|---Inspect running, unjammed peers without requiring visual contact or publishing output.
//|---@param dbref integer
//|---@param pilot integer
//|---@return {rows: NetworkStatusRow[], text: string}|nil
//|---@return table|nil error
//|function btech_unit.c3i_network(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00384
//|---Join a visible friendly classic C3 network, or leave with "-". Capacity depends on working masters.
//|---@param dbref integer
//|---@param pilot integer
//|---@param target string
//|---@return Notice[]|nil
//|---@return table|nil error
//|function btech_unit.c3(dbref, pilot, target) end
// lua-types-end

// lua-types-begin btech 00385
//|---Send to available classic C3 peers using current master capacity; echo to your cockpit.
//|---@param dbref integer
//|---@param pilot integer
//|---@param message string
//|---@return Notice[]|nil
//|---@return table|nil error
//|function btech_unit.c3_message(dbref, pilot, message) end
// lua-types-end

// lua-types-begin btech 00386
//|---Inspect classic C3 peers using active master capacity; emits no messages.
//|---@param dbref integer
//|---@param pilot integer
//|---@return {rows: NetworkStatusRow[], text: string}|nil
//|---@return table|nil error
//|function btech_unit.c3_network(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00387
//|---Inspect or set a running vehicle turret's absolute heading. Set accepts integer degrees. Transactional.
//|---@param dbref integer
//|---@param pilot integer
//|---@param heading integer|nil
//|---@return number|boolean
//|function btech_unit.turret(dbref, pilot, heading) end
// lua-types-end

// lua-types-begin btech 00388
//|--- Begin a 60-second turret repair, blocking fire until pending attempts finish.
//|---@param unit integer
//|---@param pilot integer
//|---@return boolean
//|function btech_unit.fixturret(unit, pilot) end
// lua-types-end

// lua-types-begin btech 00389
//|---Begin a two-minute attempt to put out vehicle section fires while shut down.
//|---@param dbref integer
//|---@param pilot integer
//|---@return boolean
//|function btech_unit.extinguish(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00390
//|---Pick up a visible unit using shared towing, shutdown and terrain rules.
//|---@param dbref integer Carrier unit.
//|---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
//|---@param target integer Target unit.
//|---@return boolean
//|function btech_unit.pickup(dbref, pilot, target) end
// lua-types-end

// lua-types-begin btech 00391
//|---Release the carrier's tow; elevated targets begin forced descent.
//|---@param dbref integer Carrier unit.
//|---@param pilot integer Conscious assigned pilot.
//|---@return boolean
//|function btech_unit.dropoff(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00392
//|---Inspect or set scenario permission to tow this unit out of character.
//|---Trusted scripts own authorization for edits; this is not a pilot preference.
//|---Disabling permission does not release an existing tow.
//|---@param dbref integer
//|---@param enabled boolean? Omit to inspect without changing state.
//|---@return boolean Current permission.
//|function btech_unit.towable(dbref, enabled) end
// lua-types-end

// lua-types-begin btech 00393
//|---Begin twenty seconds of digging in a stopped tracked or wheeled vehicle.
//|---Completed cover permits only turret weapons; requesting movement leaves cover.
//|---@param dbref integer
//|---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
//|---@return boolean
//|function btech_unit.dig(dbref, pilot) end
// lua-types-end

// lua-types-begin btech 00394
//|---Lower a quad, raise it with "-", or cancel its pending change with "stop".
//|---@param dbref integer
//|---@param pilot integer Conscious assigned pilot; scripts own authority to act for them.
//|---@param argument string? Omit to lower.
//|---@return boolean
//|function btech_unit.hulldown(dbref, pilot, argument) end
// lua-types-end

// lua-types-begin btech 00395
//|---Inspect or set scenario fortification. Trusted scripts own authorization.
//|---Enabling requires settled motion, no tow relationship, no building-entry request,
//|---and a landed unit. Disabling does not restart any action.
//|---@param dbref integer
//|---@param enabled boolean? Omit to inspect.
//|---@return boolean Current fortification state.
//|function btech_unit.fortified(dbref, enabled) end
// lua-types-end

// lua-types-begin btech 00396
//|---Inspect or set observer role. Trusted scripts own authorization; cockpit pilots cannot grant this role.
//|---@param dbref integer
//|---@param enabled boolean? Omit to inspect the saved role.
//|---@return boolean
//|function btech_unit.observer(dbref, enabled) end
// lua-types-end

// lua-types-begin btech 00397
//|---Read or change operator weapons hold inside a trusted callback transaction.
//|---Hold blocks fire and TIC admission before argument decoding or loss of cover.
//|---The setting persists through shutdown and restart; aborted callbacks restore it.
//|---@param dbref integer Constructed unit dbref.
//|---@param enabled boolean|nil Omit to inspect without changing the setting.
//|---@return boolean enabled
//|function btech_unit.weapons_hold(dbref, enabled) end
// lua-types-end

// lua-types-begin btech 00398
//|---Detonate a Mech reactor in a trusted callback; damage, sensor flashes and casualties commit together.
//|---This scenario action bypasses cockpit self-destruct configuration and countdown admission.
//|---@param dbref integer
//|---@return table report
//|function btech_unit.reactor_explode(dbref) end
// lua-types-end

// lua-types-begin btech 00399
//|---Start or stop cockpit self-destruction. Engagement releases the pilot assignment.
//|---The same argument grammar, configuration and override checks apply as the native explode command.
//|---@param dbref integer
//|---@param pilot integer
//|---@param argument string "ammo", "reactor", "stop", optionally followed by wizard "override".
//|---@return boolean
//|function btech_unit.explode(dbref, pilot, argument) end
// lua-types-end

// lua-types-begin btech 00400
//|---Set scenario protection from new ammunition self-destruct requests; admitted timers continue.
//|---@param dbref integer
//|---@param enabled boolean
//|---@return boolean
//|function btech_unit.explode_safe(dbref, enabled) end
// lua-types-end

// lua-types-begin btech 00401
//|---Read or replace trusted scenario visibility. Both fields are required when replacing it.
//|---Clairvoyance bypasses visibility checks; ordinary sensor acquisition still rejects invisible targets.
//|---@param dbref integer
//|---@param flags {invisible: boolean, clairvoyant: boolean}?
//|---@return {invisible: boolean, clairvoyant: boolean}
//|function btech_unit.visibility(dbref, flags) end
// lua-types-end

// lua-types-begin btech 00402
//|---Read or change scenario combat immunity in a trusted callback transaction.
//|---@param dbref integer
//|---@param enabled boolean|nil Omit to inspect.
//|---@return boolean
//|function btech_unit.combat_safe(dbref, enabled) end
// lua-types-end

// lua-types-begin btech 00403
//|---Select Swarm missiles; unused missiles can retarget friendly units, including the launcher.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.fireswarm(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00404
//|---Select Swarm-1 missiles; secondary targets must belong to another team.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.fireswarm1(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00407
//|---Alias of cluster: select artillery cluster rounds, rejecting a different selected artillery payload.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.firecluster(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00408
//|---Select missile Smoke rounds. This cockpit control does not select artillery payloads.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.firesmoke(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00409
//|---Select missile Mine (Thunder) rounds. They bypass AMS and retain ordinary missile damage
//|---against units; a hit on a hex lays a minefield there.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.firemine(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00548
//|---Select Thunder-Augmented rounds, which mine a target hex and its neighbors at half strength.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.fireaugmented(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00549
//|---Select Thunder-Vibrabomb rounds, which lay a field that trips under units heavier than the shooter.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.firevibrabomb(dbref, pilot, weapon) end
// lua-types-end

// lua-types-begin btech 00550
//|---Select Thunder-Active rounds, whose mines also catch units hovering just above the ground.
//|---@param dbref integer
//|---@param pilot integer
//|---@param weapon integer
//|---@return AmmunitionMode
//|function btech_unit.fireactive(dbref, pilot, weapon) end
// lua-types-end


// lua-types-begin btech 00418
//|---Wizard-only predictive firing using fixed horizontal target orders and normal weapon launches.
//|---Sets the cockpit hex target. Does not simulate future damage or order changes.
//|---@param dbref integer Shooter unit
//|---@param player integer Assigned wizard pilot
//|---@param target integer Target unit on the same battlefield
//|---@param selection string Comma-separated weapon numbers and inclusive ranges
//|---@return boolean success
//|function btech_unit.snipe(dbref, player, target, selection) end
// lua-types-end

// lua-types-begin btech 00421
//|---Wizard field inspection using optional 1/4 column selector and case-insensitive prefix.
//|---@param actor integer
//|---@param unit integer
//|---@param arguments? string
//|---@return UnitFieldReport
//|function btech_unit.fields(actor, unit, arguments) end
// lua-types-end

// lua-types-begin btech 00422
//|---Wizard named edits; accepts identity, team, xpmod, VTOL fuel, sensor/radio hardware and Mech thermal fields.
//|---@param actor integer
//|---@param unit integer
//|---@param field string
//|---@param value string
//|function btech_unit.set_field(actor, unit, field, value) end
// lua-types-end

// lua-types-begin btech 00475
//|---Install one technology code on the unit.
//|---@param unit DbRef|Object
//|---@param technology TechnologyCode Typed constant from btech.unit.technology.
//|function btech_unit.add_technology(unit, technology) end
// lua-types-end

// lua-types-begin btech 00476
//|---Remove one installed technology code.
//|---@param unit DbRef|Object
//|---@param technology TechnologyCode Typed constant from btech.unit.technology.
//|function btech_unit.remove_technology(unit, technology) end
// lua-types-end

// lua-types-begin btech 00477
//|---Remove every technology in one group.
//|---@param unit DbRef|Object
//|---@param group TechnologyGroup Typed constant from btech.unit.technology_groups.
//|function btech_unit.clear_technologies(unit, group) end
// lua-types-end

// lua-types-begin btech 00478
//|---List configured and inferred unit technologies.
//|---@param unit DbRef|Object
//|---@return Technology[] technologies
//|function btech_unit.technologies(unit) end
// lua-types-end

// lua-types-begin btech 00479
//|---Apply a C-contract damage request to a live unit.
//|---@param unit DbRef|Object
//|---@param request table Damage request record.
//|function btech_unit.apply_damage(unit, request) end
// lua-types-end

// lua-types-begin btech 00480
//|---Read current, original and rear armor values; an omitted section reports the totals.
//|---@param unit DbRef|Object
//|---@param section? MechSection Typed section constant from btech.unit.sections.
//|---@return ArmorStatus status
//|function btech_unit.armor(unit, section) end
// lua-types-end

// lua-types-begin btech 00481
//|---Read the assigned pilot object, or nil when the cockpit is unassigned.
//|---@param unit DbRef|Object
//|---@return Object|nil pilot
//|function btech_unit.assigned_pilot(unit) end
// lua-types-end

// lua-types-begin btech 00482
//|---Read offensive, defensive and total Battle Value.
//|---@param unit DbRef|Object
//|---@return BattleValue value
//|function btech_unit.battle_value(unit) end
// lua-types-end

// lua-types-begin btech 00483
//|---List one section's critical slots with resolved parts, modes and ammunition state.
//|---@param unit DbRef|Object
//|---@param section MechSection Typed section constant from btech.unit.sections.
//|---@return CriticalSlot[] slots
//|function btech_unit.critical_slots(unit, section) end
// lua-types-end

// lua-types-begin btech 00484
//|---Read the engine rating and suspension factor.
//|---@param unit DbRef|Object
//|---@return Engine engine
//|function btech_unit.engine(unit) end
// lua-types-end

// lua-types-begin btech 00485
//|---List installed equipment in catalogue order.
//|---@param unit DbRef|Object
//|---@return PartStack[] parts
//|function btech_unit.installed_parts(unit) end
// lua-types-end

// lua-types-begin btech 00486
//|---List carried ammunition stock in catalogue order.
//|---@param unit DbRef|Object
//|---@return PartStack[] parts
//|function btech_unit.payload(unit) end
// lua-types-end

// lua-types-begin btech 00487
//|---Replace the unit definition from a saved template reference.
//|---@param unit DbRef|Object
//|---@param reference string Template reference: the file stem of a `.toml` document anywhere under database.mech_database.
//|function btech_unit.load_template(unit, reference) end
// lua-types-end

// lua-types-begin btech 00488
//|---Save the unit definition under a template reference in the mech database.
//|---@param unit DbRef|Object
//|---@param reference string Template reference: the file stem of a `.toml` document anywhere under database.mech_database.
//|function btech_unit.save_template(unit, reference) end
// lua-types-end

// lua-types-begin btech 00489
//|---Run one shared piloting check; returns whether it succeeded.
//|---@param unit DbRef|Object
//|---@param options table Situational modifier request.
//|---@return boolean succeeded
//|function btech_unit.piloting_check(unit, options) end
// lua-types-end

// lua-types-begin btech 00490
//|---Read the saved two-letter battlefield ID preference, or nil when unset.
//|---@param unit DbRef|Object
//|---@return string|nil id
//|function btech_unit.preferred_id(unit) end
// lua-types-end

// lua-types-begin btech 00491
//|---List configured radio channels with active mode names.
//|---@param unit DbRef|Object
//|---@return RadioChannelReport[] channels
//|function btech_unit.radio_channels(unit) end
// lua-types-end

// lua-types-begin btech 00492
//|---Restore destroyed critical slots to their original equipment.
//|---@param unit DbRef|Object
//|function btech_unit.reset_critical_slots(unit) end
// lua-types-end

// lua-types-begin btech 00493
//|---Refill one ammunition bin to its installed capacity.
//|---@param unit DbRef|Object
//|---@param section MechSection Typed section constant from btech.unit.sections.
//|---@param slot integer One-based critical slot.
//|function btech_unit.restock_ammunition(unit, section, slot) end
// lua-types-end

// lua-types-begin btech 00494
//|---Restore armor, internal structure, critical slots and ammunition to template values.
//|---@param unit DbRef|Object
//|function btech_unit.restore(unit) end
// lua-types-end

// lua-types-begin btech 00495
//|---Read a section's damage condition.
//|---@param unit DbRef|Object
//|---@param section MechSection Typed section constant from btech.unit.sections.
//|---@return "operational"|"destroyed"|"flooded" condition
//|function btech_unit.section_condition(unit, section) end
// lua-types-end

// lua-types-begin btech 00500
//|---Install a weapon into explicit critical slots.
//|---@param unit DbRef|Object
//|---@param request WeaponInstall
//|function btech_unit.install_weapon(unit, request) end
// lua-types-end

// lua-types-begin btech 00501
//|---Install or clear non-weapon equipment in one critical slot.
//|---@param unit DbRef|Object
//|---@param request SpecialInstall
//|function btech_unit.install_special(unit, request) end
// lua-types-end

// lua-types-begin btech 00502
//|---Configure one ammunition bin's half-ton flag and selected modes.
//|---@param unit DbRef|Object
//|---@param request AmmunitionConfiguration
//|function btech_unit.configure_ammunition(unit, request) end
// lua-types-end

// lua-types-begin btech 00503
//|---Replace the selected fire and ammunition modes of one mounted weapon.
//|---@param unit DbRef|Object
//|---@param weapon_number integer Zero-based stable weapon number.
//|---@param modes WeaponModes
//|function btech_unit.set_weapon_modes(unit, weapon_number, modes) end
// lua-types-end

// lua-types-begin btech 00504
//|---Patch armor values on one section.
//|---@param unit DbRef|Object
//|---@param section MechSection Typed section constant from btech.unit.sections.
//|---@param patch table Current-armor, internal or rear-armor integers, each 0 through 255.
//|function btech_unit.set_armor(unit, section, patch) end
// lua-types-end

// lua-types-begin btech 00505
//|---Assign or clear the saved pilot; the player need not enter the cockpit.
//|---@param unit DbRef|Object
//|---@param pilot DbRef|Object|nil Player object.
//|function btech_unit.set_assigned_pilot(unit, pilot) end
// lua-types-end

// lua-types-begin btech 00506
//|---Set cargo space and the maximum carried tonnage.
//|---@param unit DbRef|Object
//|---@param space integer
//|---@param maximum_tons integer
//|function btech_unit.set_cargo_capacity(unit, space, maximum_tons) end
// lua-types-end

// lua-types-begin btech 00507
//|---Set the installed heat-sink count.
//|---@param unit DbRef|Object
//|---@param count integer
//|function btech_unit.set_heat_sinks(unit, count) end
// lua-types-end

// lua-types-begin btech 00508
//|---Set the jump speed in movement points.
//|---@param unit DbRef|Object
//|---@param movement_points number
//|function btech_unit.set_jump_speed(unit, movement_points) end
// lua-types-end

// lua-types-begin btech 00509
//|---Set the long-range sensor ceiling in hexes.
//|---@param unit DbRef|Object
//|---@param range integer
//|function btech_unit.set_long_range_sensor_range(unit, range) end
// lua-types-end

// lua-types-begin btech 00510
//|---Set the tactical sensor range in hexes.
//|---@param unit DbRef|Object
//|---@param range integer
//|function btech_unit.set_tactical_range(unit, range) end
// lua-types-end

// lua-types-begin btech 00511
//|---Set the scan range in hexes.
//|---@param unit DbRef|Object
//|---@param range integer
//|function btech_unit.set_scan_range(unit, range) end
// lua-types-end

// lua-types-begin btech 00512
//|---Set the radio range in hexes.
//|---@param unit DbRef|Object
//|---@param range integer
//|function btech_unit.set_radio_range(unit, range) end
// lua-types-end

// lua-types-begin btech 00513
//|---Set the maximum ground speed in movement points.
//|---@param unit DbRef|Object
//|---@param movement_points number
//|function btech_unit.set_max_speed(unit, movement_points) end
// lua-types-end

// lua-types-begin btech 00514
//|---Replace the movement class.
//|---@param unit DbRef|Object
//|---@param movement_type MovementType Typed constant from btech.unit.movement_types.
//|function btech_unit.set_movement_type(unit, movement_type) end
// lua-types-end

// lua-types-begin btech 00515
//|---Set the unit tonnage.
//|---@param unit DbRef|Object
//|---@param tons integer
//|function btech_unit.set_tonnage(unit, tons) end
// lua-types-end

// lua-types-begin btech 00516
//|---Replace the unit class.
//|---@param unit DbRef|Object
//|---@param unit_type UnitType Typed constant from btech.unit.types.
//|function btech_unit.set_unit_type(unit, unit_type) end
// lua-types-end

// lua-types-begin btech 00517
//|---Set the radio quality grade.
//|---@param unit DbRef|Object
//|---@param quality integer
//|function btech_unit.set_radio_quality(unit, quality) end
// lua-types-end

// lua-types-begin btech 00518
//|---Remove the object's BattleTech registration and forget its configuration
//|---references. Rust extension without a C Lua counterpart: the reference exposes
//|---teardown only through the native wizard command, and this binding shares that
//|---command's teardown exactly. Succeeds silently for an already-plain object and
//|---never moves or destroys the container thing; mutations join the surrounding
//|---callback transaction.
//|---@param unit DbRef|Object Live thing to tear down.
//|---@return boolean true
//|function btech_unit.unregister(unit) end
// lua-types-end

// lua-types-begin btech 00519
//|---Read the damage-adjusted maximum speed in movement points.
//|---@param unit DbRef|Object
//|---@return number movement_points
//|function btech_unit.effective_max_speed(unit) end
// lua-types-end

// lua-types-begin btech 00520
//|---Read the damage-adjusted maximum speed in kilometers per hour.
//|---@param unit DbRef|Object
//|---@return number kilometers_per_hour
//|function btech_unit.effective_max_speed_kph(unit) end
// lua-types-end

// lua-types-begin btech 00521
//|---List the weapons of one trigger group in mounting order.
//|---@param unit DbRef|Object
//|---@param tic integer Group number from 0 through 3.
//|---@return MountedWeapon[] weapons
//|function btech_unit.tic_weapons(unit, tic) end
// lua-types-end
