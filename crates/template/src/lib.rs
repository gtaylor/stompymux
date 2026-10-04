//! BattleTech unit templates, the equipment catalogue and construction rules, shared by the
//! stompymux server and template tools.
//!
//! A template is a unit as it is authored on disk: a TOML document naming the chassis,
//! tonnage, movement, armor and the equipment in each critical slot. [`BattleUnitTemplate`]
//! decodes either class of document; [`BattleTemplate`] is a BattleMech and
//! [`BattleVehicleTemplate`] a ground vehicle or VTOL. [`RawTemplate`] keeps any class's
//! fields and sections without validating construction.
//!
//! ```
//! use stompymux_template::{BattleTemplate, BattleUnitTemplate};
//!
//! let source = include_str!("../tests/fixtures/JR7-D.toml");
//! let BattleUnitTemplate::Mech(jenner) = BattleUnitTemplate::parse("JR7-D", source).unwrap()
//! else {
//!     panic!("the Jenner is a Mech");
//! };
//! let saved = jenner.to_document().unwrap();
//! assert_eq!(BattleTemplate::parse("JR7-D", &saved).unwrap(), jenner);
//! ```
//!
//! The crate also owns what construction derives from a template: the weapon and system
//! catalogue ([`BattleWeapon`], [`BattleSystem`]), resolved loadouts, engine ratings,
//! mass, construction cost and the template views the inspection commands report. It knows nothing about the game world or live unit state; the server builds
//! constructed units and combat rules on top of these types.
mod administrative;
mod ammunition;
mod ammunition_slots;
mod chassis;
mod construction;
mod cost;
mod document;
mod engine;
mod engine_sink_override;
mod equipment;
mod fire_mode;
mod inspection;
mod loadout;
mod mass;
mod mech;
mod metadata;
mod part_forms;
mod parts;
mod parts_catalogue;
mod raw;
mod registry;
mod render;
mod speed;
mod technology;
mod unit;
mod vehicle;
mod vehicle_engine;
mod vehicle_loadout;
mod vehicle_mass;

pub use administrative::{
    administrative_technology, administrative_template_movement, administrative_template_tonnage,
    edit_special,
};
pub use ammunition::BattleAmmunitionMode;
pub use chassis::BattleMechChassis;
pub use construction::mixed_technology_flag;
pub use cost::{
    BattlePartPrices, part_price, raw_template_base_cost, template_base_cost,
    vehicle_template_base_cost,
};
pub use document::ParsedTemplate;
pub use engine::{BattleEngine, rated_output};
pub use engine_sink_override::{
    parse_engine_sink_override, read_engine_sink_override, write_engine_sink_override,
};
pub use equipment::{
    BattleRangeBracket, BattleSystem, BattleWaterRanges, BattleWeapon, BattleWeaponRange,
    WeaponProfile, strip_name_prefix,
};
pub use fire_mode::BattleFireMode;
pub use inspection::{
    InspectionArmor, InspectionCritical, InspectionPart, InspectionWeapon,
    inspect_raw_template_armor, inspect_raw_template_criticals, inspect_raw_template_engine,
    inspect_raw_template_weapons, inspection_ammunition_modes, inspection_canonical_mech_internal,
    inspection_compatible_template, inspection_compatible_vehicle_template,
    inspection_configured_technology, inspection_configured_technology_attributes,
    inspection_engine_rating, inspection_fire_modes, inspection_normalized_jump_speed,
    inspection_raw_part, inspection_template_part, inspection_vehicle_engine_rating,
    inspection_vehicle_engine_values,
};
pub use loadout::{
    AmmunitionBin, BattleLoadout, CriticalLocation, ResolvedLoadout, SystemCritical, WeaponMount,
};
pub use mass::{
    armor_mass, cargo_space_mass, engine_mass, half_ton, one_shot_mass, power_amplifier_mass,
    structure_mass, system_slot_mass,
};
pub use mech::{BattleSection, BattleTemplate, CriticalDefinition, SectionDefinition};
pub use metadata::{unit_metadata, validate_unit_metadata};
pub use part_forms::{
    BattlePartForm, BattlePartNames, part_abbreviation, part_catalogue, part_names, part_short_name,
};
pub use parts::{
    AMMUNITION_PART_OFFSET, BattlePart, BattlePartKind, PART_ID_LIMIT, WEAPON_PART_IDS,
};
pub use raw::{RawMovement, RawSectionCode, RawTemplate, RawUnitClass, raw_default_mech_criticals};
pub use registry::{
    TemplateRegistryCache, finalize_raw_load_specials, read_resolved_raw_template,
    read_resolved_template, read_template_document, resolve_template_path,
    resolve_template_path_bytes_cached, resolve_template_path_cached, write_template,
};
pub use speed::{parse_template_speed, read_template_speed, write_template_speed};
pub use technology::{
    BattleDamageClass, BattleTechnology, flag_spells_technology, reflective_armor_slots,
};
pub use unit::BattleUnitTemplate;
pub use vehicle::{BattleVehicleMovement, BattleVehicleSection, BattleVehicleTemplate};
pub use vehicle_engine::{BattleVehicleEngine, BattleVehiclePowerplant};
pub use vehicle_loadout::{BattleVehicleLoadout, VehicleCriticalLocation};
pub use vehicle_mass::{BattleVehicleMass, BattleVehicleMaterial};
