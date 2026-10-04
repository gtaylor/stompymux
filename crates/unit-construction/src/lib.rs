//! BattleTech unit templates, the equipment catalogue and construction rules, shared by the
//! stompymux server and template tools.
//!
//! A template is a unit as it is authored on disk: a TOML document naming the chassis,
//! tonnage, movement, armor and the equipment in each critical slot. [`UnitTemplate`]
//! decodes either class of document; [`MechTemplate`] is a BattleMech and
//! [`VehicleTemplate`] a ground vehicle or VTOL. [`RawTemplate`] keeps any class's
//! fields and sections without validating construction.
//!
//! ```
//! use stompymux_unit_construction::{MechTemplate, UnitTemplate};
//!
//! let source = include_str!("../tests/fixtures/JR7-D.toml");
//! let UnitTemplate::Mech(jenner) = UnitTemplate::parse("JR7-D", source).unwrap()
//! else {
//!     panic!("the Jenner is a Mech");
//! };
//! let saved = jenner.to_document().unwrap();
//! assert_eq!(MechTemplate::parse("JR7-D", &saved).unwrap(), jenner);
//! ```
//!
//! The crate also owns what construction derives from a template: the weapon and system
//! catalogue ([`Weapon`], [`System`]), resolved loadouts, engine ratings,
//! mass, construction cost and the template views the inspection commands report. It
//! knows nothing about the game world or live unit state; the server builds constructed
//! units and combat rules on top of these types.
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
pub use ammunition::AmmunitionMode;
pub use chassis::MechChassis;
pub use construction::mixed_technology_flag;
pub use cost::{
    PartPrices, part_price, raw_template_base_cost, template_base_cost, vehicle_template_base_cost,
};
pub use document::ParsedTemplate;
pub use engine::{Engine, rated_output};
pub use engine_sink_override::{
    parse_engine_sink_override, read_engine_sink_override, write_engine_sink_override,
};
pub use equipment::{
    RangeBracket, System, WaterRanges, Weapon, WeaponProfile, WeaponRange, strip_name_prefix,
};
pub use fire_mode::FireMode;
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
    AmmunitionBin, CriticalLocation, MechLoadout, ResolvedLoadout, SystemCritical, WeaponMount,
};
pub use mass::{
    armor_mass, cargo_space_mass, engine_mass, half_ton, one_shot_mass, power_amplifier_mass,
    structure_mass, system_slot_mass,
};
pub use mech::{CriticalDefinition, MechSection, MechTemplate, SectionDefinition};
pub use metadata::{unit_metadata, validate_unit_metadata};
pub use part_forms::{
    PartForm, PartNames, part_abbreviation, part_catalogue, part_names, part_short_name,
};
pub use parts::{AMMUNITION_PART_OFFSET, PART_ID_LIMIT, Part, PartKind, WEAPON_PART_IDS};
pub use raw::{RawMovement, RawSectionCode, RawTemplate, RawUnitClass, raw_default_mech_criticals};
pub use registry::{
    TemplateRegistryCache, finalize_raw_load_specials, read_resolved_raw_template,
    read_resolved_template, read_template_document, resolve_template_path,
    resolve_template_path_bytes_cached, resolve_template_path_cached, write_template,
};
pub use speed::{parse_template_speed, read_template_speed, write_template_speed};
pub use technology::{DamageClass, Technology, flag_spells_technology, reflective_armor_slots};
pub use unit::UnitTemplate;
pub use vehicle::{VehicleMovement, VehicleSection, VehicleTemplate};
pub use vehicle_engine::{VehicleEngine, VehiclePowerplant};
pub use vehicle_loadout::{VehicleCriticalLocation, VehicleLoadout};
pub use vehicle_mass::{VehicleMass, VehicleMaterial};
