//! Typed, immutable BattleTech constant catalogs shared by all contract modules.

use super::error;
use mlua::{AnyUserData, Lua, MetaMethod, Table, UserData, UserDataMethods, Value};

/// One native value with its stable public spelling.
#[derive(Debug)]
pub(super) struct Entry {
    pub(super) name: &'static str,
    pub(super) value: i32,
}

/// Identity and inventory for one typed constant family.
#[derive(Debug)]
pub(super) struct Catalog {
    pub(super) qualified_name: &'static str,
    pub(super) entries: &'static [Entry],
}

/// One stable Lua spelling for a serialized state or reason.
#[derive(Debug)]
pub(super) struct StringEntry {
    pub(super) name: &'static str,
    pub(super) value: &'static str,
}

/// Identity and inventory for one string-valued constant family.
#[derive(Debug)]
pub(super) struct StringCatalog {
    pub(super) qualified_name: &'static str,
    pub(super) entries: &'static [StringEntry],
}

#[derive(Clone, Copy)]
struct Constant {
    catalog: &'static Catalog,
    entry: &'static Entry,
}

impl UserData for Constant {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, constant, ()| {
            Ok(constant.entry.name)
        });
        methods.add_meta_function(MetaMethod::Eq, |_, (left, right): (Value, Value)| {
            // C's constant __eq luaL_checkudatas both operands
            // (btech_constants.c constant_equal).
            super::super::error::typed_eq::<Self, _>("btmux.btech_constant", left, right, |a, b| {
                std::ptr::eq(a.catalog, b.catalog) && a.entry.value == b.entry.value
            })
        });
        methods.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(error::failure(
                    "mux.arg.invalid",
                    "BattleTech constants are immutable",
                ))
            },
        );
    }
}

#[derive(Clone, Copy)]
struct Namespace {
    catalog: &'static Catalog,
}

impl UserData for Namespace {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |lua, namespace, key: Value| {
            let Value::String(key) = key else {
                return Err(namespace_failure(
                    lua,
                    format!(
                        "{} constant name must be a string",
                        namespace.catalog.qualified_name
                    ),
                ));
            };
            let entry = namespace
                .catalog
                .entries
                .iter()
                .find(|entry| entry.name.as_bytes() == key.as_bytes().as_ref())
                .ok_or_else(|| {
                    namespace_failure(
                        lua,
                        format!(
                            "unknown {} constant '{}'",
                            namespace.catalog.qualified_name,
                            String::from_utf8_lossy(&key.as_bytes())
                        ),
                    )
                })?;
            push(lua, namespace.catalog, entry.value)
        });
        methods.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(error::failure(
                    "mux.arg.invalid",
                    "BattleTech constants are immutable",
                ))
            },
        );
    }
}

#[derive(Clone, Copy)]
struct StringNamespace {
    catalog: &'static StringCatalog,
}

impl UserData for StringNamespace {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |lua, namespace, key: Value| {
            let Value::String(key) = key else {
                return Err(namespace_failure(
                    lua,
                    format!(
                        "{} constant name must be a string",
                        namespace.catalog.qualified_name
                    ),
                ));
            };
            let entry = namespace
                .catalog
                .entries
                .iter()
                .find(|entry| entry.name.as_bytes() == key.as_bytes().as_ref())
                .ok_or_else(|| {
                    namespace_failure(
                        lua,
                        format!(
                            "unknown {} constant '{}'",
                            namespace.catalog.qualified_name,
                            String::from_utf8_lossy(&key.as_bytes())
                        ),
                    )
                })?;
            Ok(Value::String(lua.create_string(entry.value)?))
        });
        methods.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(error::failure(
                    "mux.arg.invalid",
                    "BattleTech constants are immutable",
                ))
            },
        );
    }
}

fn namespace_failure(lua: &Lua, message: impl ToString) -> mlua::Error {
    error::argument_failure(lua, 2, "mux.arg.invalid", message)
}

pub(super) static UNIT_TYPES: Catalog = Catalog {
    qualified_name: "btech.unit.types",
    entries: &[
        Entry {
            name: "MECH",
            value: 0,
        },
        Entry {
            name: "VEHICLE",
            value: 1,
        },
        Entry {
            name: "VTOL",
            value: 2,
        },
        Entry {
            name: "NAVAL",
            value: 3,
        },
        Entry {
            name: "SPHEROID_DROPSHIP",
            value: 4,
        },
        Entry {
            name: "AERO_FIGHTER",
            value: 5,
        },
        Entry {
            name: "MECHWARRIOR",
            value: 6,
        },
        Entry {
            name: "AERODYNE_DROPSHIP",
            value: 7,
        },
        Entry {
            name: "BATTLESUIT",
            value: 8,
        },
    ],
};

pub(super) static MOVEMENT_TYPES: Catalog = Catalog {
    qualified_name: "btech.unit.movement_types",
    entries: &[
        Entry {
            name: "BIPED",
            value: 0,
        },
        Entry {
            name: "TRACK",
            value: 1,
        },
        Entry {
            name: "WHEEL",
            value: 2,
        },
        Entry {
            name: "HOVER",
            value: 3,
        },
        Entry {
            name: "VTOL",
            value: 4,
        },
        Entry {
            name: "HULL",
            value: 5,
        },
        Entry {
            name: "FOIL",
            value: 6,
        },
        Entry {
            name: "FLY",
            value: 7,
        },
        Entry {
            name: "QUAD",
            value: 8,
        },
        Entry {
            name: "SUB",
            value: 9,
        },
        Entry {
            name: "NONE",
            value: 10,
        },
    ],
};

pub(super) static SECTIONS: Catalog = Catalog {
    qualified_name: "btech.unit.sections",
    entries: &[
        Entry {
            name: "FRONT_LEFT_LEG",
            value: 0,
        },
        Entry {
            name: "FRONT_RIGHT_LEG",
            value: 1,
        },
        Entry {
            name: "LEFT_TORSO",
            value: 2,
        },
        Entry {
            name: "RIGHT_TORSO",
            value: 3,
        },
        Entry {
            name: "CENTER_TORSO",
            value: 4,
        },
        Entry {
            name: "REAR_LEFT_LEG",
            value: 5,
        },
        Entry {
            name: "REAR_RIGHT_LEG",
            value: 6,
        },
        Entry {
            name: "HEAD",
            value: 7,
        },
        Entry {
            name: "LEFT_ARM",
            value: 8,
        },
        Entry {
            name: "RIGHT_ARM",
            value: 9,
        },
        Entry {
            name: "LEFT_LEG",
            value: 10,
        },
        Entry {
            name: "RIGHT_LEG",
            value: 11,
        },
        Entry {
            name: "SUIT_1",
            value: 12,
        },
        Entry {
            name: "SUIT_2",
            value: 13,
        },
        Entry {
            name: "SUIT_3",
            value: 14,
        },
        Entry {
            name: "SUIT_4",
            value: 15,
        },
        Entry {
            name: "SUIT_5",
            value: 16,
        },
        Entry {
            name: "SUIT_6",
            value: 17,
        },
        Entry {
            name: "SUIT_7",
            value: 18,
        },
        Entry {
            name: "SUIT_8",
            value: 19,
        },
        Entry {
            name: "LEFT_SIDE",
            value: 20,
        },
        Entry {
            name: "RIGHT_SIDE",
            value: 21,
        },
        Entry {
            name: "FRONT_SIDE",
            value: 22,
        },
        Entry {
            name: "AFT_SIDE",
            value: 23,
        },
        Entry {
            name: "TURRET",
            value: 24,
        },
        Entry {
            name: "ROTOR",
            value: 25,
        },
        Entry {
            name: "NOSE",
            value: 26,
        },
        Entry {
            name: "LEFT_WING",
            value: 27,
        },
        Entry {
            name: "RIGHT_WING",
            value: 28,
        },
        Entry {
            name: "LEFT_REAR_WING",
            value: 29,
        },
        Entry {
            name: "RIGHT_REAR_WING",
            value: 30,
        },
        Entry {
            name: "AFT",
            value: 31,
        },
        Entry {
            name: "FRONT_RIGHT_SIDE",
            value: 32,
        },
        Entry {
            name: "FRONT_LEFT_SIDE",
            value: 33,
        },
        Entry {
            name: "REAR_LEFT_SIDE",
            value: 34,
        },
        Entry {
            name: "REAR_RIGHT_SIDE",
            value: 35,
        },
    ],
};

pub(super) static TECHNOLOGY: Catalog = Catalog {
    qualified_name: "btech.unit.technology",
    entries: &[
        Entry {
            name: "TRIPLE_STRENGTH_MYOMER",
            value: 0,
        },
        Entry {
            name: "CLAN_ANTI_MISSILE",
            value: 1,
        },
        Entry {
            name: "INNER_SPHERE_ANTI_MISSILE",
            value: 2,
        },
        Entry {
            name: "DOUBLE_HEAT_SINKS",
            value: 3,
        },
        Entry {
            name: "MASC",
            value: 4,
        },
        Entry {
            name: "CLAN",
            value: 5,
        },
        Entry {
            name: "FLIPPABLE_ARMS",
            value: 6,
        },
        Entry {
            name: "C3_MASTER",
            value: 7,
        },
        Entry {
            name: "C3_SLAVE",
            value: 8,
        },
        Entry {
            name: "ARTEMIS_IV",
            value: 9,
        },
        Entry {
            name: "ECM",
            value: 10,
        },
        Entry {
            name: "BEAGLE_PROBE",
            value: 11,
        },
        Entry {
            name: "SALVAGE",
            value: 12,
        },
        Entry {
            name: "CARGO",
            value: 13,
        },
        Entry {
            name: "SEARCH_LIGHT",
            value: 14,
        },
        Entry {
            name: "LIGHT_ACTIVE_PROBE",
            value: 15,
        },
        Entry {
            name: "ANTI_AIRCRAFT",
            value: 16,
        },
        Entry {
            name: "NO_SENSORS",
            value: 17,
        },
        Entry {
            name: "SIXTH_SENSE",
            value: 18,
        },
        Entry {
            name: "FERRO_FIBROUS",
            value: 19,
        },
        Entry {
            name: "ENDO_STEEL",
            value: 20,
        },
        Entry {
            name: "XL_ENGINE",
            value: 21,
        },
        Entry {
            name: "ICE_ENGINE",
            value: 22,
        },
        Entry {
            name: "SINGLE_HEAT_SINKS",
            value: 23,
        },
        Entry {
            name: "LIGHT_ENGINE",
            value: 24,
        },
        Entry {
            name: "XXL_ENGINE",
            value: 25,
        },
        Entry {
            name: "COMPACT_ENGINE",
            value: 26,
        },
        Entry {
            name: "REINFORCED_INTERNAL",
            value: 27,
        },
        Entry {
            name: "COMPOSITE_INTERNAL",
            value: 28,
        },
        Entry {
            name: "HARDENED_ARMOR",
            value: 29,
        },
        Entry {
            name: "CRITICAL_PROOF",
            value: 30,
        },
        Entry {
            name: "STEALTH_ARMOR",
            value: 31,
        },
        Entry {
            name: "HEAVY_FERRO_FIBROUS",
            value: 32,
        },
        Entry {
            name: "LASER_REFLECTIVE_ARMOR",
            value: 33,
        },
        Entry {
            name: "REACTIVE_ARMOR",
            value: 34,
        },
        Entry {
            name: "NULL_SIGNATURE_SYSTEM",
            value: 35,
        },
        Entry {
            name: "C3I",
            value: 36,
        },
        Entry {
            name: "SUPERCHARGER",
            value: 37,
        },
        Entry {
            name: "IMPROVED_JUMP_JETS",
            value: 38,
        },
        Entry {
            name: "MECHANICAL_JUMP_JETS",
            value: 39,
        },
        Entry {
            name: "COMPACT_HEAT_SINKS",
            value: 40,
        },
        Entry {
            name: "LASER_HEAT_SINKS",
            value: 41,
        },
        Entry {
            name: "BLOODHOUND_PROBE",
            value: 42,
        },
        Entry {
            name: "ANGEL_ECM",
            value: 43,
        },
        Entry {
            name: "WATCHDOG",
            value: 44,
        },
        Entry {
            name: "LIGHT_FERRO_FIBROUS",
            value: 45,
        },
        Entry {
            name: "TAG",
            value: 46,
        },
        Entry {
            name: "OMNIMECH",
            value: 47,
        },
        Entry {
            name: "ARTEMIS_V",
            value: 48,
        },
        Entry {
            name: "CAMOUFLAGE",
            value: 49,
        },
        Entry {
            name: "CARRIER",
            value: 50,
        },
        Entry {
            name: "WATERPROOF",
            value: 51,
        },
        Entry {
            name: "XL_GYRO",
            value: 52,
        },
        Entry {
            name: "HEAVY_DUTY_GYRO",
            value: 53,
        },
        Entry {
            name: "COMPACT_GYRO",
            value: 54,
        },
        Entry {
            name: "TARGETING_COMPUTER",
            value: 55,
        },
        Entry {
            name: "SMALL_COCKPIT",
            value: 56,
        },
        Entry {
            name: "SWARM_ATTACK",
            value: 57,
        },
        Entry {
            name: "MOUNT_FRIENDS",
            value: 58,
        },
        Entry {
            name: "ANTI_LEG_ATTACK",
            value: 59,
        },
        Entry {
            name: "PURIFIER_STEALTH",
            value: 60,
        },
        Entry {
            name: "KAGE_STEALTH",
            value: 61,
        },
        Entry {
            name: "ACHILEUS_STEALTH",
            value: 62,
        },
        Entry {
            name: "INFILTRATOR_STEALTH",
            value: 63,
        },
        Entry {
            name: "INFILTRATOR_II_STEALTH",
            value: 64,
        },
        Entry {
            name: "MUST_JETTISON_PACK",
            value: 65,
        },
        Entry {
            name: "CAN_JETTISON_PACK",
            value: 66,
        },
    ],
};

pub(super) static TECHNOLOGY_GROUPS: Catalog = Catalog {
    qualified_name: "btech.unit.technology_groups",
    entries: &[
        Entry {
            name: "UNIT",
            value: 0,
        },
        Entry {
            name: "INFANTRY",
            value: 1,
        },
        Entry {
            name: "ALL",
            value: 2,
        },
    ],
};

pub(super) static FIRE_MODES: Catalog = Catalog {
    qualified_name: "btech.unit.fire_modes",
    entries: &[
        Entry {
            name: "DESTROYED",
            value: 1,
        },
        Entry {
            name: "DISABLED",
            value: 2,
        },
        Entry {
            name: "BROKEN",
            value: 4,
        },
        Entry {
            name: "DAMAGED",
            value: 8,
        },
        Entry {
            name: "TARGETING_COMPUTER",
            value: 16,
        },
        Entry {
            name: "REAR_MOUNT",
            value: 32,
        },
        Entry {
            name: "HOTLOAD",
            value: 64,
        },
        Entry {
            name: "HALF_TON",
            value: 128,
        },
        Entry {
            name: "ONE_SHOT",
            value: 256,
        },
        Entry {
            name: "ONE_SHOT_USED",
            value: 512,
        },
        Entry {
            name: "ULTRA",
            value: 1024,
        },
        Entry {
            name: "RAPID_FIRE",
            value: 2048,
        },
        Entry {
            name: "GATLING",
            value: 4096,
        },
        Entry {
            name: "ROTARY_TWO_SHOT",
            value: 8192,
        },
        Entry {
            name: "ROTARY_FOUR_SHOT",
            value: 16384,
        },
        Entry {
            name: "ROTARY_SIX_SHOT",
            value: 32768,
        },
        Entry {
            name: "HEAT",
            value: 65536,
        },
        Entry {
            name: "BACKPACK",
            value: 131072,
        },
        Entry {
            name: "JETTISONED",
            value: 262144,
        },
        Entry {
            name: "OMNI_BASE",
            value: 524288,
        },
        Entry {
            name: "ROCKET_FIRED",
            value: 1048576,
        },
    ],
};

pub(super) static AMMUNITION_MODES: Catalog = Catalog {
    qualified_name: "btech.unit.ammunition_modes",
    entries: &[
        Entry {
            name: "LBX_CLUSTER",
            value: 1,
        },
        Entry {
            name: "ARTEMIS_MINE",
            value: 2,
        },
        Entry {
            name: "NARC_SMOKE",
            value: 4,
        },
        Entry {
            name: "CLUSTER",
            value: 8,
        },
        Entry {
            name: "MINE",
            value: 16,
        },
        Entry {
            name: "SMOKE",
            value: 32,
        },
        Entry {
            name: "INFERNO",
            value: 64,
        },
        Entry {
            name: "SWARM",
            value: 128,
        },
        Entry {
            name: "SWARM_1",
            value: 256,
        },
        Entry {
            name: "INARC_EXPLOSIVE",
            value: 512,
        },
        Entry {
            name: "INARC_HAYWIRE",
            value: 1024,
        },
        Entry {
            name: "INARC_ECM",
            value: 2048,
        },
        Entry {
            name: "INARC_NEMESIS",
            value: 4096,
        },
        Entry {
            name: "ARMOR_PIERCING",
            value: 8192,
        },
        Entry {
            name: "FLECHETTE",
            value: 16384,
        },
        Entry {
            name: "INCENDIARY",
            value: 32768,
        },
        Entry {
            name: "PRECISION",
            value: 65536,
        },
        Entry {
            name: "STINGER",
            value: 131072,
        },
        Entry {
            name: "CASELESS",
            value: 262144,
        },
        Entry {
            name: "SEMI_GUIDED",
            value: 524288,
        },
        Entry {
            name: "EXTENDED_RANGE",
            value: 1048576,
        },
        Entry {
            name: "HIGH_EXPLOSIVE",
            value: 2097152,
        },
        Entry {
            name: "MML_LRM",
            value: 4194304,
        },
    ],
};

pub(super) static AUTOPILOT_ORDERS: Catalog = Catalog {
    qualified_name: "btech.autopilot.orders",
    entries: &[
        Entry {
            name: "MOVE",
            value: 0,
        },
        Entry {
            name: "HOLD",
            value: 1,
        },
        Entry {
            name: "FOLLOW",
            value: 2,
        },
        Entry {
            name: "PATROL",
            value: 3,
        },
        Entry {
            name: "ATTACK",
            value: 4,
        },
        Entry {
            name: "ATTACK_MOVE",
            value: 5,
        },
    ],
};

pub(super) static AUTOPILOT_SUBMISSION_MODES: Catalog = Catalog {
    qualified_name: "btech.autopilot.submission_modes",
    entries: &[
        Entry {
            name: "APPEND",
            value: 0,
        },
        Entry {
            name: "REPLACE",
            value: 1,
        },
    ],
};

pub(super) static AUTOPILOT_FIRE_MODES: Catalog = Catalog {
    qualified_name: "btech.autopilot.fire_modes",
    entries: &[
        Entry {
            name: "HOLD",
            value: 0,
        },
        Entry {
            name: "ASSIGNED_TARGET",
            value: 1,
        },
        Entry {
            name: "OPPORTUNISTIC",
            value: 2,
        },
    ],
};

/// Battlefield light levels accepted by map condition changes; values match the saved map column.
pub(super) static LIGHT_LEVELS: Catalog = Catalog {
    qualified_name: "btech.map.light_levels",
    entries: &[
        Entry {
            name: "NIGHT",
            value: 0,
        },
        Entry {
            name: "TWILIGHT",
            value: 1,
        },
        Entry {
            name: "DAY",
            value: 2,
        },
    ],
};

/// How a unit currently perceives a contact, as reported on contact rows and aim breakdowns.
pub(super) static DETECTION_CHANNELS: StringCatalog = StringCatalog {
    qualified_name: "btech.unit.detection_channels",
    entries: &[
        StringEntry {
            name: "SENSORS",
            value: "sensors",
        },
        StringEntry {
            name: "SIGHT",
            value: "sight",
        },
        StringEntry {
            name: "RADAR",
            value: "radar",
        },
        StringEntry {
            name: "PROBE",
            value: "probe",
        },
    ],
};

pub(super) static AUTOPILOT_STATES: StringCatalog = StringCatalog {
    qualified_name: "btech.autopilot.states",
    entries: &[
        StringEntry {
            name: "PAUSED",
            value: "paused",
        },
        StringEntry {
            name: "IDLE",
            value: "idle",
        },
        StringEntry {
            name: "EXECUTING",
            value: "executing",
        },
        StringEntry {
            name: "BLOCKED",
            value: "blocked",
        },
    ],
};

pub(super) static AUTOPILOT_ORDER_STATES: StringCatalog = StringCatalog {
    qualified_name: "btech.autopilot.order_states",
    entries: &[
        StringEntry {
            name: "QUEUED",
            value: "queued",
        },
        StringEntry {
            name: "RUNNING",
            value: "running",
        },
        StringEntry {
            name: "SUCCEEDED",
            value: "succeeded",
        },
        StringEntry {
            name: "FAILED",
            value: "failed",
        },
        StringEntry {
            name: "CANCELED",
            value: "canceled",
        },
    ],
};

pub(super) static AUTOPILOT_REASONS: StringCatalog = StringCatalog {
    qualified_name: "btech.autopilot.reasons",
    entries: &[
        StringEntry {
            name: "MANUAL_TAKEOVER",
            value: "manual_takeover",
        },
        StringEntry {
            name: "CONTACT_LOST",
            value: "contact_lost",
        },
        StringEntry {
            name: "STUCK",
            value: "stuck",
        },
        StringEntry {
            name: "UNREACHABLE",
            value: "unreachable",
        },
        StringEntry {
            name: "INVALIDATED",
            value: "invalidated",
        },
        StringEntry {
            name: "RESOURCE_LIMIT",
            value: "resource_limit",
        },
        StringEntry {
            name: "CONGESTED",
            value: "congested",
        },
        StringEntry {
            name: "INVALID_TARGET",
            value: "invalid_target",
        },
        StringEntry {
            name: "UNIT_UNAVAILABLE",
            value: "unit_unavailable",
        },
        StringEntry {
            name: "MAP_CHANGED",
            value: "map_changed",
        },
        StringEntry {
            name: "UNSUPPORTED",
            value: "unsupported",
        },
        StringEntry {
            name: "STALE_REVISION",
            value: "stale_revision",
        },
    ],
};

pub(super) static REPAIR_OPERATIONS: Catalog = Catalog {
    qualified_name: "btech.repair.operations",
    entries: &[
        Entry {
            name: "REATTACH",
            value: 0,
        },
        Entry {
            name: "REPAIR_PART",
            value: 1,
        },
        Entry {
            name: "REPAIR_WEAPON_TEMPORARY",
            value: 2,
        },
        Entry {
            name: "REPAIR_ENHANCEMENT",
            value: 3,
        },
        Entry {
            name: "REPAIR_FOCUS",
            value: 4,
        },
        Entry {
            name: "REPAIR_CRYSTAL",
            value: 5,
        },
        Entry {
            name: "REPAIR_BARREL",
            value: 6,
        },
        Entry {
            name: "REPAIR_AMMO_FEED",
            value: 7,
        },
        Entry {
            name: "REPAIR_RANGING",
            value: 8,
        },
        Entry {
            name: "REPAIR_AMMO_MOUNT",
            value: 9,
        },
        Entry {
            name: "REPLACE_WEAPON",
            value: 10,
        },
        Entry {
            name: "RELOAD",
            value: 11,
        },
        Entry {
            name: "REPAIR_ARMOR",
            value: 12,
        },
        Entry {
            name: "REPAIR_REAR_ARMOR",
            value: 13,
        },
        Entry {
            name: "REPAIR_INTERNAL",
            value: 14,
        },
        Entry {
            name: "DETACH",
            value: 15,
        },
        Entry {
            name: "SCRAP_PART",
            value: 16,
        },
        Entry {
            name: "SCRAP_WEAPON",
            value: 17,
        },
        Entry {
            name: "UNLOAD",
            value: 18,
        },
        Entry {
            name: "RESEAL",
            value: 19,
        },
        Entry {
            name: "REPLACE_SUIT",
            value: 20,
        },
    ],
};

/// Require a constant from exactly the expected catalog.
pub(super) fn require(
    value: Value,
    argument: usize,
    label: &str,
    catalog: &'static Catalog,
) -> mlua::Result<i32> {
    let Value::UserData(value) = value else {
        return Err(wrong_catalog(argument, label, catalog));
    };
    let constant = value
        .borrow::<Constant>()
        .map_err(|_| wrong_catalog(argument, label, catalog))?;
    if !std::ptr::eq(constant.catalog, catalog) {
        return Err(wrong_catalog(argument, label, catalog));
    }
    Ok(constant.entry.value)
}

fn wrong_catalog(argument: usize, label: &str, catalog: &Catalog) -> mlua::Error {
    error::failure_with_detail(
        "mux.arg.invalid",
        format!(
            "{label} must be a {} constant from this runtime",
            catalog.qualified_name
        ),
        serde_json::json!({ "argument": argument }),
    )
}

/// Push the typed spelling for a known native catalog value.
pub(super) fn push(lua: &Lua, catalog: &'static Catalog, value: i32) -> mlua::Result<Value> {
    let entry = catalog
        .entries
        .iter()
        .find(|entry| entry.value == value)
        .ok_or_else(|| {
            error::failure(
                "mux.internal",
                format!("unknown native value for {}", catalog.qualified_name),
            )
        })?;
    let constant = lua.create_userdata(Constant { catalog, entry })?;
    protect_metatable(lua, &constant, "protected BattleTech constant metatable")?;
    Ok(Value::UserData(constant))
}

fn namespace(lua: &Lua, catalog: &'static Catalog) -> mlua::Result<AnyUserData> {
    let namespace = lua.create_userdata(Namespace { catalog })?;
    protect_metatable(
        lua,
        &namespace,
        "protected BattleTech constant namespace metatable",
    )?;
    Ok(namespace)
}

fn string_namespace(lua: &Lua, catalog: &'static StringCatalog) -> mlua::Result<AnyUserData> {
    let namespace = lua.create_userdata(StringNamespace { catalog })?;
    protect_metatable(
        lua,
        &namespace,
        "protected BattleTech constant namespace metatable",
    )?;
    Ok(namespace)
}

fn protect_metatable(lua: &Lua, value: &AnyUserData, label: &'static str) -> mlua::Result<()> {
    // mlua protects userdata metatables with boolean false. The C contract exposes a
    // descriptive protection string, so replace only that protected sentinel while the
    // value and string are rooted on the stack by exec_raw.
    unsafe {
        lua.exec_raw::<()>((value.clone(), label), |state| {
            if mlua::ffi::lua_getmetatable(state, 1) != 0 {
                mlua::ffi::lua_pushvalue(state, 2);
                mlua::ffi::lua_setfield(state, -2, c"__metatable".as_ptr());
            }
        })
    }
}

fn table(lua: &Lua, package: &Table, name: &str) -> mlua::Result<Table> {
    match package.raw_get::<Value>(name)? {
        Value::Table(table) => Ok(table),
        _ => lua.create_table(),
    }
}

/// Install every C BattleTech constant namespace independently of gameplay state.
pub(super) fn install(lua: &Lua, package: &Table) -> mlua::Result<()> {
    let unit = table(lua, package, "unit")?;
    for (name, catalog) in [
        ("types", &UNIT_TYPES),
        ("movement_types", &MOVEMENT_TYPES),
        ("sections", &SECTIONS),
        ("technology", &TECHNOLOGY),
        ("technology_groups", &TECHNOLOGY_GROUPS),
        ("fire_modes", &FIRE_MODES),
        ("ammunition_modes", &AMMUNITION_MODES),
    ] {
        unit.raw_set(name, namespace(lua, catalog)?)?;
    }
    unit.raw_set(
        "detection_channels",
        string_namespace(lua, &DETECTION_CHANNELS)?,
    )?;
    package.raw_set("unit", unit)?;

    let autopilot = table(lua, package, "autopilot")?;
    for (name, catalog) in [
        ("orders", &AUTOPILOT_ORDERS),
        ("submission_modes", &AUTOPILOT_SUBMISSION_MODES),
        ("fire_modes", &AUTOPILOT_FIRE_MODES),
    ] {
        autopilot.raw_set(name, namespace(lua, catalog)?)?;
    }
    for (name, catalog) in [
        ("states", &AUTOPILOT_STATES),
        ("order_states", &AUTOPILOT_ORDER_STATES),
        ("reasons", &AUTOPILOT_REASONS),
    ] {
        autopilot.raw_set(name, string_namespace(lua, catalog)?)?;
    }
    package.raw_set("autopilot", autopilot)?;

    let map = table(lua, package, "map")?;
    map.raw_set("light_levels", namespace(lua, &LIGHT_LEVELS)?)?;
    package.raw_set("map", map)?;

    let repair = table(lua, package, "repair")?;
    repair.raw_set("operations", namespace(lua, &REPAIR_OPERATIONS)?)?;
    package.raw_set("repair", repair)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Lua catalog spells every perception channel exactly as serialization does.
    #[test]
    fn detection_channel_catalog_matches_serialization() {
        let values: Vec<_> = DETECTION_CHANNELS
            .entries
            .iter()
            .map(|entry| entry.value)
            .collect();
        assert_eq!(
            values,
            crate::BattleDetectionChannel::ALL.map(|channel| channel.name())
        );
        for entry in DETECTION_CHANNELS.entries {
            assert_eq!(entry.name, entry.value.to_ascii_uppercase());
        }
    }

    /// Light constants decode to the matching battlefield light level.
    #[test]
    fn light_level_catalog_matches_battlefield_light() {
        for entry in LIGHT_LEVELS.entries {
            let light = crate::BattleLight::from_stored(i64::from(entry.value)).unwrap();
            assert_eq!(light.stored(), i64::from(entry.value));
            assert_eq!(
                entry
                    .name
                    .to_ascii_lowercase()
                    .parse::<crate::BattleLight>()
                    .unwrap(),
                light
            );
        }
        assert_eq!(LIGHT_LEVELS.entries.len(), 3);
    }
}
