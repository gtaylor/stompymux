//! Transactional maps and saved special-object identities, with shared immutable terrain.
use super::{Hex, HexCoordinate, MapAsset, Mech, MechTemplate};
use crate::{Kind, ObjectId, SharedMap, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Identity and dimensions of a saved map. Encoded tiles are not interpreted without a dictionary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredMap {
    /// Allocated membership span, including holes retained after removal.
    #[serde(default)]
    pub(crate) membership_extent: u32,
    /// Optional loading location and its coordinate disclosure policy.
    #[serde(default)]
    pub(crate) cargo_transfer_point: Option<super::CargoTransferPoint>,
    /// Admitted rounds in stable launch order.
    #[serde(default)]
    pub(crate) artillery_shots: Arc<BTreeMap<u32, super::ArtilleryShot>>,
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
    pub building: super::BuildingState,
    /// Committed seconds until the next construction repair event.
    #[serde(default)]
    pub building_repair: Option<u16>,
    /// Stable mine definitions, separate from visual terrain overlays.
    #[serde(default)]
    pub(crate) minefields: Arc<BTreeMap<u32, super::Minefield>>,
    /// Traversal order is independent of the persistent record identifiers.
    #[serde(default)]
    pub(crate) minefield_order: Arc<Vec<u32>>,
    /// Circular team-aware landing restrictions.
    #[serde(default)]
    pub(crate) landing_exclusions: Arc<BTreeMap<u32, super::LandingExclusion>>,
    /// Display and deletion traversal, independent of persistent restriction identities.
    #[serde(default)]
    pub(crate) landing_exclusion_order: Arc<Vec<u32>>,
    /// Ordered entrances pointing to interior maps.
    #[serde(default)]
    pub(crate) building_entrances: Arc<BTreeMap<u32, super::BuildingEntrance>>,
    /// Ordered interior arrival points selected by a direction code.
    #[serde(default)]
    pub(crate) building_entry_points: Arc<BTreeMap<u32, super::BuildingEntryPoint>>,
    /// Ordered return-map links; the first link is the active exit.
    #[serde(default)]
    pub(crate) building_exits: Arc<BTreeMap<u32, super::BuildingExit>>,
    /// Authored configuration used to rebuild runtime routes.
    #[serde(default)]
    pub(crate) authored_link: Option<super::MapLink>,
    /// Saved map movement percentage; zero or negative uses the standard rate.
    #[serde(default)]
    pub movement_modifier: i64,
    /// Battlefield illumination, weather visibility and saved sensor range ceiling.
    pub light: i64,
    pub visibility: i64,
    pub maximum_visibility: i64,
    /// Cloud boundary in elevation levels; zero disables cloud obstruction.
    pub cloud_base: i16,
    /// Perception channels switched off for this battlefield; see `MapPerceptionFlag`.
    pub sensor_flags: i64,
    /// Persisted wind bearing in degrees and strength used by terrain effects.
    #[serde(default)]
    pub wind_direction: i64,
    #[serde(default)]
    pub wind_speed: i64,
    /// Map-owned replayable stream established with decoded map creation.
    #[serde(default)]
    pub(crate) fire_dice: Option<super::Dice>,

    /// Absent until the terrain dictionary has been explicitly established.
    #[serde(default)]
    pub(crate) terrain: Option<Arc<Vec<Hex>>>,
    /// Sparse overlays indexed by row-major tile position, independent of source terrain.
    #[serde(default)]
    pub(crate) decorations: Arc<BTreeMap<u32, super::Decoration>>,
    /// Generic saved records carry restoration terrain but no autonomous timers.
    #[serde(default)]
    pub(crate) static_decorations: [Arc<BTreeMap<u32, super::StaticDecoration>>; 3],
    /// Scripted points of interest from the map file, in file order. Never shown to units.
    #[serde(default)]
    pub(crate) points_of_interest: Arc<Vec<super::MapPointOfInterest>>,
}

impl StoredMap {
    /// The saved map restriction blocks non-coolant fire between teammates.
    pub fn blocks_friendly_fire(&self) -> bool {
        self.has_flag(super::MapFlag::NoFriendlyFire)
    }

    /// Environmental rules are enabled by the map's persisted special-conditions flag.
    pub fn uses_special_rules(&self) -> bool {
        self.has_flag(super::MapFlag::SpecialRules)
    }

    /// Whether every tile has a known terrain/elevation interpretation.
    pub fn terrain_ready(&self) -> bool {
        self.terrain.is_some()
    }

    /// Inspect a decoded tile, rejecting ambiguous maps and invalid coordinates.
    pub fn hex(&self, x: i64, y: i64) -> Result<Hex> {
        let hex = self.base_hex(x, y)?;
        let overlay = self
            .decorations
            .get(&((y * self.width + x) as u32))
            .map(|effect| effect.kind);
        Ok(hex.with_overlay(overlay))
    }

    /// Install decoded terrain, turning any fire or smoke overlays it carries into permanent
    /// decorations so the terrain grid holds only the ground beneath them.
    pub(crate) fn establish_terrain(&mut self, hexes: Arc<Vec<Hex>>) -> Result<()> {
        self.decorations = Default::default();
        if hexes.iter().all(|hex| hex.overlay().is_none()) {
            self.terrain = Some(hexes);
            return Ok(());
        }
        let mut base = Vec::with_capacity(hexes.len());
        for (index, hex) in hexes.iter().enumerate() {
            base.push(hex.with_overlay(None));
            let Some(kind) = hex.overlay() else {
                continue;
            };
            let index = u32::try_from(index).context("Map is too large")?;
            let order = self
                .decorations
                .values()
                .filter(|effect| effect.kind == kind)
                .count();
            let mut effect = super::Decoration::new(kind, 0, None);
            effect.order = -1 - i64::try_from(order)?;
            Arc::make_mut(&mut self.decorations).insert(index, effect);
        }
        self.terrain = Some(Arc::new(base));
        Ok(())
    }

    /// Inspect the underlying tile without its fire or smoke overlay.
    pub fn base_hex(&self, x: i64, y: i64) -> Result<Hex> {
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

    /// The neighbors of `coordinate` indexed by direction clockwise from north, with `None` for
    /// each direction that leaves the map.
    pub fn neighbors(&self, coordinate: HexCoordinate) -> Result<[Option<HexCoordinate>; 6]> {
        let width = u16::try_from(self.width).context("Map width out of range")?;
        let height = u16::try_from(self.height).context("Map height out of range")?;
        Ok(coordinate.neighbors_within(width, height))
    }

    /// Check the complete decoded map before it can participate in a world transaction.
    pub(crate) fn validate(&self) -> Result<()> {
        let _measurement = super::autopilot::diagnostics::combat("validation_map");
        self.building.validate()?;
        if let Some(point) = self.cargo_transfer_point {
            point.validate(self)?;
        }
        self.validate_artillery()?;
        for point in self.points_of_interest.iter() {
            point.validate(self.width, self.height)?;
        }
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
            self.static_decorations
                .iter()
                .all(|records| records.len() <= 1_000_000),
            "Too many generic decorations"
        );
        for (kind, decoration) in super::StaticDecorationKind::ALL
            .into_iter()
            .flat_map(|kind| {
                self.static_decorations(kind)
                    .values()
                    .map(move |d| (kind, d))
            })
        {
            decoration.validate(kind)?;
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
                    .any(|effect| effect.kind == super::DecorationKind::Fire),
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
                .map(|effect| (effect.kind == super::DecorationKind::Fire, effect.order))
                .collect::<BTreeSet<_>>()
                .len()
                == self.decorations.len(),
            "Duplicate decoration creation order"
        );
        for hex in terrain.iter() {
            hex.validate().context("Invalid map elevation")?;
            ensure!(
                hex.overlay().is_none(),
                "Map terrain cannot hold fire or smoke"
            );
        }
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
    pub(crate) inventories: SharedMap<ObjectId, Vec<super::InventoryEntry>>,
    #[serde(default)]
    pub(crate) part_costs: Arc<BTreeMap<i32, u64>>,
    /// Runtime catalogue overrides reset on reload; saved countdowns retain their remaining time.
    #[serde(default)]
    pub(crate) weapon_settings: super::WeaponSettings,
    /// Seconds until a fully destroyed native unit retires from the battlefield.
    #[serde(default)]
    pub(crate) wrecks: Arc<BTreeMap<ObjectId, u8>>,
    #[serde(default)]
    pub(crate) reactor: super::reactor_instability::ReactorState,
    /// External towing is one chassis-independent carrier-to-target relationship.
    #[serde(default)]
    pub(crate) tows: Arc<BTreeMap<ObjectId, ObjectId>>,
    /// Saved player map dimensions and contact-list categories.
    #[serde(default)]
    pub(crate) player_preferences: SharedMap<ObjectId, super::PlayerPreferences>,
    /// Player-owned template and personal-combat configuration; UI configuration is tracked
    /// independently by presence in `player_preferences`.
    #[serde(default)]
    pub(crate) player_configuration: SharedMap<ObjectId, super::PlayerConfiguration>,
    #[serde(default)]
    pub(crate) unit_configuration: SharedMap<ObjectId, super::UnitConfiguration>,
    /// Runtime sensor band reach supplied by the host configuration; not stored in database tables.
    #[serde(default)]
    pub(crate) sensor_range: super::SensorRange,
    /// Runtime skill threshold overrides; database reload starts with catalog defaults.
    #[serde(default)]
    pub(crate) skill_thresholds: Arc<BTreeMap<String, u32>>,
    #[serde(default)]
    pub(crate) character_values: SharedMap<ObjectId, BTreeMap<String, super::CharacterValue>>,
    #[serde(default)]
    pub(crate) recoveries: SharedMap<ObjectId, super::Recovery>,
    #[serde(default)]
    pub(crate) characters: SharedMap<ObjectId, super::Character>,
    #[serde(default)]
    pub(crate) constructed: SharedMap<ObjectId, Mech>,
    /// Owned ground-vehicle state, awaiting battlefield admission.
    #[serde(default)]
    pub(crate) vehicles: SharedMap<ObjectId, super::Vehicle>,
    pub(crate) registrations: Arc<BTreeMap<ObjectId, String>>,
    pub(crate) maps: SharedMap<ObjectId, StoredMap>,
    pub(crate) units: SharedMap<ObjectId, StoredBattleUnit>,
}

impl BtechState {
    /// Elapsed committed simulation seconds, shared by observations and outcomes.
    pub fn simulation_time(&self) -> i64 {
        self.simulation_seconds
    }

    /// Shared effective weapon values for the current runtime.
    pub fn weapon_settings(&self) -> &super::WeaponSettings {
        &self.weapon_settings
    }

    /// Ground autopilot controllers keyed by controlled unit identity.
    pub fn controllers(&self) -> &SharedMap<ObjectId, super::autopilot::AutopilotController> {
        &self.controllers
    }
    /// Exact named skill/advantage records, separate from fixed health and attributes.
    pub fn character_values(
        &self,
    ) -> &SharedMap<ObjectId, BTreeMap<String, super::CharacterValue>> {
        &self.character_values
    }

    /// Recovery state follows the player across cockpit changes; random streams are private to Rust.
    pub fn recoveries(&self) -> &SharedMap<ObjectId, super::Recovery> {
        &self.recoveries
    }

    /// Whether the player is currently unable to operate a cockpit.
    pub fn unconscious(&self, player: ObjectId) -> bool {
        self.recoveries
            .get(&player)
            .is_some_and(|recovery| recovery.remaining > 0)
    }

    /// Persisted character attributes and health, separate from cockpit occupancy.
    pub fn characters(&self) -> &SharedMap<ObjectId, super::Character> {
        &self.characters
    }

    /// Units with complete Rust-owned construction state.
    pub fn constructed_units(&self) -> &SharedMap<ObjectId, Mech> {
        &self.constructed
    }

    /// Ground vehicles with owned material state, separate from live Mech simulation.
    pub fn vehicles(&self) -> &SharedMap<ObjectId, super::Vehicle> {
        &self.vehicles
    }

    /// Inspect registrations, including deferred special-object types.
    pub fn registrations(&self) -> &BTreeMap<ObjectId, String> {
        &self.registrations
    }

    /// Inspect saved map metadata without guessing at terrain codes.
    pub fn maps(&self) -> &SharedMap<ObjectId, StoredMap> {
        &self.maps
    }

    /// Inspect saved unit metadata without activating simulation.
    pub fn units(&self) -> &SharedMap<ObjectId, StoredBattleUnit> {
        &self.units
    }

    /// Rewrite one Mech or vehicle record through its serialized form.
    ///
    /// The result matches serializing the whole state, editing the record, and
    /// deserializing it all back, including clearing the runtime-only state that
    /// never survives serialization. It re-encodes only the one record instead of
    /// every unit and map. Fixtures use it to set fields that gameplay never writes.
    pub fn rewrite_unit_record(
        &mut self,
        id: ObjectId,
        edit: impl FnOnce(&mut serde_json::Value),
    ) -> Result<()> {
        if let Some(unit) = self.constructed.get(&id) {
            let mut record = serde_json::to_value(unit)?;
            edit(&mut record);
            let unit: Mech = serde_json::from_value(record)?;
            self.constructed.insert(id, unit);
        } else if let Some(vehicle) = self.vehicles.get(&id) {
            let mut record = serde_json::to_value(vehicle)?;
            edit(&mut record);
            let vehicle: super::Vehicle = serde_json::from_value(record)?;
            self.vehicles.insert(id, vehicle);
        } else {
            anyhow::bail!("#{} has no unit or vehicle record", id.0);
        }
        self.clear_runtime_state();
        Ok(())
    }

    /// Rewrite one battle map record through its serialized form.
    ///
    /// Like [`Self::rewrite_unit_record`], the result matches a whole-state serialize,
    /// edit and deserialize, clearing the same runtime-only state, while re-encoding
    /// only the one map. Fixtures use it to set fields that gameplay never writes, such
    /// as the map's fire stream.
    pub fn rewrite_map_record(
        &mut self,
        id: ObjectId,
        edit: impl FnOnce(&mut serde_json::Value),
    ) -> Result<()> {
        let map = self
            .maps
            .get(&id)
            .with_context(|| format!("#{} has no map record", id.0))?;
        let mut record = serde_json::to_value(map)?;
        edit(&mut record);
        let map: StoredMap = serde_json::from_value(record)?;
        self.maps.insert(id, map);
        self.clear_runtime_state();
        Ok(())
    }

    /// Rewrite one player's consciousness recovery record through its serialized form.
    ///
    /// Like [`Self::rewrite_unit_record`], the result matches a whole-state serialize,
    /// edit and deserialize, clearing the same runtime-only state, while re-encoding
    /// only the one record. Fixtures use it to set fields that gameplay never writes,
    /// such as the recovery's private random stream.
    pub fn rewrite_recovery_record(
        &mut self,
        player: ObjectId,
        edit: impl FnOnce(&mut serde_json::Value),
    ) -> Result<()> {
        let recovery = self
            .recoveries
            .get(&player)
            .with_context(|| format!("#{} has no recovery record", player.0))?;
        let mut record = serde_json::to_value(recovery)?;
        edit(&mut record);
        let recovery: super::Recovery = serde_json::from_value(record)?;
        self.recoveries.insert(player, recovery);
        self.clear_runtime_state();
        Ok(())
    }

    /// Drop the runtime-only state a serialization round trip never carries. Keep this
    /// in step with this type's `#[serde(skip)]` fields.
    pub(super) fn clear_runtime_state(&mut self) {
        self.template_registry = Default::default();
        self.retire_sanctions = Default::default();
        self.autopilot_plans = Default::default();
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
                self.registrations.get(unit).map(String::as_str) == Some("UNIT"),
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
        let mut roster = super::unit_validation::UnitRoster::default();
        for (id, vehicle) in self.vehicles.iter() {
            self.validate_unit(
                world,
                *id,
                super::UnitRef::Vehicle(vehicle),
                tow_targets.contains(id),
                &mut roster,
                contact_positions.as_ref(),
            )?;
        }
        super::tag::validate(self)?;
        super::spotter_events::validate(self)?;
        for (id, unit) in self.constructed.iter() {
            self.validate_unit(
                world,
                *id,
                super::UnitRef::Mech(unit),
                tow_targets.contains(id),
                &mut roster,
                contact_positions.as_ref(),
            )?;
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
                            super::MapEntrance::Offset { distance } => *distance >= 0,
                            super::MapEntrance::Exact { coordinate } =>
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
    pub(super) fn validate_target_lock(
        &self,
        id: ObjectId,
        position: Option<super::Position>,
        lock: super::TargetLock,
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
    pub(super) fn validate_contacts(
        &self,
        observer: ObjectId,
        position: Option<super::Position>,
        contacts: &BTreeMap<ObjectId, super::Contact>,
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
                vehicle.power = super::Power::Off;
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
                unit.power = super::Power::Off;
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
pub fn create_map(world: &mut World, id: ObjectId, name: &str, asset: MapAsset) -> Result<()> {
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
pub fn reload_map(world: &mut World, id: ObjectId, name: &str, asset: MapAsset) -> Result<()> {
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
    asset: MapAsset,
) -> Result<()> {
    let old = world.btech.maps.get(&id).context("Map not found")?;
    let modifier = old.movement_modifier;
    // Conditions the file leaves out keep the live map's current ones.
    let (light, visibility, wind) = (asset.light, asset.visibility, asset.wind);
    let mut map = map_from_asset(name, asset)?;
    map.movement_modifier = modifier;
    map.cargo_transfer_point = old.cargo_transfer_point;
    if light.is_none() {
        map.light = old.light;
    }
    if visibility.is_none() {
        map.visibility = old.visibility;
    }
    map.maximum_visibility = old.maximum_visibility;
    map.cloud_base = old.cloud_base;
    map.sensor_flags = old.sensor_flags;
    if wind.is_none() {
        map.wind_direction = old.wind_direction;
        map.wind_speed = old.wind_speed;
    }
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
pub(super) fn map_from_asset(name: &str, asset: MapAsset) -> Result<StoredMap> {
    ensure!(
        !name.is_empty() && name.len() <= 1024 && !name.contains('\0'),
        "Invalid map asset name"
    );
    let mut map = StoredMap {
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
        light: asset.light.map_or(2, super::Light::stored),
        visibility: asset.visibility.map_or(30, i64::from),
        maximum_visibility: 60,
        cloud_base: 200,
        sensor_flags: 0,
        wind_direction: asset.wind.map_or(0, |wind| i64::from(wind.direction)),
        wind_speed: asset.wind.map_or(0, |wind| i64::from(wind.speed)),
        fire_dice: Some(super::Dice::fresh()),
        terrain: None,
        decorations: Default::default(),
        static_decorations: Default::default(),
        points_of_interest: Arc::new(asset.points_of_interest),
    };
    map.establish_terrain(asset.hexes)?;
    map.validate()?;
    Ok(map)
}

/// Construct a BattleMech on an unused live thing, publishing only a checked candidate.
pub fn create_unit(world: &mut World, id: ObjectId, definition: MechTemplate) -> Result<()> {
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
    let unit = Mech::from_template(definition)?;
    world.btech.units.insert(id, unit.identity());
    world.btech.constructed.insert(id, unit);
    Arc::make_mut(&mut world.btech.registrations).insert(id, "UNIT".into());
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
    Arc::make_mut(&mut world.btech.registrations).insert(id, "UNIT".into());
    Ok(())
}

/// Change light and weather visibility without changing occupied terrain or unit placement.
pub fn set_map_visibility(
    world: &mut World,
    id: ObjectId,
    light: super::Light,
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

#[cfg(test)]
mod rewrite_tests {
    use super::*;
    use crate::{Config, VehicleTemplate};

    /// Rewriting one record matches a whole-state serialize, edit, deserialize.
    #[test]
    fn unit_record_rewrite_matches_a_whole_state_round_trip() {
        let config = Config::load("tests/fixtures/game").unwrap();
        let mut world = World {
            next_id: 42,
            ..Default::default()
        };
        let mech = world.create(&config, "Mech".into(), Kind::Thing);
        let vehicle = world.create(&config, "Vehicle".into(), Kind::Thing);
        let map = world.create(&config, "Map".into(), Kind::Room);
        create_unit(
            &mut world,
            mech,
            MechTemplate::parse("JR7-D", include_str!("../../game/units/JR7-D.toml")).unwrap(),
        )
        .unwrap();
        crate::create_battle_vehicle(
            &mut world,
            vehicle,
            VehicleTemplate::parse(
                "Demolisher",
                include_str!("../../game/units/Demolisher.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        create_map(
            &mut world,
            map,
            "test",
            MapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        world.btech.retire_sanctions.borrow_mut().insert(mech);
        for (id, class) in [(mech, "constructed"), (vehicle, "vehicles")] {
            let edit = |record: &mut serde_json::Value| record["heading"] = 120.into();
            let mut rewritten = world.btech.clone();
            rewritten.rewrite_unit_record(id, edit).unwrap();
            let mut whole = serde_json::to_value(&world.btech).unwrap();
            edit(&mut whole[class][id.0.to_string()]);
            let round_trip: BtechState = serde_json::from_value(whole).unwrap();
            assert_eq!(rewritten, round_trip);
            assert!(rewritten.retire_sanctions.borrow().is_empty());
        }
        assert!(world.btech.rewrite_unit_record(map, |_| {}).is_err());
    }

    /// Rewriting one map or recovery record matches a whole-state round trip too.
    #[test]
    fn map_and_recovery_record_rewrites_match_a_whole_state_round_trip() {
        let config = Config::load("tests/fixtures/game").unwrap();
        let mut world = World {
            next_id: 42,
            ..Default::default()
        };
        let map = world.create(&config, "Map".into(), Kind::Room);
        let player = world.create(&config, "Pilot".into(), Kind::Player);
        create_map(
            &mut world,
            map,
            "test",
            MapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        super::super::prepare_recovery(&mut world, player).unwrap();
        world.btech.retire_sanctions.borrow_mut().insert(map);
        let dice = serde_json::to_value(super::super::Dice::seeded([7; 32])).unwrap();
        let edit = |record: &mut serde_json::Value| record["fire_dice"] = dice.clone();
        let mut rewritten = world.btech.clone();
        rewritten.rewrite_map_record(map, edit).unwrap();
        let mut whole = serde_json::to_value(&world.btech).unwrap();
        edit(&mut whole["maps"][map.0.to_string()]);
        let round_trip: BtechState = serde_json::from_value(whole).unwrap();
        assert_eq!(rewritten, round_trip);
        assert!(rewritten.retire_sanctions.borrow().is_empty());

        let edit = |record: &mut serde_json::Value| record["dice"] = dice.clone();
        let mut rewritten = world.btech.clone();
        rewritten.rewrite_recovery_record(player, edit).unwrap();
        let mut whole = serde_json::to_value(&world.btech).unwrap();
        edit(&mut whole["recoveries"][player.0.to_string()]);
        let round_trip: BtechState = serde_json::from_value(whole).unwrap();
        assert_eq!(rewritten, round_trip);
        assert!(world.btech.rewrite_map_record(player, |_| {}).is_err());
        assert!(world.btech.rewrite_recovery_record(map, |_| {}).is_err());
    }
}
