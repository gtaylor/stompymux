//! TOML unit template documents: the on-disk syntax shared by every unit class.
//!
//! A document decodes into [`ParsedTemplate`], the class-neutral field and
//! section layout that the mech, vehicle and raw decoders validate further.
//! Documents state construction choices rather than their consequences: the
//! loader derives technology flags from `[construction]`, places the engine,
//! gyro, cockpit and actuators in mech sections, fills internal structure from
//! tonnage and converts movement points to speeds. Rendering goes the other
//! way, so saved units use the same syntax as the stock assets.
//!
//! ```toml
//! name = "Zeus"
//! class = "mech"
//! movement = "biped"
//! tons = 80
//! walk_mp = 6
//!
//! [construction]
//! engine = "xl"
//! heat_sinks = "double"
//!
//! [sections.left_arm]
//! armor = 22
//! omit = ["hand_actuator"]
//! slots = [
//!     { at = "4-6", item = "IS.ERPPC" },
//! ]
//!
//! [[split_mounts]]
//! item = "IS.AC/20"
//! placements = [
//!     { section = "center_torso", at = "11-12" },
//!     { section = "left_torso", at = "4-11" },
//! ]
//! ```
use super::construction::{
    Construction, FLIP_ARMS, Omission, SPEED_PER_MP, SectionPlan, arms_flip,
    canonical_infantry_special, canonical_special, derives_internals, fixed_equipment,
    is_fixed_item, mech_internal, movement_points, vehicle_internal,
};
use super::{
    BattleMechChassis, BattleSection, CriticalDefinition, RawMovement, RawUnitClass,
    SectionDefinition,
};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Largest template source accepted from disk or scripts.
pub const TEMPLATE_SIZE_LIMIT: usize = 1_048_576;

/// Equipment name of the internal slot that extends a split mount into a left-side section.
pub const SPLIT_LEFT: &str = "SplitCrit_Left";

/// Equipment name of the internal slot that extends a split mount into a right-side section.
pub const SPLIT_RIGHT: &str = "SplitCrit_Right";

/// How a unit-level document value is typed.
#[derive(Clone, Copy)]
enum Kind {
    Integer,
    Float,
    Text,
    Flags,
}

/// Unit-level document keys, their internal attribute names and types, in rendering order.
const FIELDS: &[(&str, &str, Kind)] = &[
    ("name", "name", Kind::Text),
    ("class", "type", Kind::Text),
    ("movement", "move_type", Kind::Text),
    ("tons", "tons", Kind::Integer),
    ("max_speed", "max_speed", Kind::Float),
    ("jump_speed", "jump_speed", Kind::Float),
    ("template_speed", "template_speed", Kind::Float),
    ("heat_sinks", "heat_sinks", Kind::Integer),
    ("hs_engine_override", "hsengoverride", Kind::Integer),
    ("radio", "radio", Kind::Integer),
    ("radio_type", "radiotype", Kind::Integer),
    ("radio_range", "radio_range", Kind::Integer),
    ("tac_range", "tac_range", Kind::Integer),
    ("lrs_range", "lrs_range", Kind::Integer),
    ("scan_range", "scan_range", Kind::Integer),
    ("si", "si", Kind::Integer),
    ("fuel", "fuel", Kind::Integer),
    ("cargo_space", "cargo_space", Kind::Integer),
    ("max_suits", "max_suits", Kind::Integer),
    ("max_ton", "max_ton", Kind::Integer),
    (
        "carrier_maximum_tonnage",
        "carrier_maximum_tonnage",
        Kind::Integer,
    ),
    ("unit_era", "unit_era", Kind::Text),
    ("unit_tro", "unit_tro", Kind::Text),
    ("specials", "specials", Kind::Flags),
    ("infantry_specials", "infantryspecials", Kind::Flags),
];

/// Movement point keys and the speed attribute each one sets.
const MOVEMENT_POINTS: [(&str, &str); 2] = [("walk_mp", "max_speed"), ("jump_mp", "jump_speed")];

/// Internal attributes that a rendered document carries elsewhere or not at all.
const UNRENDERED: &[&str] = &[
    "reference",
    "comment",
    "administrative_tonnage",
    "administrative_unit_type",
    "administrative_movement_type",
];

/// One section's protection and occupied slots.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SectionDocument {
    #[serde(default)]
    armor: u16,
    internals: Option<u16>,
    #[serde(default)]
    rear: u16,
    config: Option<String>,
    /// The slots list the section's fixed equipment too; nothing is placed for it.
    #[serde(default)]
    explicit: bool,
    #[serde(default)]
    omit: Vec<String>,
    engine_at: Option<i64>,
    engine_slots: Option<i64>,
    #[serde(default)]
    slots: Vec<SlotDocument>,
}

/// One item occupying a contiguous run of slots in a single section.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotDocument {
    at: SlotRange,
    item: String,
    rounds: Option<u32>,
    link: Option<i32>,
    #[serde(default)]
    modes: Vec<String>,
}

/// A weapon whose slots continue from its primary section into one adjacent section.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SplitMountDocument {
    item: String,
    #[serde(default)]
    modes: Vec<String>,
    placements: Vec<PlacementDocument>,
}

/// The slots one split mount occupies in one section.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementDocument {
    section: String,
    at: SlotRange,
}

/// A one-based slot or an inclusive `"first-last"` run.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SlotRange {
    One(i64),
    Run(String),
}

impl SlotRange {
    /// Decode into an inclusive zero-based range within a twelve-slot section.
    fn bounds(&self) -> Result<(u8, u8)> {
        let (first, last) = match self {
            Self::One(slot) => (*slot, *slot),
            Self::Run(run) => {
                let (first, last) = run.split_once('-').unwrap_or((run, run));
                (
                    first.trim().parse().context("invalid first slot")?,
                    last.trim().parse().context("invalid last slot")?,
                )
            }
        };
        ensure!(
            (1..=12).contains(&first) && (first..=12).contains(&last),
            "slots must be between 1 and 12 in ascending order"
        );
        Ok((first as u8 - 1, last as u8 - 1))
    }
}

/// Class-neutral fields and section layouts, before unit-specific validation.
///
/// Field keys use the internal attribute names in [`FIELDS`]; section keys are
/// lowercase template headings such as `left_arm` or `front_left_leg`.
pub struct ParsedTemplate {
    pub fields: BTreeMap<String, String>,
    pub sections: BTreeMap<String, SectionDefinition>,
}

/// Unit facts the section decoder needs, read from the unit-level fields first.
struct Unit {
    class: Option<RawUnitClass>,
    chassis: Option<BattleMechChassis>,
    tons: i64,
    construction: Construction,
}

impl Unit {
    /// Whether construction places fixed equipment: constructed biped and quad mechs.
    fn places_equipment(&self) -> bool {
        self.chassis.is_some()
    }
}

impl ParsedTemplate {
    /// Decode a TOML document, recording `reference` as the unit's identity.
    pub fn parse(reference: &str, source: &str) -> Result<Self> {
        ensure!(
            source.len() <= TEMPLATE_SIZE_LIMIT,
            "template exceeds size limit"
        );
        let mut document: toml::Table = toml::from_str(source)?;
        let sections = document.remove("sections");
        let split_mounts = document.remove("split_mounts");
        let construction = document.remove("construction");
        let construction = construction
            .map(Construction::decode)
            .transpose()
            .context("invalid construction")?
            .unwrap_or_default();
        let mut fields = BTreeMap::new();
        if !reference.is_empty() {
            fields.insert("reference".to_owned(), reference.to_owned());
        }
        for (key, value) in document {
            if let Some((_, speed)) = MOVEMENT_POINTS.iter().find(|(name, _)| *name == key) {
                let toml::Value::Integer(points) = value else {
                    bail!("{key} must be an integer");
                };
                ensure!((0..=100).contains(&points), "{key} is out of range");
                ensure!(
                    fields
                        .insert(
                            (*speed).to_owned(),
                            (points as f64 * SPEED_PER_MP).to_string()
                        )
                        .is_none(),
                    "give either {key} or {speed}, not both"
                );
                continue;
            }
            let (_, attribute, kind) = FIELDS
                .iter()
                .find(|(name, _, _)| *name == key)
                .with_context(|| format!("unsupported template field {key}"))?;
            if let Some(value) = decode_field(&key, *kind, value)? {
                ensure!(
                    fields.insert((*attribute).to_owned(), value).is_none(),
                    "give either {attribute} or its movement points, not both"
                );
            }
        }
        let flags: Vec<_> = construction.flags();
        if !flags.is_empty() {
            let specials = fields.remove("specials").unwrap_or_default();
            let combined: Vec<&str> = flags
                .iter()
                .copied()
                .chain(specials.split_ascii_whitespace())
                .collect();
            fields.insert("specials".into(), combined.join(" "));
        }
        let class = fields
            .get("type")
            .map(|class| RawUnitClass::parse(class))
            .transpose()?;
        let chassis = match class {
            Some(RawUnitClass::Mech) => fields
                .get("move_type")
                .and_then(|movement| BattleMechChassis::parse(movement).ok()),
            _ => None,
        };
        let unit = Unit {
            class,
            chassis,
            tons: fields
                .get("tons")
                .and_then(|tons| tons.parse().ok())
                .unwrap_or_default(),
            construction,
        };
        let mut parsed = Self {
            fields,
            sections: BTreeMap::new(),
        };
        if let Some(sections) = sections {
            let toml::Value::Table(sections) = sections else {
                bail!("sections must be a table");
            };
            for (name, section) in sections {
                parsed
                    .add_section(&name, section, &unit)
                    .with_context(|| format!("section {name}"))?;
            }
        }
        if let Some(mounts) = split_mounts {
            let mounts: Vec<SplitMountDocument> =
                mounts.try_into().context("invalid split_mounts")?;
            for mount in mounts {
                let item = mount.item.clone();
                parsed
                    .add_split_mount(mount)
                    .with_context(|| format!("split mount {item}"))?;
            }
        }
        if unit.places_equipment() && unit.chassis == Some(BattleMechChassis::Biped) {
            parsed.derive_flip_arms();
        }
        Ok(parsed)
    }

    /// Require a nonempty unit-level field while preserving its spelling.
    pub fn required(&self, name: &str) -> Result<String> {
        self.fields
            .get(name)
            .filter(|value| !value.is_empty())
            .cloned()
            .with_context(|| format!("missing template field {name}"))
    }

    /// Decode one `[sections.<name>]` table, placing its fixed equipment and structure.
    fn add_section(&mut self, name: &str, section: toml::Value, unit: &Unit) -> Result<()> {
        let key = name.to_ascii_lowercase();
        ensure!(!self.sections.contains_key(&key), "duplicate section");
        let section: SectionDocument = section.try_into()?;
        let mech = match unit.class {
            Some(RawUnitClass::Mech) => mech_section(&key).ok(),
            _ => None,
        };
        let constructed = unit.places_equipment() && !section.explicit;
        ensure!(
            unit.places_equipment() || !section.explicit,
            "only mech sections can be explicit"
        );
        ensure!(
            constructed
                || (section.omit.is_empty()
                    && section.engine_at.is_none()
                    && section.engine_slots.is_none()),
            "omit, engine_at and engine_slots need constructed mech equipment"
        );
        let internal = match section.internals {
            Some(internal) => internal,
            None => default_internal(unit, mech).with_context(|| {
                format!("internals are required for {key} at {} tons", unit.tons)
            })?,
        };
        let mut layout = SectionDefinition {
            armor: section.armor,
            internal,
            rear: section.rear,
            criticals: BTreeMap::new(),
            configuration: section.config,
        };
        if constructed {
            let (Some(chassis), Some(mech)) = (unit.chassis, mech) else {
                bail!("unknown mech section");
            };
            let omit = section
                .omit
                .iter()
                .map(|omission| Omission::parse(omission))
                .collect::<Result<Vec<_>>>()?;
            let engine_at = section
                .engine_at
                .map(|slot| {
                    ensure!(
                        (1..=12).contains(&slot),
                        "engine_at must be a slot from 1 to 12"
                    );
                    Ok(slot as u8 - 1)
                })
                .transpose()?;
            let engine_slots = section
                .engine_slots
                .map(|count| {
                    ensure!(
                        (0..=12).contains(&count),
                        "engine_slots must be from 0 to 12"
                    );
                    Ok(count as u8)
                })
                .transpose()?;
            let plan = SectionPlan {
                omit: &omit,
                engine_at,
                engine_slots,
            };
            for (slot, item) in fixed_equipment(&unit.construction, chassis, mech, plan)? {
                let critical = CriticalDefinition {
                    equipment: item.into(),
                    data: "-".into(),
                    modes: Vec::new(),
                };
                occupy(&mut layout, slot, slot, &critical)?;
            }
        }
        for slot in section.slots {
            let (first, last) = slot
                .at
                .bounds()
                .with_context(|| format!("item {}", slot.item))?;
            ensure!(
                !constructed || !is_fixed_item(&slot.item),
                "{} is placed by construction; mark the section explicit to list it",
                slot.item
            );
            let critical = slot_critical(slot)?;
            occupy(&mut layout, first, last, &critical)
                .context("slot already holds fixed equipment or another item")?;
        }
        self.sections.insert(key, layout);
        Ok(())
    }

    /// Expand a split mount into its primary run plus linked extension slots.
    fn add_split_mount(&mut self, mount: SplitMountDocument) -> Result<()> {
        ensure!(
            self.fields
                .get("type")
                .is_some_and(|class| class.eq_ignore_ascii_case("Mech")),
            "only mechs support split mounts"
        );
        let [primary, extension] = mount.placements.as_slice() else {
            bail!("split mounts span exactly two sections");
        };
        let primary_section = mech_section(&primary.section)?;
        let extension_section = mech_section(&extension.section)?;
        ensure!(
            split_adjacent(primary_section, extension_section),
            "{} and {} are not adjacent",
            primary.section,
            extension.section
        );
        ensure!(!is_ammunition(&mount.item), "ammunition cannot be split");
        let (first, last) = primary.at.bounds()?;
        let weapon = CriticalDefinition {
            equipment: mount.item,
            data: "-".into(),
            modes: mount.modes,
        };
        let proxy = CriticalDefinition {
            equipment: split_proxy_name(primary_section, extension_section).into(),
            data: split_link_data(primary_section, first),
            modes: Vec::new(),
        };
        let layout = self
            .sections
            .get_mut(&primary.section.to_ascii_lowercase())
            .with_context(|| format!("missing section {}", primary.section))?;
        occupy(layout, first, last, &weapon)?;
        let (first, last) = extension.at.bounds()?;
        let layout = self
            .sections
            .get_mut(&extension.section.to_ascii_lowercase())
            .with_context(|| format!("missing section {}", extension.section))?;
        occupy(layout, first, last, &proxy)
    }

    /// Record flippable arms exactly when neither biped arm has a lower or hand actuator.
    fn derive_flip_arms(&mut self) {
        let (Some(left), Some(right)) = (
            self.sections.get("left_arm"),
            self.sections.get("right_arm"),
        ) else {
            return;
        };
        if !arms_flip([left, right]) {
            return;
        }
        let specials = self.fields.entry("specials".into()).or_default();
        if !specials.is_empty() {
            specials.push(' ');
        }
        specials.push_str(FLIP_ARMS);
    }
}

/// Internal structure a section receives when its document leaves it out.
fn default_internal(unit: &Unit, mech: Option<BattleSection>) -> Option<u16> {
    let class = unit.class?;
    if !derives_internals(class) {
        return Some(0);
    }
    match mech {
        Some(section) => mech_internal(
            u16::try_from(unit.tons).ok()?,
            section,
            unit.chassis == Some(BattleMechChassis::Quad),
        ),
        None => Some(vehicle_internal(unit.tons)),
    }
}

/// Convert one typed document value into its internal attribute spelling.
fn decode_field(key: &str, kind: Kind, value: toml::Value) -> Result<Option<String>> {
    Ok(Some(match (kind, value) {
        (Kind::Integer, toml::Value::Integer(value)) => i32::try_from(value)
            .with_context(|| format!("{key} is out of range"))?
            .to_string(),
        (Kind::Float, toml::Value::Integer(value)) => (value as f64).to_string(),
        (Kind::Float, toml::Value::Float(value)) => {
            ensure!(value.is_finite(), "{key} must be finite");
            value.to_string()
        }
        (Kind::Text, toml::Value::String(value)) => match key {
            "class" => {
                ensure!(
                    value == value.to_ascii_lowercase(),
                    "class must be lowercase"
                );
                RawUnitClass::parse(&value)?.name().to_owned()
            }
            "movement" => {
                ensure!(
                    value == value.to_ascii_lowercase(),
                    "movement must be lowercase"
                );
                RawMovement::parse(&value)?.name().to_owned()
            }
            _ => value,
        },
        (Kind::Flags, toml::Value::Array(values)) => {
            let mut flags: Vec<String> = Vec::new();
            for value in values {
                let toml::Value::String(flag) = value else {
                    bail!("{key} entries must be strings");
                };
                ensure!(
                    !flag.is_empty() && !flag.contains(char::is_whitespace),
                    "{key} entries must be single words"
                );
                let flag = if key == "specials" {
                    canonical_special(&flag)?
                } else {
                    canonical_infantry_special(&flag)?
                };
                if !flags.iter().any(|known| known == flag) {
                    flags.push(flag.to_owned());
                }
            }
            if flags.is_empty() {
                return Ok(None);
            }
            flags.join(" ")
        }
        (Kind::Integer, _) => bail!("{key} must be an integer"),
        (Kind::Float, _) => bail!("{key} must be a number"),
        (Kind::Text, _) => bail!("{key} must be a string"),
        (Kind::Flags, _) => bail!("{key} must be an array of strings"),
    }))
}

/// Build the stored critical for one single-section slot entry.
fn slot_critical(slot: SlotDocument) -> Result<CriticalDefinition> {
    let ammunition = is_ammunition(&slot.item);
    ensure!(
        !slot.item.eq_ignore_ascii_case(SPLIT_LEFT) && !slot.item.eq_ignore_ascii_case(SPLIT_RIGHT),
        "use split_mounts for weapons that span sections"
    );
    ensure!(
        ammunition || slot.rounds.is_none(),
        "only ammunition takes rounds"
    );
    ensure!(
        !ammunition || slot.link.is_none(),
        "ammunition cannot take a link"
    );
    ensure!(
        slot.modes
            .iter()
            .all(|mode| !mode.is_empty() && !mode.contains(char::is_whitespace)),
        "modes must be single words"
    );
    let data = slot
        .rounds
        .map(|rounds| rounds.to_string())
        .or_else(|| slot.link.map(|link| link.to_string()))
        .unwrap_or_else(|| "-".into());
    Ok(CriticalDefinition {
        equipment: slot.item,
        data,
        modes: slot.modes,
    })
}

/// Install one critical across an inclusive slot range, refusing overlaps.
fn occupy(
    layout: &mut SectionDefinition,
    first: u8,
    last: u8,
    critical: &CriticalDefinition,
) -> Result<()> {
    for slot in first..=last {
        ensure!(
            layout.criticals.insert(slot, critical.clone()).is_none(),
            "overlapping slot {}",
            slot + 1
        );
    }
    Ok(())
}

/// Resolve a biped or quad section heading to its stable mech section.
fn mech_section(heading: &str) -> Result<BattleSection> {
    super::BattleMechChassis::Biped
        .parse_section(heading)
        .or_else(|_| super::BattleMechChassis::Quad.parse_section(heading))
}

/// Whether a split mount may continue from one section into the other.
pub fn split_adjacent(primary: BattleSection, extension: BattleSection) -> bool {
    use BattleSection::*;
    matches!(
        (primary, extension),
        (LeftArm | LeftLeg | CenterTorso, LeftTorso)
            | (LeftTorso, LeftArm | LeftLeg | CenterTorso)
            | (RightArm | RightLeg | CenterTorso, RightTorso)
            | (RightTorso, RightArm | RightLeg | CenterTorso)
    )
}

/// The extension marker names the side of the body the mount occupies.
pub fn split_proxy_name(primary: BattleSection, extension: BattleSection) -> &'static str {
    use BattleSection::*;
    if [primary, extension]
        .iter()
        .any(|section| matches!(section, LeftArm | LeftTorso | LeftLeg))
    {
        SPLIT_LEFT
    } else {
        SPLIT_RIGHT
    }
}

/// Encode an extension slot's link to the primary section and its first zero-based slot.
pub fn split_link_data(section: BattleSection, slot: u8) -> String {
    format!("{}:{slot}", section.name())
}

/// Decode an extension slot's link to the primary section and its first zero-based slot.
pub fn parse_split_link(data: &str) -> Result<(BattleSection, u8)> {
    let (section, slot) = data
        .split_once(':')
        .context("Invalid split critical parent")?;
    Ok((
        BattleSection::parse(section)?,
        slot.parse().context("Invalid split critical parent slot")?,
    ))
}

/// One section to render, in document order.
pub struct RenderSection<'a> {
    /// Lowercase document heading such as `left_arm`.
    pub heading: String,
    /// The stable mech section, so split links and fixed equipment can name it.
    pub mech: Option<BattleSection>,
    pub layout: &'a SectionDefinition,
}

/// Render internal attributes and section layouts as a TOML template document,
/// stating construction choices and leaving out everything they imply.
pub fn render(
    attributes: &BTreeMap<String, String>,
    sections: &[RenderSection<'_>],
) -> Result<String> {
    for key in attributes.keys() {
        ensure!(
            UNRENDERED.contains(&key.as_str())
                || FIELDS.iter().any(|(_, attribute, _)| attribute == key),
            "unsupported template attribute {key}"
        );
    }
    let flags: Vec<&str> = attributes
        .get("specials")
        .map(|specials| {
            specials
                .split_ascii_whitespace()
                .filter(|flag| *flag != "-")
                .collect()
        })
        .unwrap_or_default();
    let (construction, specials) = Construction::from_flags(&flags)?;
    let class = attributes
        .get("type")
        .map(|class| RawUnitClass::parse(class))
        .transpose()?;
    let chassis = match class {
        Some(RawUnitClass::Mech) => attributes
            .get("move_type")
            .and_then(|movement| BattleMechChassis::parse(movement).ok()),
        _ => None,
    };
    let unit = Unit {
        class,
        chassis,
        tons: attributes
            .get("tons")
            .and_then(|tons| tons.trim().parse().ok())
            .unwrap_or_default(),
        construction,
    };
    let mut output = String::new();
    for (name, attribute, kind) in FIELDS {
        let Some(value) = attributes.get(*attribute) else {
            continue;
        };
        let rendered = match kind {
            Kind::Integer => value
                .trim()
                .parse::<i64>()
                .with_context(|| format!("invalid {attribute} {value}"))?
                .to_string(),
            Kind::Float => {
                let value: f64 = value
                    .trim()
                    .parse()
                    .with_context(|| format!("invalid {attribute} {value}"))?;
                let points = MOVEMENT_POINTS
                    .iter()
                    .find(|(_, speed)| speed == attribute)
                    .and_then(|(key, _)| Some((key, movement_points(value)?)));
                if let Some((key, points)) = points {
                    writeln!(output, "{key} = {points}")?;
                    continue;
                }
                toml::Value::Float(value).to_string()
            }
            Kind::Text if matches!(*name, "class" | "movement") => {
                quote(&value.to_ascii_lowercase())
            }
            Kind::Text => quote(value),
            Kind::Flags => {
                let flags = if *attribute == "specials" {
                    specials
                        .iter()
                        .map(|flag| canonical_special(flag).map(str::to_owned))
                        .collect::<Result<Vec<_>>>()?
                } else {
                    value
                        .split_ascii_whitespace()
                        .filter(|flag| *flag != "-")
                        .map(|flag| canonical_infantry_special(flag).map(str::to_owned))
                        .collect::<Result<Vec<_>>>()?
                };
                if flags.is_empty() {
                    continue;
                }
                quoted_list(&flags)
            }
        };
        writeln!(output, "{name} = {rendered}")?;
    }
    output.push_str(&unit.construction.render());
    let mounts = split_mounts(sections)?;
    for section in sections {
        render_section(&mut output, section, &mounts, &unit)?;
    }
    for mount in &mounts {
        render_split_mount(&mut output, mount, sections)?;
    }
    Ok(output)
}

/// How construction reproduces a section's fixed equipment, if it can.
struct SectionFit {
    omit: Vec<Omission>,
    engine_at: Option<u8>,
    engine_slots: Option<u8>,
    placed: BTreeMap<u8, &'static str>,
}

/// Find the omissions and engine placement under which construction places exactly
/// the fixed equipment a mech section holds; `None` means the section must be explicit.
fn fit_section(
    unit: &Unit,
    section: BattleSection,
    layout: &SectionDefinition,
) -> Option<SectionFit> {
    use BattleSection::*;
    use Omission::*;
    let chassis = unit.chassis?;
    let omissions: Vec<Vec<Omission>> = match section {
        LeftArm | RightArm | LeftLeg | RightLeg => {
            let last =
                if chassis == BattleMechChassis::Biped && matches!(section, LeftArm | RightArm) {
                    Hand
                } else {
                    Foot
                };
            let actuators = [Shoulder, Upper, Lower, last];
            (0..16u8)
                .map(|mask| {
                    actuators
                        .iter()
                        .enumerate()
                        .filter(|(bit, _)| mask & (1 << bit) != 0)
                        .map(|(_, omission)| *omission)
                        .collect()
                })
                .collect()
        }
        _ => vec![Vec::new()],
    };
    let engines = layout
        .criticals
        .values()
        .filter(|critical| critical.equipment == "Engine")
        .count();
    let engine_at = layout
        .criticals
        .iter()
        .find(|(_, critical)| critical.equipment == "Engine")
        .map(|(slot, _)| *slot)
        .filter(|slot| *slot != 0 && matches!(section, LeftTorso | RightTorso));
    let engine_counts: &[Option<u8>] = match section {
        LeftTorso | RightTorso | CenterTorso => &[None, Some(engines as u8)],
        _ => &[None],
    };
    omissions.iter().find_map(|omit| {
        engine_counts.iter().find_map(|&engine_slots| {
            let plan = SectionPlan {
                omit,
                engine_at,
                engine_slots,
            };
            let placed = fixed_equipment(&unit.construction, chassis, section, plan).ok()?;
            let reproduced = placed.iter().all(|(slot, item)| {
                layout.criticals.get(slot).is_some_and(|critical| {
                    critical.equipment == *item && critical.data == "-" && critical.modes.is_empty()
                })
            }) && layout.criticals.iter().all(|(slot, critical)| {
                !is_fixed_item(&critical.equipment) || placed.contains_key(slot)
            });
            reproduced.then(|| SectionFit {
                omit: omit.clone(),
                engine_at,
                engine_slots,
                placed,
            })
        })
    })
}

/// A split mount recovered from its primary run and linked extension slots.
struct RenderedMount {
    primary: BattleSection,
    first: u8,
    last: u8,
    extension: BattleSection,
    extension_slots: Vec<u8>,
}

/// Group linked extension slots with the primary runs they continue.
fn split_mounts(sections: &[RenderSection<'_>]) -> Result<Vec<RenderedMount>> {
    let mut mounts: Vec<RenderedMount> = Vec::new();
    for section in sections {
        for (&slot, critical) in &section.layout.criticals {
            if !is_split_proxy(&critical.equipment) {
                continue;
            }
            let extension = section.mech.context("split mount outside a mech section")?;
            let (primary, first) = parse_split_link(&critical.data)?;
            if let Some(mount) = mounts
                .iter_mut()
                .find(|mount| mount.primary == primary && mount.first == first)
            {
                ensure!(
                    mount.extension == extension,
                    "split mount spans more than two sections"
                );
                mount.extension_slots.push(slot);
                continue;
            }
            let layout = sections
                .iter()
                .find(|candidate| candidate.mech == Some(primary))
                .map(|candidate| candidate.layout)
                .context("split mount primary section is missing")?;
            let weapon = layout
                .criticals
                .get(&first)
                .context("split mount primary slot is empty")?;
            let mut last = first;
            let local = primary_slot_count(weapon, sections, primary, first);
            while layout.criticals.get(&(last + 1)) == Some(weapon)
                && local.is_none_or(|count| last + 1 < first + count)
            {
                last += 1;
            }
            mounts.push(RenderedMount {
                primary,
                first,
                last,
                extension,
                extension_slots: vec![slot],
            });
        }
    }
    Ok(mounts)
}

/// Slots a split weapon occupies in its primary section, when its profile is known.
fn primary_slot_count(
    weapon: &CriticalDefinition,
    sections: &[RenderSection<'_>],
    primary: BattleSection,
    first: u8,
) -> Option<u8> {
    let total = super::BattleWeapon::parse(&weapon.equipment)
        .ok()?
        .profile()
        .critical_slots;
    let link = split_link_data(primary, first);
    let extension = sections
        .iter()
        .flat_map(|section| section.layout.criticals.values())
        .filter(|critical| is_split_proxy(&critical.equipment) && critical.data == link)
        .count();
    total.checked_sub(u8::try_from(extension).ok()?)
}

/// Whether an item names an ammunition bin.
fn is_ammunition(item: &str) -> bool {
    super::equipment::strip_name_prefix(item, "Ammo_").is_some()
}

/// Whether a stored critical is a split-mount extension marker.
pub fn is_split_proxy(equipment: &str) -> bool {
    equipment.eq_ignore_ascii_case(SPLIT_LEFT) || equipment.eq_ignore_ascii_case(SPLIT_RIGHT)
}

/// Write one section table, compressing identical neighbouring slots into runs.
fn render_section(
    output: &mut String,
    section: &RenderSection<'_>,
    mounts: &[RenderedMount],
    unit: &Unit,
) -> Result<()> {
    let layout = section.layout;
    writeln!(output, "\n[sections.{}]", section.heading)?;
    writeln!(output, "armor = {}", layout.armor)?;
    if default_internal(unit, section.mech) != Some(layout.internal) {
        writeln!(output, "internals = {}", layout.internal)?;
    }
    if layout.rear > 0 {
        writeln!(output, "rear = {}", layout.rear)?;
    }
    if let Some(config) = &layout.configuration {
        writeln!(output, "config = {}", quote(config))?;
    }
    let fit = match section.mech {
        Some(mech) if unit.places_equipment() => {
            let fit = fit_section(unit, mech, layout);
            if fit.is_none() {
                writeln!(output, "explicit = true")?;
            }
            fit
        }
        _ => None,
    };
    if let Some(fit) = &fit {
        if !fit.omit.is_empty() {
            let omit: Vec<_> = fit
                .omit
                .iter()
                .map(|omission| omission.spelling().to_owned())
                .collect();
            writeln!(output, "omit = {}", quoted_list(&omit))?;
        }
        if let Some(slot) = fit.engine_at {
            writeln!(output, "engine_at = {}", slot + 1)?;
        }
        if let Some(count) = fit.engine_slots {
            writeln!(output, "engine_slots = {count}")?;
        }
    }
    let in_mount = |slot: u8| {
        fit.as_ref()
            .is_some_and(|fit| fit.placed.contains_key(&slot))
            || mounts.iter().any(|mount| {
                (Some(mount.primary) == section.mech && (mount.first..=mount.last).contains(&slot))
                    || (Some(mount.extension) == section.mech
                        && mount.extension_slots.contains(&slot))
            })
    };
    let mut runs: Vec<(u8, u8, &CriticalDefinition)> = Vec::new();
    for (&slot, critical) in &layout.criticals {
        if in_mount(slot) {
            continue;
        }
        ensure!(
            !is_split_proxy(&critical.equipment),
            "unlinked split critical in {}",
            section.heading
        );
        match runs.last_mut() {
            Some((_, last, previous)) if *last + 1 == slot && *previous == critical => {
                *last = slot;
            }
            _ => runs.push((slot, slot, critical)),
        }
    }
    if runs.is_empty() {
        return Ok(());
    }
    writeln!(output, "slots = [")?;
    for (first, last, critical) in runs {
        let mut entry = format!(
            "at = {}, item = {}",
            range(first, last),
            quote(&critical.equipment)
        );
        if critical.data != "-" {
            let value: i64 = critical.data.parse().with_context(|| {
                format!(
                    "unsupported data {} on {}",
                    critical.data, critical.equipment
                )
            })?;
            let ammunition = is_ammunition(&critical.equipment);
            let key = if ammunition { "rounds" } else { "link" };
            write!(entry, ", {key} = {value}")?;
        }
        push_metadata(&mut entry, critical);
        writeln!(output, "    {{ {entry} }},")?;
    }
    writeln!(output, "]")?;
    Ok(())
}

/// Write one `[[split_mounts]]` entry.
fn render_split_mount(
    output: &mut String,
    mount: &RenderedMount,
    sections: &[RenderSection<'_>],
) -> Result<()> {
    let primary = sections
        .iter()
        .find(|section| section.mech == Some(mount.primary))
        .context("split mount primary section is missing")?;
    let extension = sections
        .iter()
        .find(|section| section.mech == Some(mount.extension))
        .context("split mount extension section is missing")?;
    let weapon = &primary.layout.criticals[&mount.first];
    let first = *mount.extension_slots.iter().min().unwrap();
    let last = *mount.extension_slots.iter().max().unwrap();
    ensure!(
        usize::from(last - first) + 1 == mount.extension_slots.len(),
        "split mount extension is not contiguous"
    );
    writeln!(output, "\n[[split_mounts]]")?;
    writeln!(output, "item = {}", quote(&weapon.equipment))?;
    if !weapon.modes.is_empty() {
        writeln!(output, "modes = {}", quoted_list(&weapon.modes))?;
    }
    writeln!(output, "placements = [")?;
    writeln!(
        output,
        "    {{ section = {}, at = {} }},",
        quote(&primary.heading),
        range(mount.first, mount.last)
    )?;
    writeln!(
        output,
        "    {{ section = {}, at = {} }},",
        quote(&extension.heading),
        range(first, last)
    )?;
    writeln!(output, "]")?;
    Ok(())
}

/// Append modes to an inline slot entry.
fn push_metadata(entry: &mut String, critical: &CriticalDefinition) {
    if !critical.modes.is_empty() {
        let _ = write!(entry, ", modes = {}", quoted_list(&critical.modes));
    }
}

/// Render strings as a one-line TOML array.
fn quoted_list(values: &[String]) -> String {
    let values: Vec<_> = values.iter().map(|value| quote(value)).collect();
    format!("[{}]", values.join(", "))
}

/// Render an inclusive zero-based range as a one-based slot or run.
fn range(first: u8, last: u8) -> String {
    if first == last {
        (first + 1).to_string()
    } else {
        format!("\"{}-{}\"", first + 1, last + 1)
    }
}

/// Quote a TOML basic string.
fn quote(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete standard biped: every fixed item, internal value and flag is implied.
    const ZEUS: &str = r#"
name = "Zeus"
class = "mech"
movement = "biped"
tons = 80
walk_mp = 6
specials = ["searchlight"]

[construction]
engine = "xl"
heat_sinks = "double"

[sections.left_arm]
armor = 22
omit = ["hand_actuator"]
slots = [
    { at = "4-6", item = "IS.ERPPC", modes = ["OnTC"] },
]

[sections.right_arm]
armor = 22
omit = ["lower_actuator", "hand_actuator"]

[sections.left_torso]
armor = 25
rear = 6
engine_at = 4
slots = [
    { at = 1, item = "Ammo_IS.LRM-15", rounds = 8 },
    { at = 2, item = "ArtemisIV", link = 2 },
]

[sections.right_torso]
armor = 25
rear = 6

[sections.center_torso]
armor = 26
rear = 8

[sections.left_leg]
armor = 24

[sections.right_leg]
armor = 24

[sections.head]
armor = 9
"#;

    #[test]
    fn construction_places_fixed_equipment_flags_structure_and_speed() {
        let parsed = ParsedTemplate::parse("ZEU-9S", ZEUS).unwrap();
        assert_eq!(parsed.fields["reference"], "ZEU-9S");
        assert_eq!(parsed.fields["type"], "Mech");
        assert_eq!(parsed.fields["max_speed"], "64.5");
        assert_eq!(
            parsed.fields["specials"],
            "XLEngine_Tech DoubleHS SearchLight"
        );
        let arm = &parsed.sections["left_arm"];
        assert_eq!(arm.internal, 13);
        assert_eq!(arm.criticals[&0].equipment, "ShoulderOrHip");
        assert_eq!(arm.criticals[&2].equipment, "LowerActuator");
        assert_eq!(arm.criticals[&3].equipment, "IS.ERPPC");
        assert_eq!(arm.criticals[&4].modes, ["OnTC"]);
        let torso = &parsed.sections["left_torso"];
        assert_eq!(torso.criticals[&0].data, "8");
        assert_eq!(torso.criticals[&1].data, "2");
        assert_eq!(
            torso
                .criticals
                .iter()
                .filter(|(_, critical)| critical.equipment == "Engine")
                .map(|(slot, _)| *slot)
                .collect::<Vec<_>>(),
            [3, 4, 5]
        );
        assert_eq!(parsed.sections["right_torso"].criticals.len(), 3);
        assert_eq!(parsed.sections["center_torso"].criticals.len(), 10);
        assert_eq!(parsed.sections["center_torso"].internal, 25);
        assert_eq!(parsed.sections["head"].criticals.len(), 5);
        assert_eq!(parsed.sections["head"].internal, 3);

        let flipping = ZEUS.replace(
            "omit = [\"hand_actuator\"]",
            "omit = [\"lower_actuator\", \"hand_actuator\"]",
        );
        let parsed = ParsedTemplate::parse("ZEU-9S", &flipping).unwrap();
        assert!(parsed.fields["specials"].ends_with(" FlipArms"));
    }

    #[test]
    fn malformed_documents_are_rejected() {
        for source in [
            "unknown = 1",
            "tons = \"80\"",
            "tons = 2147483648",
            "class = \"Mech\"",
            "class = \"tank\"",
            "max_speed = nan",
            "walk_mp = 4\nmax_speed = 43.0",
            "walk_mp = -1",
            "specials = [\"Two Words\"]",
            "specials = [\"XLEngine_Tech\"]",
            "specials = [\"FlipArms\"]",
            "specials = [\"Mystery\"]",
            "[construction]\nengine = \"warp\"",
            "[sections.head]\nslots = [{ at = 0, item = \"Cockpit\" }]",
            "[sections.head]\nslots = [{ at = \"3-2\", item = \"Cockpit\" }]",
            "[sections.head]\nslots = [{ at = 13, item = \"Cockpit\" }]",
            "[sections.head]\nslots = [{ at = 1, item = \"Cockpit\", colour = 1 }]",
            "[sections.head]\nslots = [{ at = \"1-2\", item = \"A\" }, { at = 2, item = \"B\" }]",
            "[sections.head]\nslots = [{ at = 1, item = \"IS.SmallLaser\", rounds = 4 }]",
            "[sections.head]\nslots = [{ at = 1, item = \"Ammo_IS.AC/2\", link = 4 }]",
            "[sections.head]\nslots = [{ at = 1, item = \"SplitCrit_Left\", link = 4 }]",
            "[sections.head]\nshield = 1",
            "class = \"mech\"\nmovement = \"biped\"\ntons = 20\n[sections.head]\nslots = [{ at = 3, item = \"IS.SmallLaser\" }]",
            "class = \"mech\"\nmovement = \"biped\"\ntons = 20\n[sections.head]\nslots = [{ at = 4, item = \"Sensors\" }]",
            "class = \"mech\"\nmovement = \"biped\"\ntons = 20\n[sections.left_leg]\nomit = [\"hand_actuator\"]",
            "class = \"mech\"\nmovement = \"biped\"\ntons = 20\n[sections.left_torso]\nengine_at = 3",
            "class = \"mech\"\nmovement = \"biped\"\ntons = 20\n[sections.left_arm]\nengine_slots = 2",
            "class = \"mech\"\nmovement = \"biped\"\ntons = 20\n[sections.center_torso]\nengine_slots = 9",
            "class = \"mech\"\nmovement = \"biped\"\ntons = 22\n[sections.head]",
            "class = \"vehicle\"\nmovement = \"track\"\ntons = 20\n[sections.turret]\nexplicit = true",
        ] {
            assert!(ParsedTemplate::parse("X", source).is_err(), "{source}");
        }
    }

    #[test]
    fn explicit_sections_list_their_own_fixed_equipment() {
        let source = ZEUS.replace(
            "[sections.head]\narmor = 9\n",
            "[sections.head]\narmor = 9\nexplicit = true\nslots = [{ at = 1, item = \"Cockpit\" }]\n",
        );
        let parsed = ParsedTemplate::parse("ZEU-9S", &source).unwrap();
        let head = &parsed.sections["head"];
        assert_eq!(head.criticals.len(), 1);
    }

    #[test]
    fn vehicles_and_other_classes_keep_their_own_structure_rules() {
        let parsed = ParsedTemplate::parse(
            "Truck",
            "class = \"vehicle\"\nmovement = \"wheel\"\ntons = 40\nwalk_mp = 5\n[construction]\nengine = \"ice\"\n[sections.front_side]\narmor = 10\n[sections.turret]\ninternals = 0\n",
        )
        .unwrap();
        assert_eq!(parsed.fields["specials"], "ICEEngine_Tech");
        assert_eq!(parsed.fields["max_speed"], "53.75");
        assert_eq!(parsed.sections["front_side"].internal, 4);
        assert_eq!(parsed.sections["turret"].internal, 0);
        assert!(parsed.sections["front_side"].criticals.is_empty());
        let aero = ParsedTemplate::parse(
            "Aero",
            "class = \"aerofighter\"\nmovement = \"fly\"\ntons = 50\n[sections.nose]\narmor = 3\n",
        )
        .unwrap();
        assert_eq!(aero.sections["nose"].internal, 0);
    }

    #[test]
    fn split_mounts_link_extensions_to_their_primary_run() {
        let source = r#"
class = "mech"
movement = "biped"
tons = 50
[sections.center_torso]
[sections.left_torso]
[sections.left_arm]
[[split_mounts]]
item = "IS.AC/20"
placements = [
    { section = "center_torso", at = "11-12" },
    { section = "left_torso", at = "1-8" },
]
"#;
        let parsed = ParsedTemplate::parse("X", source).unwrap();
        let center = &parsed.sections["center_torso"];
        assert_eq!(center.criticals[&10].equipment, "IS.AC/20");
        let torso = &parsed.sections["left_torso"];
        assert_eq!(torso.criticals.len(), 8);
        assert_eq!(torso.criticals[&0].equipment, SPLIT_LEFT);
        assert_eq!(
            parse_split_link(&torso.criticals[&0].data).unwrap(),
            (BattleSection::CenterTorso, 10)
        );

        for (placements, class) in [
            (
                "{ section = \"left_arm\", at = 5 }, { section = \"center_torso\", at = 11 }",
                "mech",
            ),
            ("{ section = \"left_torso\", at = 1 }", "mech"),
            (
                "{ section = \"left_torso\", at = 1 }, { section = \"center_torso\", at = 11 }, { section = \"left_arm\", at = 5 }",
                "mech",
            ),
            (
                "{ section = \"left_torso\", at = 1 }, { section = \"center_torso\", at = 11 }",
                "vehicle",
            ),
            (
                "{ section = \"center_torso\", at = \"1-2\" }, { section = \"left_torso\", at = \"1-8\" }",
                "mech",
            ),
        ] {
            let source = format!(
                "class = \"{class}\"\nmovement = \"biped\"\ntons = 50\n[sections.center_torso]\n[sections.left_torso]\n[sections.left_arm]\n[[split_mounts]]\nitem = \"IS.AC/20\"\nplacements = [{placements}]"
            );
            assert!(ParsedTemplate::parse("X", &source).is_err(), "{source}");
        }
    }

    /// Render the sections of a parsed mech in anatomical order.
    fn render_mech(parsed: &ParsedTemplate) -> String {
        let sections: Vec<_> = BattleSection::ALL
            .into_iter()
            .filter_map(|mech| {
                let heading = BattleMechChassis::Biped
                    .section_name(mech)
                    .to_ascii_lowercase();
                let layout = parsed.sections.get(&heading)?;
                Some(RenderSection {
                    heading,
                    mech: Some(mech),
                    layout,
                })
            })
            .collect();
        render(&parsed.fields, &sections).unwrap()
    }

    #[test]
    fn rendering_states_choices_and_leaves_out_what_they_imply() {
        let parsed = ParsedTemplate::parse("ZEU-9S", ZEUS).unwrap();
        let rendered = render_mech(&parsed);
        assert!(rendered.contains("walk_mp = 6\n"));
        assert!(rendered.contains("specials = [\"SearchLight\"]\n"));
        assert!(rendered.contains("[construction]\nengine = \"xl\"\nheat_sinks = \"double\"\n"));
        assert!(rendered.contains("omit = [\"hand_actuator\"]"));
        assert!(rendered.contains("engine_at = 4"));
        assert!(!rendered.contains("internals"));
        assert!(!rendered.contains("Engine"));
        assert!(!rendered.contains("FlipArms"));
        let reparsed = ParsedTemplate::parse("ZEU-9S", &rendered).unwrap();
        assert_eq!(reparsed.fields, parsed.fields);
        assert_eq!(reparsed.sections, parsed.sections);

        let mut irregular = ParsedTemplate::parse("ZEU-9S", ZEUS).unwrap();
        let head = irregular.sections.get_mut("head").unwrap();
        head.criticals.get_mut(&1).unwrap().modes = vec!["Destroyed".into()];
        head.internal = 4;
        let rendered = render_mech(&irregular);
        assert!(rendered.contains("[sections.head]\narmor = 9\ninternals = 4\nexplicit = true\n"));
        let reparsed = ParsedTemplate::parse("ZEU-9S", &rendered).unwrap();
        assert_eq!(reparsed.sections["head"], irregular.sections["head"]);
    }

    #[test]
    fn irregular_sections_state_their_differences_instead_of_listing_fixed_equipment() {
        let source = ZEUS
            .replace(
                "omit = [\"lower_actuator\", \"hand_actuator\"]",
                "omit = [\"shoulder\", \"upper_actuator\", \"lower_actuator\", \"hand_actuator\"]",
            )
            .replace(
                "[sections.left_leg]\narmor = 24\n",
                "[sections.left_leg]\narmor = 24\nomit = [\"lower_actuator\"]\n",
            )
            .replace(
                "[sections.right_torso]\narmor = 25\nrear = 6\n",
                "[sections.right_torso]\narmor = 25\nrear = 6\nengine_slots = 1\n",
            )
            .replace(
                "[sections.center_torso]\narmor = 26\nrear = 8\n",
                "[sections.center_torso]\narmor = 26\nrear = 8\nengine_slots = 8\n",
            );
        let parsed = ParsedTemplate::parse("ZEU-9S", &source).unwrap();
        assert!(parsed.sections["right_arm"].criticals.is_empty());
        let leg = &parsed.sections["left_leg"];
        assert!(!leg.criticals.contains_key(&2));
        assert_eq!(leg.criticals[&3].equipment, "HandOrFootActuator");
        let torso = &parsed.sections["right_torso"];
        assert_eq!(torso.criticals.len(), 1);
        let center = &parsed.sections["center_torso"];
        let engines = center
            .criticals
            .values()
            .filter(|critical| critical.equipment == "Engine")
            .count();
        assert_eq!(engines, 8);
        assert_eq!(center.criticals[&11].equipment, "Engine");

        let rendered = render_mech(&parsed);
        for line in [
            "omit = [\"shoulder\", \"upper_actuator\", \"lower_actuator\", \"hand_actuator\"]\n",
            "omit = [\"lower_actuator\"]\n",
            "engine_slots = 1\n",
            "engine_slots = 8\n",
        ] {
            assert!(rendered.contains(line), "{line}");
        }
        assert!(!rendered.contains("explicit"));
        let reparsed = ParsedTemplate::parse("ZEU-9S", &rendered).unwrap();
        assert_eq!(reparsed.sections, parsed.sections);
    }

    #[test]
    fn rendering_round_trips_split_mounts() {
        let source = r#"
name = "Split"
class = "mech"
movement = "quad"
tons = 50
max_speed = 86.5
specials = ["SearchLight"]

[construction]
tech_base = "clan"

[sections.front_left_leg]
armor = 1

[sections.left_torso]
armor = 3
internals = 4
rear = 5
config = "Case"
slots = [
    { at = "1-2", item = "HeatSink" },
    { at = 3, item = "Ammo_IS.AC/20", rounds = 5, modes = ["Halfton"] },
]

[sections.center_torso]
armor = 6

[[split_mounts]]
item = "IS.AC/20"
placements = [
    { section = "center_torso", at = "11-12" },
    { section = "left_torso", at = "4-11" },
]
"#;
        let parsed = ParsedTemplate::parse("X", source).unwrap();
        let sections: Vec<_> = [
            ("front_left_leg", BattleSection::LeftArm),
            ("left_torso", BattleSection::LeftTorso),
            ("center_torso", BattleSection::CenterTorso),
        ]
        .into_iter()
        .map(|(heading, mech)| RenderSection {
            heading: heading.into(),
            mech: Some(mech),
            layout: &parsed.sections[heading],
        })
        .collect();
        let rendered = render(&parsed.fields, &sections).unwrap();
        assert_eq!(rendered.trim(), source.trim());
    }

    #[test]
    fn rendering_rejects_attributes_without_a_document_field() {
        let attributes = BTreeMap::from([("mystery".to_owned(), "1".to_owned())]);
        assert!(render(&attributes, &[]).is_err());
    }
}
