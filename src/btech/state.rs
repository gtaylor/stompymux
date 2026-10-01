//! Transactional maps and saved special-object identities, with shared immutable terrain.
use super::{BattleHex, BattleMapAsset, BattleTemplate, BattleUnit};
use crate::{Kind, ObjectId, SharedMap, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Identity and dimensions of a saved map. Encoded tiles are not interpreted without a dictionary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredBattleMap {
    /// Allocated membership span, including holes retained after removal.
    #[serde(default)]
    pub(crate) membership_extent: u32,
    /// Optional loading location and its coordinate disclosure policy.
    #[serde(default)]
    pub(crate) cargo_transfer_point: Option<super::BattleCargoTransferPoint>,
    /// Admitted rounds in stable launch order.
    #[serde(default)]
    pub(crate) artillery_shots: Arc<BTreeMap<u32, super::BattleArtilleryShot>>,
    /// Parent recorded by link rebuilding; zero means cleared. Independent of return routes.
    #[serde(default)]
    pub building_parent: i64,
    pub name: String,
    pub width: i64,
    pub height: i64,
    pub gravity: i64,
    pub temperature: i64,
    /// Environmental flags stored alongside the map identity.
    pub flags: i64,
    /// Construction integrity and policy belong to this interior map.
    #[serde(default)]
    pub building: super::BattleBuildingState,
    /// Committed seconds until the next construction repair event.
    #[serde(default)]
    pub building_repair: Option<u16>,
    /// Stable mine definitions, separate from visual terrain overlays.
    #[serde(default)]
    pub(crate) minefields: Arc<BTreeMap<u32, super::BattleMinefield>>,
    /// Traversal order is independent of the persistent record identifiers.
    #[serde(default)]
    pub(crate) minefield_order: Arc<Vec<u32>>,
    /// Circular team-aware landing restrictions.
    #[serde(default)]
    pub(crate) landing_exclusions: Arc<BTreeMap<u32, super::BattleLandingExclusion>>,
    /// Display and deletion traversal, independent of persistent restriction identities.
    #[serde(default)]
    pub(crate) landing_exclusion_order: Arc<Vec<u32>>,
    /// Ordered entrances pointing to interior maps.
    #[serde(default)]
    pub(crate) building_entrances: Arc<BTreeMap<u32, super::BattleBuildingEntrance>>,
    /// Ordered interior arrival points selected by a direction code.
    #[serde(default)]
    pub(crate) building_entry_points: Arc<BTreeMap<u32, super::BattleBuildingEntryPoint>>,
    /// Ordered return-map links; the first link is the active exit.
    #[serde(default)]
    pub(crate) building_exits: Arc<BTreeMap<u32, super::BattleBuildingExit>>,
    /// Authored configuration used to rebuild runtime routes.
    #[serde(default)]
    pub(crate) authored_link: Option<super::BattleMapLink>,
    /// Saved map movement percentage; zero or negative uses the standard rate.
    #[serde(default)]
    pub movement_modifier: i64,
    /// Linked-map marker coordinates; any marker enables opposite-edge wrapping.
    #[serde(default)]
    pub(crate) linked_markers: Arc<BTreeMap<u32, super::BattleLinkedMarker>>,
    /// Battlefield illumination, weather visibility and saved sensor range ceiling.
    pub light: i64,
    pub visibility: i64,
    pub maximum_visibility: i64,
    /// Cloud boundary in elevation levels; zero disables cloud obstruction.
    pub cloud_base: i16,
    /// Perception channels switched off for this battlefield; see `BattleMapPerceptionFlag`.
    pub sensor_flags: i64,
    /// Persisted wind bearing in degrees and strength used by terrain effects.
    #[serde(default)]
    pub wind_direction: i64,
    #[serde(default)]
    pub wind_speed: i64,
    /// Map-owned replayable stream established with decoded map creation.
    #[serde(default)]
    pub(crate) fire_dice: Option<super::BattleDice>,

    /// Absent until the terrain dictionary has been explicitly established.
    #[serde(default)]
    pub(crate) terrain: Option<Arc<Vec<BattleHex>>>,
    /// Sparse overlays indexed by row-major tile position, independent of source terrain.
    #[serde(default)]
    pub(crate) decorations: Arc<BTreeMap<u32, super::BattleDecoration>>,
    /// Generic saved records carry restoration terrain but no autonomous timers.
    #[serde(default)]
    pub(crate) static_decorations: [Arc<BTreeMap<u32, super::BattleStaticDecoration>>; 3],
}

impl StoredBattleMap {
    /// The saved map restriction blocks non-coolant fire between teammates.
    pub fn blocks_friendly_fire(&self) -> bool {
        self.has_flag(super::BattleMapFlag::NoFriendlyFire)
    }

    /// Environmental rules are enabled by the map's persisted special-conditions flag.
    pub fn uses_special_rules(&self) -> bool {
        self.has_flag(super::BattleMapFlag::SpecialRules)
    }

    /// Whether every tile has a known terrain/elevation interpretation.
    pub fn terrain_ready(&self) -> bool {
        self.terrain.is_some()
    }

    /// Inspect a decoded tile, rejecting ambiguous maps and invalid coordinates.
    pub fn hex(&self, x: i64, y: i64) -> Result<BattleHex> {
        let mut hex = self.stored_hex(x, y)?;
        if let Some(effect) = self.decorations.get(&((y * self.width + x) as u32)) {
            hex.terrain = effect.kind.terrain();
        }
        Ok(hex)
    }

    /// Inspect the underlying tile without a transient fire or smoke marker.
    pub fn base_hex(&self, x: i64, y: i64) -> Result<BattleHex> {
        let mut hex = self.stored_hex(x, y)?;
        if matches!(hex.terrain, super::Terrain::Fire | super::Terrain::Smoke)
            && let Some(record) = self
                .static_decorations
                .iter()
                .flat_map(|records| records.values())
                .find(|record| {
                    i64::from(record.coordinate.x) == x && i64::from(record.coordinate.y) == y
                })
        {
            hex.terrain = record.restored_terrain;
        }
        Ok(hex)
    }

    /// Read the terrain dictionary without overlay or restoration lookup.
    pub(crate) fn stored_hex(&self, x: i64, y: i64) -> Result<BattleHex> {
        let terrain = self
            .terrain
            .as_ref()
            .context("Map terrain is ambiguous; explicitly reload its map asset first")?;
        ensure!(
            x >= 0 && x < self.width && y >= 0 && y < self.height,
            "Map coordinates out of bounds"
        );
        let index = y
            .checked_mul(self.width)
            .and_then(|offset| offset.checked_add(x))
            .and_then(|index| usize::try_from(index).ok())
            .context("Map coordinates out of bounds")?;
        terrain
            .get(index)
            .copied()
            .context("Incomplete map terrain")
    }

    /// Check the complete decoded map before it can participate in a world transaction.
    pub(crate) fn validate(&self) -> Result<()> {
        let _measurement = super::autopilot::diagnostics::combat("validation_map");
        self.building.validate()?;
        if let Some(point) = self.cargo_transfer_point {
            point.validate(self)?;
        }
        self.validate_artillery()?;
        ensure!(
            self.landing_exclusion_order.len() == self.landing_exclusions.len()
                && self
                    .landing_exclusion_order
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    == self.landing_exclusions.keys().copied().collect(),
            "Invalid landing exclusion order"
        );
        ensure!(
            self.landing_exclusions.len() <= 1_000_000
                && self
                    .landing_exclusions
                    .values()
                    .all(|zone| zone.coordinate.x >= 0
                        && zone.coordinate.y >= 0
                        && i64::from(zone.coordinate.x) < self.width
                        && i64::from(zone.coordinate.y) < self.height
                        && zone.owner.0 >= -1),
            "Invalid landing exclusions"
        );
        ensure!(
            self.minefield_order.len() == self.minefields.len()
                && self
                    .minefield_order
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    == self.minefields.keys().copied().collect(),
            "Invalid minefield order"
        );
        ensure!(
            self.minefields.len() <= 1_000_000
                && self.minefields.values().all(|mine| mine.coordinate.x >= 0
                    && mine.coordinate.y >= 0
                    && i64::from(mine.coordinate.x) < self.width
                    && i64::from(mine.coordinate.y) < self.height
                    && mine.owner.0 >= -1),
            "Invalid minefield definitions"
        );
        ensure!(
            self.building_repair
                .is_none_or(|remaining| (1..=120).contains(&remaining)),
            "Invalid building repair countdown"
        );
        ensure!(
            self.building_entrances.len() <= 1_000_000
                && self
                    .building_entrances
                    .values()
                    .all(|entrance| entrance.coordinate.x >= 0
                        && entrance.coordinate.y >= 0
                        && i64::from(entrance.coordinate.x) < self.width
                        && i64::from(entrance.coordinate.y) < self.height),
            "Invalid building entrances"
        );
        ensure!(
            self.building_entry_points.len() <= 1_000_000 && self.building_exits.len() <= 1_000_000,
            "Too many building routes"
        );
        for point in self.building_entry_points.values() {
            ensure!(
                point.coordinate.x >= 0
                    && point.coordinate.y >= 0
                    && i64::from(point.coordinate.x) < self.width
                    && i64::from(point.coordinate.y) < self.height,
                "Invalid building entry point"
            );
        }
        ensure!(
            self.linked_markers.len() <= 1_000_000,
            "Too many linked markers"
        );
        ensure!(
            self.static_decorations
                .iter()
                .all(|records| records.len() <= 1_000_000),
            "Too many generic decorations"
        );
        for decoration in self
            .static_decorations
            .iter()
            .flat_map(|records| records.values())
        {
            ensure!(
                decoration.coordinate.x >= 0
                    && decoration.coordinate.y >= 0
                    && i64::from(decoration.coordinate.x) < self.width
                    && i64::from(decoration.coordinate.y) < self.height,
                "Invalid generic decoration coordinate"
            );
        }
        let Some(terrain) = &self.terrain else {
            ensure!(
                self.decorations.is_empty(),
                "Decorations require decoded terrain"
            );
            return Ok(());
        };
        ensure!(
            (0..=2).contains(&self.light)
                && (0..=60).contains(&self.visibility)
                && (0..=i64::from(i16::MAX)).contains(&self.maximum_visibility),
            "Invalid map visibility conditions"
        );
        ensure!(
            (1..=1000).contains(&self.width) && (1..=1000).contains(&self.height),
            "Invalid map dimensions"
        );
        ensure!(
            terrain.len() == (self.width * self.height) as usize,
            "Incomplete map terrain"
        );
        ensure!(
            self.fire_dice.is_some()
                || !self
                    .decorations
                    .values()
                    .any(|effect| effect.kind == super::BattleDecorationKind::Fire),
            "Fire requires a saved map random stream"
        );
        ensure!(
            self.decorations
                .iter()
                .all(|(&index, effect)| (index as usize) < terrain.len() && effect.valid()),
            "Invalid map decoration"
        );
        ensure!(
            self.decorations
                .values()
                .map(|effect| (
                    effect.kind == super::BattleDecorationKind::Fire,
                    effect.order
                ))
                .collect::<BTreeSet<_>>()
                .len()
                == self.decorations.len(),
            "Duplicate decoration creation order"
        );
        ensure!(
            terrain.iter().all(|hex| hex.elevation <= 9),
            "Invalid map elevation"
        );
        ensure!(
            (0..=255).contains(&self.gravity) && (-128..=127).contains(&self.temperature),
            "Invalid map environment"
        );
        ensure!(
            (0..360).contains(&self.wind_direction)
                && (0..=i64::from(i16::MAX)).contains(&self.wind_speed),
            "Invalid map wind"
        );
        ensure!(i32::try_from(self.flags).is_ok(), "Invalid map flags");
        ensure!(
            i32::try_from(self.sensor_flags).is_ok(),
            "Invalid sensor flags"
        );
        Ok(())
    }
}

/// Saved unit identity; class/movement codes remain observable for deferred unit classes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredBattleUnit {
    pub name: String,
    pub template: String,
    pub class_code: i64,
    pub movement_code: i64,
    pub tons: i64,
    pub map: Option<ObjectId>,
}

/// Copy-on-write domain state shared by world checkpoints.
/// Registration is not a claim that a unit's behavior is implemented.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BtechState {
    /// Runtime-only per-context template registry. Reloaded state scans lazily.
    #[serde(skip)]
    pub(crate) template_registry: super::TemplateRegistryCache,
    /// Runtime-only sanction marking MECH roles retired by the `@btech
    /// unregister` command, letting persistence distinguish command-driven
    /// teardown from accidental state loss. Cleared when a save commits.
    #[serde(skip)]
    pub(crate) retire_sanctions: Arc<std::cell::RefCell<BTreeSet<ObjectId>>>,
    /// Independent, insertion-ordered computer display recovery events.
    #[serde(default)]
    pub(crate) sensor_recoveries: Arc<Vec<super::computer_runtime::SensorRecovery>>,
    /// Shared committed phase for turn-boundary rules.
    #[serde(default)]
    pub(crate) turn_clock: super::turn_clock::TurnClock,
    /// Committed simulation seconds; never advanced by wall time or loading.
    #[serde(default)]
    pub(crate) simulation_seconds: i64,
    /// Ground autopilot intent keyed by the directly controlled unit.
    #[serde(default)]
    pub(crate) controllers: SharedMap<ObjectId, super::autopilot::AutopilotController>,
    /// Runtime-only path searches and steering cursors; rebuilt from active orders after load.
    #[serde(skip)]
    pub(crate) autopilot_plans: SharedMap<ObjectId, super::autopilot::runtime::AutopilotPlan>,
    /// Loose parts shared by rooms, units and other game objects.
    #[serde(default)]
    pub(crate) inventories: SharedMap<ObjectId, Vec<super::BattleInventoryEntry>>,
    #[serde(default)]
    pub(crate) part_costs: Arc<BTreeMap<i32, u64>>,
    /// Runtime catalogue overrides reset on reload; saved countdowns retain their remaining time.
    #[serde(default)]
    pub(crate) weapon_settings: super::BattleWeaponSettings,
    /// Seconds until a fully destroyed native unit retires from the battlefield.
    #[serde(default)]
    pub(crate) wrecks: Arc<BTreeMap<ObjectId, u8>>,
    #[serde(default)]
    pub(crate) reactor: super::reactor_instability::BattleReactorState,
    /// External towing is one chassis-independent carrier-to-target relationship.
    #[serde(default)]
    pub(crate) tows: Arc<BTreeMap<ObjectId, ObjectId>>,
    /// Saved player map dimensions and contact-list categories.
    #[serde(default)]
    pub(crate) player_preferences: SharedMap<ObjectId, super::BattlePlayerPreferences>,
    /// Player-owned template and personal-combat configuration; UI configuration is tracked
    /// independently by presence in `player_preferences`.
    #[serde(default)]
    pub(crate) player_configuration: SharedMap<ObjectId, super::BattlePlayerConfiguration>,
    #[serde(default)]
    pub(crate) unit_configuration: SharedMap<ObjectId, super::BattleUnitConfiguration>,
    /// Runtime sensor band reach supplied by the host configuration; not stored in database tables.
    #[serde(default)]
    pub(crate) sensor_range: super::BattleSensorRange,
    /// Runtime skill threshold overrides; database reload starts with catalog defaults.
    #[serde(default)]
    pub(crate) skill_thresholds: Arc<BTreeMap<String, u32>>,
    #[serde(default)]
    pub(crate) character_values: SharedMap<ObjectId, BTreeMap<String, super::BattleCharacterValue>>,
    #[serde(default)]
    pub(crate) recoveries: SharedMap<ObjectId, super::BattleRecovery>,
    #[serde(default)]
    pub(crate) characters: SharedMap<ObjectId, super::BattleCharacter>,
    #[serde(default)]
    pub(crate) constructed: SharedMap<ObjectId, BattleUnit>,
    /// Owned ground-vehicle state, awaiting battlefield admission.
    #[serde(default)]
    pub(crate) vehicles: SharedMap<ObjectId, super::BattleVehicle>,
    pub(crate) registrations: Arc<BTreeMap<ObjectId, String>>,
    pub(crate) maps: SharedMap<ObjectId, StoredBattleMap>,
    pub(crate) units: SharedMap<ObjectId, StoredBattleUnit>,
}

impl BtechState {
    /// Elapsed committed simulation seconds, shared by observations and outcomes.
    pub fn simulation_time(&self) -> i64 {
        self.simulation_seconds
    }

    /// Shared effective weapon values for the current runtime.
    pub fn weapon_settings(&self) -> &super::BattleWeaponSettings {
        &self.weapon_settings
    }

    /// Ground autopilot controllers keyed by controlled unit identity.
    pub fn controllers(&self) -> &SharedMap<ObjectId, super::autopilot::AutopilotController> {
        &self.controllers
    }
    /// Exact named skill/advantage records, separate from fixed health and attributes.
    pub fn character_values(
        &self,
    ) -> &SharedMap<ObjectId, BTreeMap<String, super::BattleCharacterValue>> {
        &self.character_values
    }

    /// Recovery state follows the player across cockpit changes; random streams are private to Rust.
    pub fn recoveries(&self) -> &SharedMap<ObjectId, super::BattleRecovery> {
        &self.recoveries
    }

    /// Whether the player is currently unable to operate a cockpit.
    pub fn unconscious(&self, player: ObjectId) -> bool {
        self.recoveries
            .get(&player)
            .is_some_and(|recovery| recovery.remaining > 0)
    }

    /// Persisted character attributes and health, separate from cockpit occupancy.
    pub fn characters(&self) -> &SharedMap<ObjectId, super::BattleCharacter> {
        &self.characters
    }

    /// Units with complete Rust-owned construction state.
    pub fn constructed_units(&self) -> &SharedMap<ObjectId, BattleUnit> {
        &self.constructed
    }

    /// Ground vehicles with owned material state, separate from live Mech simulation.
    pub fn vehicles(&self) -> &SharedMap<ObjectId, super::BattleVehicle> {
        &self.vehicles
    }

    /// Inspect registrations, including deferred special-object types.
    pub fn registrations(&self) -> &BTreeMap<ObjectId, String> {
        &self.registrations
    }

    /// Inspect saved map metadata without guessing at terrain codes.
    pub fn maps(&self) -> &SharedMap<ObjectId, StoredBattleMap> {
        &self.maps
    }

    /// Inspect saved unit metadata without activating simulation.
    pub fn units(&self) -> &SharedMap<ObjectId, StoredBattleUnit> {
        &self.units
    }

    /// Invariant check after one gameplay operation, with the same build split as
    /// [`World::validate_action`]: full in debug and test builds, skipped in release
    /// builds, where the server validates the finished transaction before persisting it.
    pub(crate) fn validate_action(&self, world: &World) -> Result<()> {
        if cfg!(debug_assertions) {
            return self.validate(world);
        }
        Ok(())
    }

    /// Validate decoded maps without interpreting deferred identities or opaque terrain.
    pub(crate) fn validate(&self, world: &World) -> Result<()> {
        let _measurement = super::autopilot::diagnostics::validation();
        let _loadouts = super::loadout_context::LoadoutScope::state(self);
        ensure!(self.simulation_seconds >= 0, "Invalid simulation time");
        super::autopilot::validate_controllers(&self.controllers)?;
        super::battlefield_identity::validate(self)?;
        self.weapon_settings.validate()?;
        super::inventory::validate(world)?;
        super::self_destruct::validate(self)?;
        self.reactor.validate()?;
        super::wreck_cleanup::validate(self)?;
        super::towing::validate(world)?;
        let tow_targets: BTreeSet<_> = self.tows.values().copied().collect();
        super::command_network::validate(world)?;
        for (player, dimensions) in self.player_preferences.iter() {
            ensure!(
                world
                    .objects
                    .get(player)
                    .is_some_and(|object| object.kind == Kind::Player),
                "View dimensions reference missing player"
            );
            dimensions.dimensions.validate()?;
        }
        for (player, configuration) in self.player_configuration.iter() {
            ensure!(
                world.objects.get(player).is_some_and(|object| {
                    object.kind == Kind::Player && !object.flags.contains(crate::Flag::Going)
                }),
                "Player configuration references unavailable player"
            );
            configuration.validate()?;
        }
        for (unit, configuration) in self.unit_configuration.iter() {
            ensure!(
                self.registrations.get(unit).map(String::as_str) == Some("MECH"),
                "Unit configuration references an unavailable unit"
            );
            ensure!(
                configuration
                    .preferred_id
                    .as_ref()
                    .is_none_or(|value| value.len() <= 2),
                "Invalid preferred unit id"
            );
            if let Some(pilot) = configuration.assigned_pilot {
                ensure!(
                    world
                        .objects
                        .get(&pilot)
                        .is_some_and(|object| object.kind == Kind::Player
                            && !object.flags.contains(crate::Flag::Going)),
                    "Assigned pilot is unavailable"
                );
            }
        }
        for (name, threshold) in self.skill_thresholds.iter() {
            let skill = super::skill_definition(name).context("Unknown threshold skill")?;
            ensure!(
                skill.name == name && *threshold <= i32::MAX as u32,
                "Invalid skill threshold"
            );
        }
        ensure!(
            self.characters
                .keys()
                .all(|id| world.objects.contains_key(id)),
            "Character references missing object"
        );
        for (id, entries) in self.character_values.iter() {
            ensure!(
                self.characters.contains_key(id),
                "Character values reference missing attributes"
            );
            ensure!(
                entries.keys().all(|name| !name.is_empty()
                    && name.chars().count() <= 255
                    && !name.contains('\0')),
                "Invalid character value name"
            );
        }
        for event in self.sensor_recoveries.iter() {
            event.validate(self)?;
        }
        for (id, recovery) in self.recoveries.iter() {
            recovery.validate()?;
            ensure!(
                world
                    .objects
                    .get(id)
                    .is_some_and(|object| object.kind == Kind::Player),
                "Recovery requires a player"
            );
            recovery.target(world, *id)?;
        }
        let contact_positions = super::validation_contacts::Positions::prepare(self);
        let mut pilots = BTreeSet::new();
        let mut map_slots = BTreeSet::new();
        for (id, vehicle) in self.vehicles.iter() {
            let local_validation = super::autopilot::diagnostics::combat("validation_unit");
            vehicle.hardware.validate()?;
            vehicle.validate_flight_state()?;
            vehicle.validate_orbital_drop()?;
            vehicle.validate_dig()?;
            super::radio::validate_channels(&vehicle.radio)?;
            super::radio::validate_attributes(&vehicle.definition().attributes)?;
            ensure!(
                vehicle.radio_experience_remaining <= 61,
                "Invalid radio experience countdown"
            );
            drop(local_validation);
            if !tow_targets.contains(id)
                && let Some(motion) = vehicle.motion()
            {
                motion.validate(
                    super::speed_bonus::saved_limit(vehicle.maximum_speed(), false, false, false)
                        + 10.75,
                )?;
                ensure!(
                    vehicle.power() == super::BattlePower::Running
                        || !motion.active()
                        || vehicle.idle_flight_controls(),
                    "Inactive untowed vehicle retains motion"
                );
                ensure!(
                    (vehicle.maximum_speed() > 0.0 && !vehicle.rotor_destroyed())
                        || !motion.active(),
                    "Immobile untowed vehicle retains motion"
                );
            }
            self.validate_contacts(
                *id,
                vehicle.position(),
                vehicle.contacts(),
                contact_positions.as_ref(),
            )?;
            if let Some(lock) = vehicle.target_lock() {
                self.validate_target_lock(*id, vehicle.position(), lock)?;
            }
            if let Some(lock) = vehicle.hex_lock() {
                let position = vehicle
                    .position()
                    .context("Hex lock requires a battlefield")?;
                self.maps
                    .get(&position.map)
                    .context("Map not found")?
                    .hex(i64::from(lock.hex.x), i64::from(lock.hex.y))?;
            }
            if let Some(pilot) = vehicle.pilot() {
                ensure!(pilots.insert(pilot), "Player pilots multiple units");
                ensure!(
                    world.objects.get(&pilot).is_some_and(
                        |object| object.kind == Kind::Player && object.location == Some(*id)
                    ),
                    "Pilot must be inside its unit"
                );
            }
            if let Some(position) = vehicle.position() {
                let map = self
                    .maps
                    .get(&position.map)
                    .context("Vehicle references missing map")?;
                let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
                ensure!(
                    !vehicle.under_bridge()
                        || (tile.terrain == super::Terrain::Bridge && tile.elevation >= 2),
                    "Vehicle under-bridge state requires a clear bridge span"
                );
                ensure!(
                    map_slots.insert((
                        position.map,
                        vehicle
                            .map_slot()
                            .context("Placed vehicle lacks a map slot")?
                    )),
                    "Duplicate battlefield slot"
                );
                ensure!(
                    world
                        .objects
                        .get(id)
                        .is_some_and(|object| object.location == Some(position.map)),
                    "Placed vehicle location differs from battlefield"
                );
            }
            ensure!(
                !self.constructed.contains_key(id) && !self.maps.contains_key(id),
                "Conflicting vehicle records"
            );
            ensure!(
                self.units.get(id) == Some(&vehicle.identity()),
                "Vehicle identity mismatch"
            );
            ensure!(
                self.registrations
                    .get(id)
                    .is_some_and(|kind| kind == "MECH"),
                "Vehicle lacks MECH registration"
            );
            ensure!(
                world
                    .objects
                    .get(id)
                    .is_some_and(|object| object.kind == Kind::Thing),
                "Vehicle requires a thing object"
            );
        }
        super::tag::validate(self)?;
        super::spotter_events::validate(self)?;
        for (id, unit) in self.constructed.iter() {
            if let Some(pilot) = unit.pilot() {
                ensure!(pilots.insert(pilot), "Player pilots multiple units");
                ensure!(
                    world.objects.get(&pilot).is_some_and(
                        |object| object.kind == Kind::Player && object.location == Some(*id)
                    ),
                    "Pilot must be inside its unit"
                );
            }
            super::validation_context::unit(*id, unit)?;
            if !tow_targets.contains(id)
                && let Some(motion) = unit.motion()
            {
                ensure!(
                    unit.power() == super::BattlePower::Running
                        || (motion.speed == 0.0 && motion.desired_speed == 0.0),
                    "Unpowered untowed unit cannot move"
                );
                motion.validate(unit.motion_speed_limit(unit.definition().max_speed))?;
                motion.validate(unit.motion_speed_limit(unit.mobility().maximum_speed))?;
            }
            ensure!(
                tow_targets.contains(id)
                    || unit
                        .ground_elevation
                        .is_none_or(
                            |height| (f64::from(i16::MIN)..=f64::from(i16::MAX)).contains(&height)
                        ),
                "Untowed unit retains an altitude outside scenario limits"
            );
            if let Some(position) = unit.position() {
                ensure!(
                    map_slots.insert((position.map, unit.map_slot().unwrap())),
                    "Duplicate battlefield slot"
                );
            }
            if let Some(lock) = unit.target_lock() {
                self.validate_target_lock(*id, unit.position(), lock)?;
            }
            if let Some(lock) = unit.hex_lock() {
                let position = unit.position().context("Hex lock requires a battlefield")?;
                self.maps[&position.map].hex(i64::from(lock.hex.x), i64::from(lock.hex.y))?;
            }
            self.validate_contacts(
                *id,
                unit.position(),
                unit.contacts(),
                contact_positions.as_ref(),
            )?;

            if let Some(position) = unit.position() {
                let map = self
                    .maps
                    .get(&position.map)
                    .context("Unit references missing map")?;
                map.hex(i64::from(position.x), i64::from(position.y))?;
                if let Some(flight) = unit.flight() {
                    ensure!(
                        !flight.arrived(),
                        "Unresolved jump landing cannot be committed"
                    );
                    ensure!(
                        flight.dfa_target() != Some(*id),
                        "DFA flight cannot target itself"
                    );
                    flight.validate_on_map(map)?;
                }
                ensure!(
                    world
                        .objects
                        .get(id)
                        .is_some_and(|object| object.location == Some(position.map)),
                    "Placed unit location differs from battlefield; remove it from the map before moving it"
                );
            }
            ensure!(
                self.units.get(id) == Some(&unit.identity()),
                "Unit identity mismatch"
            );
            ensure!(
                self.registrations
                    .get(id)
                    .is_some_and(|kind| kind == "MECH"),
                "Unit lacks MECH registration"
            );
            ensure!(
                world
                    .objects
                    .get(id)
                    .is_some_and(|object| object.kind == Kind::Thing),
                "Unit requires a thing object"
            );
        }
        for (id, map) in self.maps.iter().filter(|(_, map)| map.terrain_ready()) {
            super::validation_context::map(*id, map).with_context(|| format!("Map #{}", id.0))?;
            ensure!(
                map.landing_exclusions
                    .values()
                    .all(|zone| zone.owner == ObjectId(-1)
                        || world.objects.contains_key(&zone.owner)),
                "Landing exclusion references missing owner"
            );
            ensure!(
                map.minefields
                    .values()
                    .all(|mine| mine.owner == ObjectId(-1)
                        || world.objects.contains_key(&mine.owner)),
                "Minefield references missing owner"
            );
            ensure!(
                map.building_entrances.values().all(|entrance| world
                    .objects
                    .get(&entrance.interior)
                    .is_some_and(|object| object.kind != Kind::Garbage)),
                "Entrance references missing interior object"
            );
            ensure!(
                map.building_exits
                    .values()
                    .all(|exit| exit.destination != *id
                        && world
                            .objects
                            .get(&exit.destination)
                            .is_some_and(|object| object.kind != Kind::Garbage)),
                "Building exit references itself or a missing object"
            );
            if let Some(link) = map.authored_link {
                ensure!(
                    link.parent != *id && self.maps.contains_key(&link.parent),
                    "Authored map link references itself or a missing map"
                );
                ensure!(
                    link.coordinate.x >= 0
                        && link.coordinate.y >= 0
                        && link.entrances.iter().all(|entrance| match entrance {
                            super::BattleMapEntrance::Offset { distance } => *distance >= 0,
                            super::BattleMapEntrance::Exact { coordinate } =>
                                coordinate.x >= 0 && coordinate.y >= 0,
                            _ => true,
                        }),
                    "Invalid authored map link"
                );
            }

            ensure!(
                self.registrations.get(id).is_some_and(|kind| kind == "MAP"),
                "Decoded map #{} lacks MAP registration",
                id.0
            );
            ensure!(
                world
                    .objects
                    .get(id)
                    .is_some_and(|object| matches!(object.kind, Kind::Room | Kind::Thing)),
                "Decoded map #{} is not a live room or thing",
                id.0
            );
        }
        Ok(())
    }

    /// A saved unit selection must address another placed unit on the same battlefield.
    fn validate_target_lock(
        &self,
        id: ObjectId,
        position: Option<super::BattlePosition>,
        lock: super::BattleTargetLock,
    ) -> Result<()> {
        let target = self
            .constructed
            .get(&lock.target)
            .and_then(|unit| unit.position())
            .or_else(|| {
                self.vehicles
                    .get(&lock.target)
                    .and_then(|unit| unit.position())
            })
            .context("Lock references an unplaced or missing unit")?;
        ensure!(
            lock.target != id && position.is_some_and(|position| position.map == target.map),
            "Invalid target lock battlefield"
        );
        Ok(())
    }

    /// Observations must connect distinct, placed construction records on the same map.
    fn validate_contacts(
        &self,
        observer: ObjectId,
        position: Option<super::BattlePosition>,
        contacts: &BTreeMap<ObjectId, super::BattleContact>,
        positions: Option<&super::validation_contacts::Positions>,
    ) -> Result<()> {
        for target in contacts.keys() {
            ensure!(*target != observer, "Invalid unit contact");
            let target_position = if let Some(positions) = positions {
                positions
                    .get(*target)
                    .context("Contact references missing unit")?
            } else if let Some(vehicle) = self.vehicles.get(target) {
                vehicle.position()
            } else {
                self.constructed
                    .get(target)
                    .context("Contact references missing unit")?
                    .position()
            };
            ensure!(
                position.is_some() && position.map(|p| p.map) == target_position.map(|p| p.map),
                "Contact units must share a battlefield"
            );
        }
        Ok(())
    }

    /// Reconcile the projection with the same destruction plan used by relational cleanup.
    pub(crate) fn purge(&mut self, ids: &BTreeSet<ObjectId>) {
        self.inventories.retain(|object, _| !ids.contains(object));
        if ids.is_empty() {
            return;
        }
        self.controllers.retain(|unit, _| !ids.contains(unit));
        self.autopilot_plans.retain(|unit, _| !ids.contains(unit));
        let unplaced: BTreeSet<_> = self
            .constructed
            .iter()
            .filter_map(|(&id, unit)| {
                unit.position()
                    .filter(|position| ids.contains(&position.map))
                    .map(|_| id)
            })
            .chain(self.vehicles.iter().filter_map(|(&id, unit)| {
                unit.position()
                    .filter(|position| ids.contains(&position.map))
                    .map(|_| id)
            }))
            .collect();
        Arc::make_mut(&mut self.tows).retain(|carrier, target| {
            !ids.contains(carrier)
                && !ids.contains(target)
                && !unplaced.contains(carrier)
                && !unplaced.contains(target)
        });
        self.character_values.retain(|id, _| !ids.contains(id));
        self.player_preferences.retain(|id, _| !ids.contains(id));
        self.player_configuration.retain(|id, _| !ids.contains(id));
        self.unit_configuration.retain(|id, _| !ids.contains(id));
        for configuration in self.unit_configuration.values_mut() {
            if configuration
                .assigned_pilot
                .is_some_and(|pilot| ids.contains(&pilot))
            {
                configuration.assigned_pilot = None;
            }
        }
        self.characters.retain(|id, _| !ids.contains(id));
        Arc::make_mut(&mut self.wrecks).retain(|id, _| !ids.contains(id));
        self.recoveries.retain(|id, _| !ids.contains(id));
        Arc::make_mut(&mut self.sensor_recoveries).retain(|event| !ids.contains(&event.unit()));
        for &id in ids {
            super::map_slots::depart(self, id);
            self.constructed.remove(&id);
            self.vehicles.remove(&id);
        }
        self.vehicles.retain(|id, _| !ids.contains(id));
        for vehicle in self.vehicles.values_mut() {
            vehicle.contacts.retain(|id, _| !ids.contains(id));
            if vehicle
                .target_lock()
                .is_some_and(|lock| ids.contains(&lock.target))
            {
                vehicle.target_lock = None;
            }
            if vehicle.pilot.is_some_and(|pilot| ids.contains(&pilot)) {
                vehicle.pilot = None;
            }
            if vehicle
                .position()
                .is_some_and(|position| ids.contains(&position.map))
            {
                vehicle.contacts.clear();
                vehicle.target_lock = None;
                vehicle.set_placement(None);
                vehicle.power = super::BattlePower::Off;
            }
        }
        let constructed = &mut self.constructed;
        constructed.retain(|id, _| !ids.contains(id));
        for unit in constructed.values_mut() {
            unit.contacts.retain(|id, _| !ids.contains(id));
            if unit
                .target_lock()
                .is_some_and(|lock| ids.contains(&lock.target))
            {
                unit.target_lock = None;
            }
            if unit.pilot.is_some_and(|pilot| ids.contains(&pilot)) {
                unit.pilot = None;
            }
            if unit
                .position()
                .is_some_and(|position| ids.contains(&position.map))
            {
                unit.contacts.clear();
                unit.stagger = Default::default();
                unit.stand_timer = None;
                unit.target_lock = None;
                unit.flight = None;
                unit.free_fall = None;
                unit.orbital_drop = None;
                unit.jump_stabilization = 0;
                unit.c3i_network = None;
                unit.c3_network = None;
                unit.detached = false;
                unit.position = None;
                unit.map_slot = None;
                unit.hex_sync_pending = false;
                unit.ground_elevation = None;
                unit.motion = None;
                unit.power = super::BattlePower::Off;
                unit.masc.shutdown();
                unit.supercharger.shutdown();
                unit.charge.target = None;
                unit.carried_club = None;
            }
        }
        Arc::make_mut(&mut self.registrations).retain(|id, _| !ids.contains(id));
        self.maps.retain(|id, _| !ids.contains(id));
        for map in self.maps.values_mut() {
            if ids.contains(&ObjectId(map.building_parent)) {
                map.building_parent = 0;
            }
            if map
                .authored_link
                .is_some_and(|link| ids.contains(&link.parent))
            {
                map.authored_link = None;
            }
            Arc::make_mut(&mut map.landing_exclusions).retain(|_, zone| !ids.contains(&zone.owner));
            Arc::make_mut(&mut map.landing_exclusion_order)
                .retain(|slot| map.landing_exclusions.contains_key(slot));
            Arc::make_mut(&mut map.minefields).retain(|_, mine| !ids.contains(&mine.owner));
            Arc::make_mut(&mut map.minefield_order)
                .retain(|slot| map.minefields.contains_key(slot));
            Arc::make_mut(&mut map.building_entrances)
                .retain(|_, entrance| !ids.contains(&entrance.interior));
            Arc::make_mut(&mut map.building_exits)
                .retain(|_, exit| !ids.contains(&exit.destination));
        }
        let units = &mut self.units;
        units.retain(|id, _| !ids.contains(id));
        for unit in units.values_mut() {
            if unit.map.is_some_and(|map| ids.contains(&map)) {
                unit.map = None;
            }
        }
    }
}

/// Register an unused world container as a map with decoded source terrain.
pub fn create_map(
    world: &mut World,
    id: ObjectId,
    name: &str,
    asset: BattleMapAsset,
) -> Result<()> {
    map_target(world, id)?;
    ensure!(
        !world.btech.registrations.contains_key(&id)
            && !world.btech.maps.contains_key(&id)
            && !world.btech.units.contains_key(&id),
        "Object already has BattleTech state"
    );
    let map = map_from_asset(name, asset)?;
    world.btech.maps.insert(id, map);
    Arc::make_mut(&mut world.btech.registrations).insert(id, "MAP".into());
    Ok(())
}

/// Explicitly replace an unoccupied map's terrain with its source asset; dimensions must match.
pub fn reload_map(
    world: &mut World,
    id: ObjectId,
    name: &str,
    asset: BattleMapAsset,
) -> Result<()> {
    map_target(world, id)?;
    ensure!(
        world
            .btech
            .registrations
            .get(&id)
            .is_some_and(|kind| kind == "MAP"),
        "Object is not registered as a map"
    );
    let old = world.btech.maps.get(&id).context("Map not found")?;
    ensure!(
        (old.width, old.height) == (i64::from(asset.width), i64::from(asset.height)),
        "Reload must preserve map dimensions"
    );
    ensure!(
        !world.btech.units.values().any(|unit| unit.map == Some(id)),
        "Move units off the map before reloading terrain"
    );
    replace_map_asset(world, id, name, asset)
}

/// Replace decoded asset data after an enclosing operation establishes membership/object policy.
pub(super) fn replace_map_asset(
    world: &mut World,
    id: ObjectId,
    name: &str,
    asset: BattleMapAsset,
) -> Result<()> {
    let old = world.btech.maps.get(&id).context("Map not found")?;
    let modifier = old.movement_modifier;
    let mut map = map_from_asset(name, asset)?;
    map.movement_modifier = modifier;
    map.linked_markers = old.linked_markers.clone();
    map.cargo_transfer_point = old.cargo_transfer_point;
    map.light = old.light;
    map.visibility = old.visibility;
    map.maximum_visibility = old.maximum_visibility;
    map.cloud_base = old.cloud_base;
    map.sensor_flags = old.sensor_flags;
    map.wind_direction = old.wind_direction;
    map.wind_speed = old.wind_speed;
    map.building_parent = old.building_parent;
    map.building = old.building;
    map.building_repair = old.building_repair;
    map.artillery_shots = old.artillery_shots.clone();
    map.landing_exclusions = old.landing_exclusions.clone();
    map.membership_extent = old.membership_extent;
    map.landing_exclusion_order = old.landing_exclusion_order.clone();
    map.minefields = old.minefields.clone();
    map.minefield_order = old.minefield_order.clone();
    map.building_entrances = old.building_entrances.clone();
    map.building_entry_points = old.building_entry_points.clone();
    map.building_exits = old.building_exits.clone();
    map.authored_link = old.authored_link;
    map.static_decorations = old.static_decorations.clone();
    map.fire_dice = old.fire_dice.clone().or(map.fire_dice);
    world.btech.maps.insert(id, map);
    Ok(())
}

/// Validate the world identity before any map operation mutates state.
fn map_target(world: &World, id: ObjectId) -> Result<()> {
    ensure!(
        world.objects.get(&id).is_some_and(|object| matches!(
            object.kind,
            Kind::Thing | Kind::Room
        ) && !object
            .flags
            .contains(crate::Flag::Going)),
        "Map target must be a live room or thing"
    );
    Ok(())
}

/// Turn a parsed source into a checked persistent domain record.
pub(super) fn map_from_asset(name: &str, mut asset: BattleMapAsset) -> Result<StoredBattleMap> {
    ensure!(
        !name.is_empty() && name.len() <= 1024 && !name.contains('\0'),
        "Invalid map asset name"
    );
    asset.generate_bridges()?;
    let map = StoredBattleMap {
        membership_extent: 0,
        building_parent: 0,
        artillery_shots: Default::default(),
        name: name.into(),
        width: i64::from(asset.width),
        height: i64::from(asset.height),
        gravity: i64::from(asset.gravity),
        temperature: i64::from(asset.temperature),
        flags: i64::from(asset.flags),
        building: Default::default(),
        building_repair: None,
        cargo_transfer_point: None,
        landing_exclusions: Default::default(),
        landing_exclusion_order: Default::default(),
        minefields: Default::default(),
        minefield_order: Default::default(),
        building_entrances: Default::default(),
        building_entry_points: Default::default(),
        building_exits: Default::default(),
        authored_link: None,
        movement_modifier: 0,
        linked_markers: Default::default(),
        light: 2,
        visibility: 30,
        maximum_visibility: 60,
        cloud_base: 200,
        sensor_flags: 0,
        wind_direction: 0,
        wind_speed: 0,
        fire_dice: Some(super::BattleDice::fresh()),
        terrain: Some(asset.hexes),
        decorations: Default::default(),
        static_decorations: Default::default(),
    };
    map.validate()?;
    Ok(map)
}

/// Construct a BattleMech on an unused live thing, publishing only a checked candidate.
pub fn create_unit(world: &mut World, id: ObjectId, definition: BattleTemplate) -> Result<()> {
    ensure!(
        world.objects.get(&id).is_some_and(
            |object| object.kind == Kind::Thing && !object.flags.contains(crate::Flag::Going)
        ),
        "Unit target must be a live thing"
    );
    ensure!(
        !world.btech.registrations.contains_key(&id)
            && !world.btech.maps.contains_key(&id)
            && !world.btech.units.contains_key(&id),
        "Object already has BattleTech state"
    );
    super::inventory_mass(world, id)?;
    let unit = BattleUnit::from_template(definition)?;
    world.btech.units.insert(id, unit.identity());
    world.btech.constructed.insert(id, unit);
    Arc::make_mut(&mut world.btech.registrations).insert(id, "MECH".into());
    Ok(())
}

/// Register a live thing as an empty BattleTech unit before a template is loaded.
///
/// The reference server creates the raw MECH registration immediately and exposes
/// its zeroed/default critical layout through inspection APIs. Construction remains
/// absent until a template is loaded.
pub fn register_empty_battle_unit(world: &mut World, id: ObjectId) -> Result<()> {
    ensure!(
        world.objects.get(&id).is_some_and(
            |object| object.kind == Kind::Thing && !object.flags.contains(crate::Flag::Going)
        ),
        "Unit target must be a live thing"
    );
    ensure!(
        !world.btech.registrations.contains_key(&id)
            && !world.btech.maps.contains_key(&id)
            && !world.btech.units.contains_key(&id)
            && !world.btech.constructed.contains_key(&id)
            && !world.btech.vehicles.contains_key(&id),
        "Object already has BattleTech state"
    );
    super::inventory_mass(world, id)?;
    Arc::make_mut(&mut world.btech.registrations).insert(id, "MECH".into());
    Ok(())
}

/// Change light and weather visibility without changing occupied terrain or unit placement.
pub fn set_map_visibility(
    world: &mut World,
    id: ObjectId,
    light: super::BattleLight,
    visibility: u8,
) -> Result<()> {
    map_target(world, id)?;
    ensure!(visibility <= 60, "Invalid battlefield visibility");
    let map = world.btech.maps.get(&id).context("Map not found")?;
    ensure!(
        map.terrain_ready(),
        "Map terrain is ambiguous; reload it first"
    );
    let map = world.btech.maps.get_mut(&id).unwrap();
    let previous = map.light;
    map.light = light.stored();
    map.visibility = i64::from(visibility);
    map.maximum_visibility = (map.visibility * 3).clamp(24, 60);
    if previous != map.light {
        super::searchlight::reconcile_map(world, id);
    }
    Ok(())
}
