//! Construction choices that template documents declare by type instead of storing as
//! derived data: technology types that map to chassis flags, the fixed equipment those
//! types place in mech critical slots, and the internal structure and speeds that follow
//! from tonnage and movement points.
use super::{BattleMechChassis, BattleSection, RawUnitClass, SectionDefinition};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Kilometres per hour for each movement point.
pub(super) const SPEED_PER_MP: f64 = 10.75;

/// Critical names of every item that construction places in a mech.
pub(super) const FIXED_ITEMS: [&str; 9] = [
    SHOULDER_OR_HIP,
    UPPER_ACTUATOR,
    LOWER_ACTUATOR,
    HAND_OR_FOOT,
    ENGINE,
    GYRO,
    COCKPIT,
    LIFE_SUPPORT,
    SENSORS,
];

const SHOULDER_OR_HIP: &str = "ShoulderOrHip";
const UPPER_ACTUATOR: &str = "UpperActuator";
const LOWER_ACTUATOR: &str = "LowerActuator";
const HAND_OR_FOOT: &str = "HandOrFootActuator";
const ENGINE: &str = "Engine";
const GYRO: &str = "Gyro";
const COCKPIT: &str = "Cockpit";
const LIFE_SUPPORT: &str = "LifeSupport";
const SENSORS: &str = "Sensors";

/// Chassis flag recording Clan technology.
const CLAN: &str = "Clan";

/// Chassis flag recording flippable arms; it always follows the arm actuator layout.
pub(super) const FLIP_ARMS: &str = "FlipArms";

/// Declares one construction choice: its document spellings and the chassis flag each sets.
macro_rules! choice {
    ($(#[$doc:meta])* $name:ident { $($(#[$variant_doc:meta])* $variant:ident = $spelling:literal => [$($flag:literal),*]),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub(super) enum $name {
            #[default]
            $($(#[$variant_doc])* $variant),+
        }

        impl $name {
            const ALL: &[Self] = &[$(Self::$variant),+];

            /// Document spelling.
            fn spelling(self) -> &'static str {
                match self {
                    $(Self::$variant => $spelling),+
                }
            }

            /// Chassis flags for this choice; the first is the spelling written on load.
            fn flags(self) -> &'static [&'static str] {
                match self {
                    $(Self::$variant => &[$($flag),*]),+
                }
            }

            /// Decode a document spelling.
            fn parse(key: &str, value: &str) -> Result<Self> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|choice| choice.spelling() == value)
                    .with_context(|| {
                        let known: Vec<_> = Self::ALL.iter().map(|choice| choice.spelling()).collect();
                        format!("{key} must be one of {}", known.join(", "))
                    })
            }

            /// The single choice named by a unit's flags, if any.
            fn from_flags(key: &str, flags: &[&str]) -> Result<Self> {
                let named: Vec<_> = Self::ALL
                    .iter()
                    .copied()
                    .filter(|choice| {
                        choice.flags().iter().any(|flag| flags.iter().any(|f| f.eq_ignore_ascii_case(flag)))
                    })
                    .collect();
                match named.as_slice() {
                    [] => Ok(Self::default()),
                    [choice] => Ok(*choice),
                    _ => bail!("conflicting {key} flags"),
                }
            }
        }
    };
}

choice!(
    /// Fusion or combustion engine family.
    Engine {
        Standard = "standard" => [],
        Xl = "xl" => ["XLEngine_Tech"],
        Light = "light" => ["LightEngine_Tech"],
        Xxl = "xxl" => ["XXL_Tech"],
        Compact = "compact" => ["CompactEngine_Tech"],
        Ice = "ice" => ["ICEEngine_Tech"],
    }
);

choice!(
    /// Gyro family.
    Gyro {
        Standard = "standard" => [],
        Xl = "xl" => ["XLGyro_Tech", "XLGYRO"],
        Compact = "compact" => ["CompactGyro_Tech", "CGYRO"],
        HeavyDuty = "heavy_duty" => ["HDGyro_Tech", "HDGYRO"],
    }
);

choice!(
    /// Cockpit family.
    Cockpit {
        Standard = "standard" => [],
        Small = "small" => ["SmallCockpit_Tech", "SMCPIT"],
    }
);

choice!(
    /// Internal structure family.
    Structure {
        Standard = "standard" => [],
        EndoSteel = "endo_steel" => ["EndoSteel_Tech"],
        Composite = "composite" => ["CompositeInternal_Tech", "CINT"],
        Reinforced = "reinforced" => ["ReinforcedInternal_Tech", "RINT"],
    }
);

choice!(
    /// Armor family.
    Armor {
        Standard = "standard" => [],
        FerroFibrous = "ferro_fibrous" => ["FerroFibrous_Tech"],
        LightFerroFibrous = "light_ferro_fibrous" => ["LtFerroFibrous_Tech"],
        HeavyFerroFibrous = "heavy_ferro_fibrous" => ["HvyFerroFibrous_Tech"],
        Stealth = "stealth" => ["StealthArmor_Tech"],
        Hardened = "hardened" => ["HardenedArmor_Tech", "HARM"],
        LaserReflective = "laser_reflective" => ["LaserRefArmor_Tech", "LRARM"],
    }
);

choice!(
    /// Heat sink family beyond the tech base default.
    HeatSinks {
        Single = "single" => [],
        Double = "double" => ["DoubleHS"],
        Laser = "laser" => ["LaserHS_Tech", "LHS"],
    }
);

choice!(
    /// Myomer family.
    Myomer {
        Standard = "standard" => [],
        TripleStrength = "triple_strength" => ["TripleMyomerTech"],
    }
);

/// A chassis component that a mixed-technology unit may build from the other technology base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Component {
    Engine,
    Structure,
    Armor,
}

impl Component {
    const ALL: [Self; 3] = [Self::Engine, Self::Structure, Self::Armor];

    /// The `[construction]` key naming this component's technology base.
    fn key(self) -> &'static str {
        match self {
            Self::Engine => "engine_tech",
            Self::Structure => "structure_tech",
            Self::Armor => "armor_tech",
        }
    }

    /// The chassis flag recording that this component uses Clan (`clan`) or Inner Sphere
    /// technology on a chassis of the other base.
    pub(super) fn flag(self, clan: bool) -> &'static str {
        match (self, clan) {
            (Self::Engine, true) => "ClanEngine_Tech",
            (Self::Engine, false) => "ISEngine_Tech",
            (Self::Structure, true) => "ClanStructure_Tech",
            (Self::Structure, false) => "ISStructure_Tech",
            (Self::Armor, true) => "ClanArmor_Tech",
            (Self::Armor, false) => "ISArmor_Tech",
        }
    }
}

/// Decode a `tech_base`-style spelling: `true` for Clan.
fn tech_base(key: &str, value: &str) -> Result<bool> {
    match value {
        "inner_sphere" => Ok(false),
        "clan" => Ok(true),
        _ => bail!("{key} must be one of inner_sphere, clan"),
    }
}

/// The `[construction]` table: technology types for the chassis and its fixed equipment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Construction {
    pub clan: bool,
    /// Components built from the other technology base, in [`Component::ALL`] order.
    pub mixed: Vec<Component>,
    pub engine: Engine,
    pub gyro: Gyro,
    pub cockpit: Cockpit,
    pub structure: Structure,
    pub armor: Armor,
    pub heat_sinks: HeatSinks,
    pub myomer: Myomer,
}

/// The document form of [`Construction`].
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstructionDocument {
    tech_base: Option<String>,
    engine: Option<String>,
    gyro: Option<String>,
    cockpit: Option<String>,
    structure: Option<String>,
    armor: Option<String>,
    heat_sinks: Option<String>,
    myomer: Option<String>,
    engine_tech: Option<String>,
    structure_tech: Option<String>,
    armor_tech: Option<String>,
}

impl Construction {
    /// Decode a `[construction]` table.
    pub fn decode(value: toml::Value) -> Result<Self> {
        let document: ConstructionDocument = value.try_into()?;
        let clan = match document.tech_base.as_deref() {
            None => false,
            Some(value) => tech_base("tech_base", value)?,
        };
        let mut mixed = Vec::new();
        for (component, value) in Component::ALL.into_iter().zip([
            &document.engine_tech,
            &document.structure_tech,
            &document.armor_tech,
        ]) {
            if let Some(value) = value
                && tech_base(component.key(), value)? != clan
            {
                mixed.push(component);
            }
        }
        let heat_sinks = match document.heat_sinks.as_deref() {
            None if clan => HeatSinks::Double,
            None => HeatSinks::Single,
            Some(value) => HeatSinks::parse("heat_sinks", value)?,
        };
        ensure!(
            !clan || heat_sinks != HeatSinks::Single,
            "Clan units always have double heat sinks"
        );
        Ok(Self {
            clan,
            mixed,
            engine: choice(document.engine, Engine::parse, "engine")?,
            gyro: choice(document.gyro, Gyro::parse, "gyro")?,
            cockpit: choice(document.cockpit, Cockpit::parse, "cockpit")?,
            structure: choice(document.structure, Structure::parse, "structure")?,
            armor: choice(document.armor, Armor::parse, "armor")?,
            heat_sinks,
            myomer: choice(document.myomer, Myomer::parse, "myomer")?,
        })
    }

    /// Recover construction choices from chassis flags, returning the flags no choice owns.
    pub fn from_flags(flags: &[&str]) -> Result<(Self, Vec<String>)> {
        let clan = flags.iter().any(|flag| flag.eq_ignore_ascii_case(CLAN));
        let heat_sinks = match HeatSinks::from_flags("heat_sinks", flags)? {
            HeatSinks::Single if clan => HeatSinks::Double,
            heat_sinks => heat_sinks,
        };
        let has = |name: &str| flags.iter().any(|flag| flag.eq_ignore_ascii_case(name));
        let mut mixed = Vec::new();
        for component in Component::ALL {
            ensure!(
                !has(component.flag(clan)),
                "{} names the chassis technology base",
                component.flag(clan)
            );
            if has(component.flag(!clan)) {
                mixed.push(component);
            }
        }
        let construction = Self {
            clan,
            mixed,
            engine: engine_from_flags(flags)?,
            gyro: Gyro::from_flags("gyro", flags)?,
            cockpit: Cockpit::from_flags("cockpit", flags)?,
            structure: Structure::from_flags("structure", flags)?,
            armor: Armor::from_flags("armor", flags)?,
            heat_sinks,
            myomer: Myomer::from_flags("myomer", flags)?,
        };
        let remaining = flags
            .iter()
            .filter(|flag| !owned_flag(flag) && !flag.eq_ignore_ascii_case(FLIP_ARMS))
            .map(|flag| (*flag).to_owned())
            .collect();
        Ok((construction, remaining))
    }

    /// Chassis flags these choices set, in a stable order.
    pub fn flags(&self) -> Vec<&'static str> {
        let mut flags = Vec::new();
        if self.clan {
            flags.push(CLAN);
        }
        flags.extend(self.engine.flags().first());
        flags.extend(self.gyro.flags().first());
        flags.extend(self.cockpit.flags().first());
        flags.extend(self.structure.flags().first());
        flags.extend(self.armor.flags().first());
        if !(self.clan && self.heat_sinks == HeatSinks::Double) {
            flags.extend(self.heat_sinks.flags().first());
        }
        flags.extend(self.myomer.flags().first());
        flags.extend(
            self.mixed
                .iter()
                .map(|component| component.flag(!self.clan)),
        );
        flags
    }

    /// Whether `component` is built from Clan technology.
    pub fn clan_component(&self, component: Component) -> bool {
        self.clan != self.mixed.contains(&component)
    }

    /// Render the non-default choices as a `[construction]` table, or nothing.
    pub fn render(&self) -> String {
        let mut lines = String::new();
        let mut line = |key: &str, value: &str| {
            let _ = writeln!(lines, "{key} = \"{value}\"");
        };
        if self.clan {
            line("tech_base", "clan");
        }
        for (key, value, default) in [
            (
                "engine",
                self.engine.spelling(),
                Engine::default().spelling(),
            ),
            ("gyro", self.gyro.spelling(), Gyro::default().spelling()),
            (
                "cockpit",
                self.cockpit.spelling(),
                Cockpit::default().spelling(),
            ),
            (
                "structure",
                self.structure.spelling(),
                Structure::default().spelling(),
            ),
            ("armor", self.armor.spelling(), Armor::default().spelling()),
            (
                "heat_sinks",
                self.heat_sinks.spelling(),
                if self.clan { "double" } else { "single" },
            ),
            (
                "myomer",
                self.myomer.spelling(),
                Myomer::default().spelling(),
            ),
        ] {
            if value != default {
                line(key, value);
            }
        }
        for component in &self.mixed {
            let _ = writeln!(
                lines,
                "{} = \"{}\"",
                component.key(),
                if self.clan { "inner_sphere" } else { "clan" }
            );
        }
        if lines.is_empty() {
            return lines;
        }
        format!("\n[construction]\n{lines}")
    }
}

/// The engine a unit's flags name. The loader derives the compact bit from centre torso
/// geometry, so another engine flag beside it names the engine actually declared.
fn engine_from_flags(flags: &[&str]) -> Result<Engine> {
    let compact = Engine::Compact.flags();
    let declared: Vec<&str> = flags
        .iter()
        .copied()
        .filter(|flag| !compact.iter().any(|bit| bit.eq_ignore_ascii_case(flag)))
        .collect();
    match Engine::from_flags("engine", &declared)? {
        Engine::Standard => Engine::from_flags("engine", flags),
        engine => Ok(engine),
    }
}

/// Decode an optional document choice, defaulting when absent.
fn choice<T: Default>(
    value: Option<String>,
    parse: fn(&str, &str) -> Result<T>,
    key: &str,
) -> Result<T> {
    value.map_or(Ok(T::default()), |value| parse(key, &value))
}

/// Whether a flag records a component built from the other technology base.
pub(super) fn mixed_technology_flag(flag: &str) -> bool {
    Component::ALL.into_iter().any(|component| {
        [true, false]
            .into_iter()
            .any(|clan| component.flag(clan).eq_ignore_ascii_case(flag))
    })
}

/// Whether a flag is set by a construction choice or derived, so `specials` may not list it.
pub(super) fn owned_flag(flag: &str) -> bool {
    flag.eq_ignore_ascii_case(CLAN)
        || mixed_technology_flag(flag)
        || Engine::ALL
            .iter()
            .flat_map(|choice| choice.flags())
            .chain(Gyro::ALL.iter().flat_map(|choice| choice.flags()))
            .chain(Cockpit::ALL.iter().flat_map(|choice| choice.flags()))
            .chain(Structure::ALL.iter().flat_map(|choice| choice.flags()))
            .chain(Armor::ALL.iter().flat_map(|choice| choice.flags()))
            .chain(HeatSinks::ALL.iter().flat_map(|choice| choice.flags()))
            .chain(Myomer::ALL.iter().flat_map(|choice| choice.flags()))
            .any(|owned| owned.eq_ignore_ascii_case(flag))
}

/// The canonical spelling of a `specials` flag, refusing unknown and construction-owned flags.
pub(super) fn canonical_special(flag: &str) -> Result<&'static str> {
    ensure!(
        !owned_flag(flag),
        "{flag} is a construction choice; declare it in [construction]"
    );
    ensure!(
        !flag.eq_ignore_ascii_case(FLIP_ARMS),
        "FlipArms follows the arm actuators and cannot be listed"
    );
    technology_names(0..=56)
        .chain(
            super::BattleTechnology::ALL
                .iter()
                .map(|technology| technology.names().0),
        )
        .find(|known| known.eq_ignore_ascii_case(flag))
        .or_else(|| {
            super::BattleTechnology::ALL
                .iter()
                .find(|technology| technology.names().1.eq_ignore_ascii_case(flag))
                .map(|technology| technology.names().0)
        })
        .with_context(|| format!("unknown special {flag}"))
}

/// The canonical spelling of an `infantry_specials` flag.
pub(super) fn canonical_infantry_special(flag: &str) -> Result<&'static str> {
    technology_names(57..=66)
        .find(|known| known.eq_ignore_ascii_case(flag))
        .with_context(|| format!("unknown infantry special {flag}"))
}

/// Reference technology flag names for a range of administrative codes.
fn technology_names(codes: std::ops::RangeInclusive<i32>) -> impl Iterator<Item = &'static str> {
    codes.filter_map(|code| {
        super::admin_contract::administrative_technology(code).map(|(name, _)| name)
    })
}

/// Actuators a section may leave out of its standard installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Omission {
    Shoulder,
    Upper,
    Lower,
    Hand,
    Foot,
}

impl Omission {
    const ALL: [Self; 5] = [
        Self::Shoulder,
        Self::Upper,
        Self::Lower,
        Self::Hand,
        Self::Foot,
    ];

    /// Document spelling.
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Shoulder => "shoulder",
            Self::Upper => "upper_actuator",
            Self::Lower => "lower_actuator",
            Self::Hand => "hand_actuator",
            Self::Foot => "foot_actuator",
        }
    }

    /// Decode a document spelling.
    pub fn parse(value: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|omission| omission.spelling() == value)
            .with_context(|| format!("unknown omitted actuator {value}"))
    }
}

/// How one section departs from the standard placement of its fixed equipment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SectionPlan<'a> {
    pub omit: &'a [Omission],
    /// Zero-based first engine slot in a side torso.
    pub engine_at: Option<u8>,
    /// Engine slots the section holds when they differ from the engine type's count.
    pub engine_slots: Option<u8>,
}

/// Fixed equipment construction places in one mech section, by zero-based slot.
pub(super) fn fixed_equipment(
    construction: &Construction,
    chassis: BattleMechChassis,
    section: BattleSection,
    plan: SectionPlan<'_>,
) -> Result<BTreeMap<u8, &'static str>> {
    use BattleSection::*;
    let mut items = BTreeMap::new();
    let limb = |items: &mut BTreeMap<u8, &'static str>, last: &'static str, omit_last: Omission| {
        if !plan.omit.contains(&Omission::Shoulder) {
            items.insert(0, SHOULDER_OR_HIP);
        }
        if !plan.omit.contains(&Omission::Upper) {
            items.insert(1, UPPER_ACTUATOR);
        }
        if !plan.omit.contains(&Omission::Lower) {
            items.insert(2, LOWER_ACTUATOR);
        }
        if !plan.omit.contains(&omit_last) {
            items.insert(3, last);
        }
    };
    let arm = chassis == BattleMechChassis::Biped && matches!(section, LeftArm | RightArm);
    let allowed: &[Omission] = match section {
        LeftArm | RightArm if arm => &[
            Omission::Shoulder,
            Omission::Upper,
            Omission::Lower,
            Omission::Hand,
        ],
        LeftArm | RightArm | LeftLeg | RightLeg => &[
            Omission::Shoulder,
            Omission::Upper,
            Omission::Lower,
            Omission::Foot,
        ],
        _ => &[],
    };
    for omission in plan.omit {
        ensure!(
            allowed.contains(omission),
            "{} has no {}",
            section.name(),
            omission.spelling()
        );
    }
    let side_engine = match (
        construction.engine,
        construction.clan_component(Component::Engine),
    ) {
        (Engine::Xl, false) => 3,
        (Engine::Xl, true) | (Engine::Light, _) => 2,
        (Engine::Xxl, false) => 6,
        (Engine::Xxl, true) => 4,
        _ => 0,
    };
    ensure!(
        plan.engine_slots.is_none() || matches!(section, LeftTorso | RightTorso | CenterTorso),
        "engine_slots applies only to torsos"
    );
    let side_engine = plan.engine_slots.map_or(side_engine, usize::from);
    ensure!(
        plan.engine_at.is_none() || (matches!(section, LeftTorso | RightTorso) && side_engine > 0),
        "engine_at applies only to side torsos holding engine slots"
    );
    match section {
        LeftArm | RightArm if arm => limb(&mut items, HAND_OR_FOOT, Omission::Hand),
        LeftArm | RightArm | LeftLeg | RightLeg => limb(&mut items, HAND_OR_FOOT, Omission::Foot),
        Head => {
            let layout: &[(u8, &str)] = match construction.cockpit {
                Cockpit::Standard => &[
                    (0, LIFE_SUPPORT),
                    (1, SENSORS),
                    (2, COCKPIT),
                    (4, SENSORS),
                    (5, LIFE_SUPPORT),
                ],
                Cockpit::Small => &[(0, LIFE_SUPPORT), (1, SENSORS), (2, COCKPIT), (3, SENSORS)],
            };
            items.extend(layout.iter().copied());
        }
        CenterTorso => {
            let gyro = match construction.gyro {
                Gyro::Standard | Gyro::HeavyDuty => 4,
                Gyro::Xl => 6,
                Gyro::Compact => 2,
            };
            let standard = if construction.engine == Engine::Compact {
                3
            } else {
                6
            };
            let engine = plan.engine_slots.unwrap_or(standard);
            ensure!(
                engine <= 12 - gyro,
                "engine and gyro slots run past the section"
            );
            let front = engine.min(3);
            items.extend((0..front).map(|slot| (slot, ENGINE)));
            items.extend((3..3 + gyro).map(|slot| (slot, GYRO)));
            items.extend((3 + gyro..engine + gyro).map(|slot| (slot, ENGINE)));
        }
        LeftTorso | RightTorso => {
            let first = plan.engine_at.unwrap_or(0);
            ensure!(
                usize::from(first) + side_engine <= 12,
                "engine slots run past the section"
            );
            items.extend((first..first + side_engine as u8).map(|slot| (slot, ENGINE)));
        }
    }
    Ok(items)
}

/// Whether a stored critical is an item construction places.
pub(super) fn is_fixed_item(equipment: &str) -> bool {
    FIXED_ITEMS.contains(&equipment)
}

/// Biped arms flip when neither arm carries a lower or hand actuator.
pub(super) fn arms_flip(arms: [&SectionDefinition; 2]) -> bool {
    arms.iter().all(|arm| {
        !arm.criticals.values().any(|critical| {
            critical.equipment.eq_ignore_ascii_case(LOWER_ACTUATOR)
                || critical.equipment.eq_ignore_ascii_case(HAND_OR_FOOT)
        })
    })
}

/// Expected mech internal structure from the reference tonnage chart.
///
/// Rows are `[tons, center torso, side torsos, arms, legs]`; quad chassis
/// use the leg column for arms. Head structure is always three and unknown
/// tonnage has no chart value, matching mech_int_check.
pub(super) fn mech_internal(tons: u16, section: BattleSection, quad: bool) -> Option<u16> {
    const STRUCTURE: [[u16; 5]; 19] = [
        [10, 4, 3, 1, 2],
        [15, 5, 4, 2, 3],
        [20, 6, 5, 3, 4],
        [25, 8, 6, 4, 6],
        [30, 10, 7, 5, 7],
        [35, 11, 8, 6, 8],
        [40, 12, 10, 6, 10],
        [45, 14, 11, 7, 11],
        [50, 16, 12, 8, 12],
        [55, 18, 13, 9, 13],
        [60, 20, 14, 10, 14],
        [65, 21, 15, 10, 15],
        [70, 22, 15, 11, 15],
        [75, 23, 16, 12, 16],
        [80, 25, 17, 13, 17],
        [85, 27, 18, 14, 18],
        [90, 29, 19, 15, 19],
        [95, 30, 20, 16, 20],
        [100, 31, 21, 17, 21],
    ];
    let row = STRUCTURE.iter().find(|row| row[0] == tons)?;
    Some(match section {
        BattleSection::Head => 3,
        BattleSection::CenterTorso => row[1],
        BattleSection::LeftTorso | BattleSection::RightTorso => row[2],
        BattleSection::LeftArm | BattleSection::RightArm if quad => row[4],
        BattleSection::LeftArm | BattleSection::RightArm => row[3],
        BattleSection::LeftLeg | BattleSection::RightLeg => row[4],
    })
}

/// Internal structure of every structured vehicle location, from tonnage (vehicle_int_check).
pub(super) fn vehicle_internal(tons: i64) -> u16 {
    u16::try_from((tons + 5).max(10) / 10).unwrap_or(u16::MAX)
}

/// Whether a class takes its internal structure from tonnage rather than its document.
pub(super) fn derives_internals(class: RawUnitClass) -> bool {
    matches!(
        class,
        RawUnitClass::Mech | RawUnitClass::Vehicle | RawUnitClass::Vtol
    )
}

/// A speed in km/h as whole movement points, when it is one.
pub(super) fn movement_points(speed: f64) -> Option<i64> {
    let points = (speed / SPEED_PER_MP).round();
    (points * SPEED_PER_MP == speed && points >= 0.0).then_some(points as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_round_trip_through_flags() {
        let construction = Construction {
            clan: true,
            mixed: vec![Component::Engine, Component::Armor],
            engine: Engine::Xl,
            gyro: Gyro::HeavyDuty,
            cockpit: Cockpit::Small,
            structure: Structure::EndoSteel,
            armor: Armor::FerroFibrous,
            heat_sinks: HeatSinks::Double,
            myomer: Myomer::TripleStrength,
        };
        let flags = construction.flags();
        assert!(!flags.contains(&"DoubleHS"));
        let mut with_extra = flags.clone();
        with_extra.push("Searchlight");
        let (recovered, remaining) = Construction::from_flags(&with_extra).unwrap();
        assert_eq!(recovered, construction);
        assert_eq!(remaining, ["Searchlight"]);
        assert_eq!(
            Construction::from_flags(&["HDGYRO", "SMCPIT"]).unwrap().0,
            Construction {
                gyro: Gyro::HeavyDuty,
                cockpit: Cockpit::Small,
                ..Construction::default()
            }
        );
        assert!(Construction::from_flags(&["XLEngine_Tech", "XXL_Tech"]).is_err());
        assert_eq!(
            Construction::from_flags(&["ICEEngine_Tech", "CompactEngine_Tech"])
                .unwrap()
                .0
                .engine,
            Engine::Ice
        );
    }

    #[test]
    fn construction_tables_decode_with_defaults_by_tech_base() {
        let table =
            |source: &str| Construction::decode(toml::from_str::<toml::Value>(source).unwrap());
        let clan = table("tech_base = \"clan\"").unwrap();
        assert_eq!(clan.heat_sinks, HeatSinks::Double);
        assert_eq!(clan.flags(), ["Clan"]);
        assert_eq!(clan.render(), "\n[construction]\ntech_base = \"clan\"\n");
        assert_eq!(table("").unwrap().render(), "");
        assert!(table("tech_base = \"clan\"\nheat_sinks = \"single\"").is_err());
        // A component from the other technology base round-trips; one from the chassis base is
        // the default and is not recorded.
        let mixed = table("engine_tech = \"clan\"\narmor_tech = \"inner_sphere\"").unwrap();
        assert_eq!(mixed.mixed, [Component::Engine]);
        assert!(mixed.clan_component(Component::Engine));
        assert!(!mixed.clan_component(Component::Armor));
        assert_eq!(mixed.flags(), ["ClanEngine_Tech"]);
        assert_eq!(mixed.render(), "\n[construction]\nengine_tech = \"clan\"\n");
        let clan_mixed = table("tech_base = \"clan\"\nstructure_tech = \"inner_sphere\"").unwrap();
        assert_eq!(clan_mixed.flags(), ["Clan", "ISStructure_Tech"]);
        assert_eq!(
            Construction::from_flags(&clan_mixed.flags()).unwrap().0,
            clan_mixed
        );
        assert!(Construction::from_flags(&["Clan", "ClanEngine_Tech"]).is_err());
        assert!(table("engine_tech = \"periphery\"").is_err());
        assert!(table("engine = \"warp\"").is_err());
        assert!(table("colour = \"red\"").is_err());
    }

    #[test]
    fn specials_are_canonical_and_exclude_construction_flags() {
        assert_eq!(canonical_special("searchlight").unwrap(), "SearchLight");
        assert_eq!(canonical_special("WDOG").unwrap(), "WatchDog_Tech");
        for flag in [
            "XLEngine_Tech",
            "clan",
            "DoubleHS",
            "HARM",
            "FlipArms",
            "Unknown",
        ] {
            assert!(canonical_special(flag).is_err(), "{flag}");
        }
    }

    #[test]
    fn fixed_equipment_follows_engine_gyro_cockpit_and_actuators() {
        use BattleSection::*;
        let place = |construction: &Construction, section, plan| {
            fixed_equipment(construction, BattleMechChassis::Biped, section, plan).unwrap()
        };
        let standard = Construction::default();
        let center = place(&standard, CenterTorso, SectionPlan::default());
        assert_eq!(center.len(), 10);
        assert_eq!(center[&3], GYRO);
        assert_eq!(center[&9], ENGINE);
        let xl = Construction {
            engine: Engine::Xl,
            gyro: Gyro::Xl,
            ..Construction::default()
        };
        assert_eq!(place(&xl, CenterTorso, SectionPlan::default())[&11], ENGINE);
        let side = place(
            &xl,
            LeftTorso,
            SectionPlan {
                engine_at: Some(3),
                ..SectionPlan::default()
            },
        );
        assert_eq!(side.keys().copied().collect::<Vec<_>>(), [3, 4, 5]);
        let arm = place(
            &standard,
            LeftArm,
            SectionPlan {
                omit: &[Omission::Lower, Omission::Hand],
                ..SectionPlan::default()
            },
        );
        assert_eq!(arm.len(), 2);
        let small = Construction {
            cockpit: Cockpit::Small,
            ..Construction::default()
        };
        assert_eq!(place(&small, Head, SectionPlan::default())[&3], SENSORS);
        let quad = fixed_equipment(
            &standard,
            BattleMechChassis::Quad,
            LeftArm,
            SectionPlan::default(),
        )
        .unwrap();
        assert_eq!(quad[&3], HAND_OR_FOOT);
        assert!(
            fixed_equipment(
                &standard,
                BattleMechChassis::Quad,
                LeftArm,
                SectionPlan {
                    omit: &[Omission::Hand],
                    ..SectionPlan::default()
                }
            )
            .is_err()
        );
        assert!(
            fixed_equipment(
                &standard,
                BattleMechChassis::Biped,
                LeftTorso,
                SectionPlan {
                    engine_at: Some(2),
                    ..SectionPlan::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn small_cockpits_construct_with_their_single_life_support() {
        let jenner = include_str!("../../tests/fixtures/btech/mechs/JR7-D.toml");
        let small = jenner
            .replace(
                "{ at = 4, item = \"HeatSink\" }",
                "{ at = 5, item = \"HeatSink\" }",
            )
            .replacen(
                "\n[sections.",
                "\n[construction]\ncockpit = \"small\"\n\n[sections.",
                1,
            );
        let template = super::super::BattleTemplate::parse("JR7-D", &small).unwrap();
        let head = &template.sections[&BattleSection::Head];
        assert_eq!(
            head.criticals
                .values()
                .filter(|critical| critical.equipment == LIFE_SUPPORT)
                .count(),
            1
        );
        assert!(super::super::BattleUnit::from_template(template).is_ok());

        let mut flagged = super::super::BattleTemplate::parse("JR7-D", jenner).unwrap();
        flagged
            .attributes
            .insert("specials".into(), "SmallCockpit_Tech".into());
        assert!(super::super::BattleUnit::from_template(flagged).is_err());
    }

    #[test]
    fn derived_numbers_follow_tonnage_and_movement_points() {
        assert_eq!(
            mech_internal(80, BattleSection::CenterTorso, false),
            Some(25)
        );
        assert_eq!(mech_internal(80, BattleSection::LeftArm, true), Some(17));
        assert_eq!(mech_internal(82, BattleSection::Head, false), None);
        assert_eq!(vehicle_internal(80), 8);
        assert_eq!(vehicle_internal(5), 1);
        assert_eq!(movement_points(64.5), Some(6));
        assert_eq!(movement_points(32.5), None);
    }
}
