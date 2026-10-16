//! Resolves MegaMek equipment names, as written in `.mtf` critical slots and `.blk` equipment
//! blocks, to stompymux template items. Anything stompymux has no item for is an error rather
//! than a silent omission.
//!
//! MegaMek spells one piece of equipment many ways (`Medium Laser`, `ISMediumLaser`,
//! `IS Medium Laser`), so names are compared as lowercase alphanumeric keys with an optional
//! technology prefix. MegaMek writes Clan equipment with a `CL` or `Clan` prefix; an
//! unprefixed weapon resolves to the chassis's own technology base when that base has it (a
//! Clan unit's `Light Machine Gun` is the Clan gun), and otherwise to the other base.
use anyhow::{Context, Result, bail, ensure};
use stompymux_unit_construction::{AmmunitionMode, MachineGunClass, System, Weapon};

/// Technology base of a chassis or a piece of equipment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechBase {
    InnerSphere,
    Clan,
}

impl TechBase {
    /// The namespace stompymux weapon and ammunition names carry, as do the systems both
    /// bases build to their own specifications.
    pub fn namespace(self) -> &'static str {
        match self {
            Self::InnerSphere => "IS",
            Self::Clan => "CL",
        }
    }

    /// The `[construction]` `tech_base` spelling.
    pub fn document_spelling(self) -> &'static str {
        match self {
            Self::InnerSphere => "inner_sphere",
            Self::Clan => "clan",
        }
    }

    /// Human-readable name for diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Self::InnerSphere => "Inner Sphere",
            Self::Clan => "Clan",
        }
    }
}

/// Heat sink families a critical slot or header can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatSinkKind {
    Single,
    Double(TechBase),
    Laser,
}

/// What one MegaMek critical slot or equipment line becomes.
#[derive(Debug, Clone, PartialEq)]
pub enum Critical {
    /// An unoccupied slot.
    Empty,
    /// Actuators, engine, gyro and cockpit equipment that mech construction places.
    Fixed(&'static str),
    HeatSink(HeatSinkKind),
    Weapon {
        weapon: Weapon,
        rear: bool,
        one_shot: bool,
    },
    /// One bin holding a ton (or half a ton) of ammunition.
    Ammo {
        weapon: Weapon,
        modes: Vec<&'static str>,
        rounds: u16,
    },
    /// Slot-occupying equipment such as CASE, Endo Steel or TAG.
    System(&'static str),
    /// A system both technology bases build to their own specifications, such as jump jets,
    /// an ECM suite or a machine gun array, with the base that built it: its item always
    /// states that base (`IS.MGA`, `CL.Ecm`).
    BaseSystem(&'static str, TechBase),
    /// A machine gun array of the technology base that built it, for machine guns of one size
    /// class, linked to the guns of that class in its section once the section is known
    /// (TechManual p. 228).
    MachineGunArray(TechBase, MachineGunClass),
    /// An Artemis IV fire-control system of the technology base that built it, linked to a
    /// launcher once its section is known.
    Artemis(TechBase),
    /// One slot of a Clan Artemis V fire-control system, linked like Artemis IV; a BattleMech
    /// installation fills two slots (Tactical Operations: Advanced Units & Equipment p. 217).
    ArtemisV,
    /// An MRM "Apollo" fire-control system, linked to an MRM launcher in its section like
    /// Artemis IV (Tactical Operations: Advanced Units & Equipment p. 142).
    Apollo,
    /// A searchlight, which stompymux records as a chassis special rather than a slot.
    Searchlight,
}

impl Critical {
    /// The stompymux item a slot holding this critical names.
    pub fn item(&self) -> Option<String> {
        Some(match self {
            Self::Empty | Self::Searchlight => return None,
            Self::Fixed(item) | Self::System(item) => (*item).to_owned(),
            Self::HeatSink(_) => "HeatSink".to_owned(),
            Self::Weapon { weapon, .. } => weapon.name().to_owned(),
            Self::Ammo { weapon, .. } => format!("Ammo_{}", weapon.name()),
            Self::BaseSystem(item, tech) => format!("{}.{item}", tech.namespace()),
            Self::MachineGunArray(tech, _) => format!("{}.MGA", tech.namespace()),
            Self::Artemis(tech) => format!("{}.ArtemisIV", tech.namespace()),
            Self::ArtemisV => "ArtemisV".to_owned(),
            Self::Apollo => "ApolloFCS".to_owned(),
        })
    }

    /// Modes stompymux records on this critical's slot.
    pub fn modes(&self) -> Vec<&'static str> {
        match self {
            Self::Weapon { rear, one_shot, .. } => {
                let mut modes = Vec::new();
                if *rear {
                    modes.push("RearMount");
                }
                if *one_shot {
                    modes.push("OneShot");
                }
                modes
            }
            Self::Ammo { modes, .. } => modes.clone(),
            _ => Vec::new(),
        }
    }
}

/// Fixed mech equipment: MegaMek key and stompymux item.
const FIXED: &[(&str, &str)] = &[
    ("shoulder", "ShoulderOrHip"),
    ("hip", "ShoulderOrHip"),
    ("upperarmactuator", "UpperActuator"),
    ("upperlegactuator", "UpperActuator"),
    ("lowerarmactuator", "LowerActuator"),
    ("lowerlegactuator", "LowerActuator"),
    ("handactuator", "HandOrFootActuator"),
    ("footactuator", "HandOrFootActuator"),
    ("lifesupport", "LifeSupport"),
    ("sensors", "Sensors"),
    ("cockpit", "Cockpit"),
    ("gyro", "Gyro"),
];

/// Slot-occupying systems: MegaMek key (without technology prefix) and stompymux item.
const SYSTEMS: &[(&str, &str)] = &[
    ("jumpjet", "JumpJet"),
    ("case", "Case"),
    ("caseii", "CASE-II"),
    ("endosteel", "EndoSteel"),
    ("ferrofibrous", "FerroFibrous"),
    ("lightferrofibrous", "LtFerroFibrous"),
    ("heavyferrofibrous", "HvyFerroFibrous"),
    ("stealth", "StealthArmor"),
    ("stealtharmor", "StealthArmor"),
    ("reflective", "LaserReflective"),
    ("laserreflective", "LaserReflective"),
    ("targetingcomputer", "TargetingComputer"),
    ("guardianecm", "Ecm"),
    ("guardianecmsuite", "Ecm"),
    ("ecmsuite", "Ecm"),
    ("angelecm", "AngelEcm"),
    ("angelecmsuite", "AngelEcm"),
    ("beagleactiveprobe", "BeagleProbe"),
    ("activeprobe", "BeagleProbe"),
    ("lightactiveprobe", "Light_BAP"),
    ("bloodhoundactiveprobe", "BloodhoundProbe"),
    ("tag", "TAG"),
    ("lighttag", "LightTAG"),
    ("c3slave", "C3Slave"),
    ("c3slaveunit", "C3Slave"),
    ("c3computerslave", "C3Slave"),
    ("c3master", "C3Master"),
    ("c3masterunit", "C3Master"),
    ("c3mastercomputer", "C3Master"),
    ("c3computermaster", "C3Master"),
    ("c3i", "C3i"),
    ("c3iunit", "C3i"),
    ("improvedc3computer", "C3i"),
    ("masc", "Masc"),
    ("talons", "Talons"),
    ("spikes", "Spikes"),
    ("tsm", "TripleStrengthMyomer"),
    ("triplestrengthmyomer", "TripleStrengthMyomer"),
    ("hatchet", "Axe"),
    ("sword", "Sword"),
    ("mace", "Mace"),
    ("claw", "Claw"),
    ("claws", "Claw"),
    ("retractableblade", "Retractable_Blade"),
    ("lance", "Lance"),
    ("flail", "Flail"),
    ("wreckingball", "Wrecking_Ball"),
    ("dualsaw", "Dual_Saw"),
    ("chainwhip", "Chain_Whip"),
    ("smallvibroblade", "Small_Vibroblade"),
    ("mediumvibroblade", "Medium_Vibroblade"),
    ("largevibroblade", "Large_Vibroblade"),
    ("nullsignaturesystem", "NullSig_Device"),
    ("meknullsignaturesystem", "NullSig_Device"),
    ("mechnullsignaturesystem", "NullSig_Device"),
];

/// MegaMek weapon keys that differ from the key of the stompymux label they name.
const WEAPON_ALIASES: &[(&str, &str)] = &[
    ("autocannon2", "ac2"),
    ("autocannon5", "ac5"),
    ("autocannon10", "ac10"),
    ("autocannon20", "ac20"),
    ("lbxac2", "lb2xac"),
    ("lbxac5", "lb5xac"),
    ("lbxac10", "lb10xac"),
    ("lbxac20", "lb20xac"),
    ("lb2x", "lb2xac"),
    ("lb5x", "lb5xac"),
    ("lb10x", "lb10xac"),
    ("lb20x", "lb20xac"),
    ("uac2", "ultraac2"),
    ("uac5", "ultraac5"),
    ("uac10", "ultraac10"),
    ("uac20", "ultraac20"),
    ("rac2", "rotaryac2"),
    ("rac5", "rotaryac5"),
    ("lac2", "lightac2"),
    ("lac5", "lightac5"),
    ("lightautocannon2", "lightac2"),
    ("lightautocannon5", "lightac5"),
    ("hvac2", "hyperac2"),
    ("hvac5", "hyperac5"),
    ("hvac10", "hyperac10"),
    ("hypervelocityautocannon2", "hyperac2"),
    ("hypervelocityautocannon5", "hyperac5"),
    ("hypervelocityautocannon10", "hyperac10"),
    ("mg", "machinegun"),
    ("lightmg", "lightmachinegun"),
    ("heavymg", "heavymachinegun"),
    ("ams", "antimissilesystem"),
    ("laserantimissilesystem", "laserams"),
    ("narc", "narcbeacon"),
    ("narcmissilebeacon", "narcbeacon"),
    ("inarc", "inarcbeacon"),
    ("improvednarc", "inarcbeacon"),
    ("inarclauncher", "inarcbeacon"),
    ("arrowiv", "arrowivsystem"),
    ("longtomartillery", "longtom"),
    ("sniperartillery", "sniper"),
    ("thumperartillery", "thumper"),
    ("antipersonnelpod", "apod"),
    ("antibattlearmorpod", "bpod"),
    ("smallxpulselaser", "xsmallpulselaser"),
    ("mediumxpulselaser", "xmediumpulselaser"),
    ("largexpulselaser", "xlargepulselaser"),
    ("snubnoseppc", "snubnosedppc"),
    ("particlecannon", "ppc"),
    ("rocketlauncher10", "rl10"),
    ("rocketlauncher15", "rl15"),
    ("rocketlauncher20", "rl20"),
    ("extendedlrm5", "elrm5"),
    ("extendedlrm10", "elrm10"),
    ("extendedlrm15", "elrm15"),
    ("extendedlrm20", "elrm20"),
    ("enhancedlrm5", "nlrm5"),
    ("enhancedlrm10", "nlrm10"),
    ("enhancedlrm15", "nlrm15"),
    ("enhancedlrm20", "nlrm20"),
    ("tbolt5", "thunderbolt5"),
    ("tbolt10", "thunderbolt10"),
    ("tbolt15", "thunderbolt15"),
    ("tbolt20", "thunderbolt20"),
    ("gauss", "gaussrifle"),
    ("lightgauss", "lightgaussrifle"),
    ("heavygauss", "heavygaussrifle"),
    ("magshot", "magshotgaussrifle"),
    ("magshotgr", "magshotgaussrifle"),
    ("apgauss", "apgaussrifle"),
    ("antipersonnelgaussrifle", "apgaussrifle"),
    ("hyperassaultgauss20", "hag20"),
    ("hyperassaultgauss30", "hag30"),
    ("hyperassaultgauss40", "hag40"),
    ("snppc", "snubnosedppc"),
];

/// Munition phrases MegaMek writes into ammunition names, and the stompymux bin mode each sets.
/// Longer phrases come first so `swarm-i` is not read as `swarm`.
const MUNITIONS: &[(&str, &str, AmmunitionMode)] = &[
    ("artemis v-capable", "Artemis/Mine", AmmunitionMode::Artemis),
    ("artemis-capable", "Artemis/Mine", AmmunitionMode::Artemis),
    ("narc-capable", "Narc/Smoke", AmmunitionMode::Narc),
    ("swarm-i", "Swarm1", AmmunitionMode::Swarm1),
    ("swarm", "Swarm", AmmunitionMode::Swarm),
    ("inferno", "Inferno", AmmunitionMode::Inferno),
    ("precision", "Precision", AmmunitionMode::Precision),
    ("armor-piercing", "AP", AmmunitionMode::ArmorPiercing),
    ("flechette", "Flechette", AmmunitionMode::Flechette),
    ("caseless", "Caseless", AmmunitionMode::Caseless),
    ("semi-guided", "Sguided", AmmunitionMode::SemiGuided),
    ("smoke", "Smoke", AmmunitionMode::Smoke),
];

/// MegaMek munitions stompymux has no equivalent for, checked before [`MUNITIONS`] so that
/// `Thunder-Inferno` is refused rather than read as `Inferno`.
const UNSUPPORTED_MUNITIONS: &[&str] = &[
    "fascam",
    "tandem-charge",
    "tracer",
    "illumination",
    "heat-seeking",
    "listen-kill",
    "anti-tsm",
    "dead-fire",
    "incendiary",
    "follow the leader",
    "fragmentation",
    "mine clearance",
    "acid",
    "coolant",
    "flak",
    "homing",
    "copperhead",
    "davy crockett",
    "anti-radiation",
    "laser inhibiting",
    "magnetic pulse",
    "fuel-air",
    "airburst",
    "chaff",
    "flare",
    "harpoon",
    "air-defense",
    "anti-personnel",
    "explosive",
    "haywire",
    "nemesis",
];

/// Resolve one MegaMek critical slot or equipment line on an Inner Sphere chassis; tests'
/// shorthand for [`parse_for`].
#[cfg(test)]
pub fn parse(raw: &str) -> Result<Critical> {
    parse_for(raw, TechBase::InnerSphere)
}

/// Resolve one MegaMek critical slot or equipment line on a chassis of technology base
/// `chassis`, which unprefixed weapon and ammunition names prefer.
pub fn parse_for(raw: &str, chassis: TechBase) -> Result<Critical> {
    let mut name = raw.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("-Empty-") {
        return Ok(Critical::Empty);
    }
    if let Some(pod) = name.len().checked_sub(5).and_then(|at| name.get(at..))
        && pod.eq_ignore_ascii_case(":omni")
    {
        name = name[..name.len() - 5].trim_end();
    }
    ensure!(
        !name.contains(['|', ':', '#']),
        "unsupported equipment option in {raw}"
    );
    let mut rear = false;
    let mut one_shot = false;
    while let Some(open) = name.rfind('(').filter(|_| name.ends_with(')')) {
        let marker = name[open + 1..name.len() - 1].trim().to_ascii_lowercase();
        match marker.as_str() {
            "omnipod" => {}
            "r" => rear = true,
            "os" => one_shot = true,
            "armored" => bail!("armored components are unsupported: {raw}"),
            "t" => bail!("mech turrets are unsupported: {raw}"),
            "i-os" | "ios" => bail!("improved one-shot launchers are unsupported: {raw}"),
            _ => bail!("unsupported mounting ({marker}) on {raw}"),
        }
        name = name[..open].trim_end();
    }
    let lower = name.to_ascii_lowercase();
    let critical = if lower.contains("ammo") || lower.ends_with("pods") {
        parse_ammo(name, chassis)?
    } else {
        parse_equipment(name, chassis)?
    };
    match critical {
        // A one-shot launcher is marked either by an `(OS)` suffix or in its key, as in
        // `ISStreakSRM2OS`; it weighs half a ton more (TechManual p. 342).
        Critical::Weapon {
            weapon,
            one_shot: spelled,
            ..
        } => Ok(Critical::Weapon {
            weapon,
            rear,
            one_shot: one_shot || spelled || weapon.is_rocket(),
        }),
        _ if rear => bail!("only weapons can be rear-mounted: {raw}"),
        _ if one_shot => bail!("only launchers can be one-shot: {raw}"),
        critical => Ok(critical),
    }
}

/// The technology base a slot's system states, for the systems both bases build to their own
/// specifications under one stompymux name: CASE II, ECM suites, active probes, MASC,
/// targeting computers, Artemis IV, jump jets and machine gun arrays (see
/// `System::built_by_either_base`). `None` when the slot names no such system or does not
/// say which base built it.
pub fn system_tech(raw: &str) -> Option<TechBase> {
    let name = raw.split(['(', ':']).next().unwrap_or_default();
    for (tech, base) in readings(&key(name)) {
        match base {
            "caseii"
            | "ecmsuite"
            | "activeprobe"
            | "masc"
            | "targetingcomputer"
            | "artemisiv"
            | "artemisivfcs"
            | "jumpjet"
            | "mga"
            | "lmga"
            | "hmga"
            | "machinegunarray"
            | "lightmachinegunarray"
            | "heavymachinegunarray"
                if tech.is_some() =>
            {
                return tech;
            }
            "guardianecm" | "guardianecmsuite" | "beagleactiveprobe" => {
                return Some(TechBase::InnerSphere);
            }
            _ => {}
        }
    }
    None
}

/// Resolve a non-ammunition item.
fn parse_equipment(name: &str, chassis: TechBase) -> Result<Critical> {
    let full = key(name);
    if let Some((_, item)) = FIXED.iter().find(|(known, _)| *known == full) {
        return Ok(Critical::Fixed(item));
    }
    if full.ends_with("engine") {
        return Ok(Critical::Fixed("Engine"));
    }
    let readings = readings(&full);
    // A system both bases build is the one its name states, or else the chassis's.
    let built_by = system_tech(name).unwrap_or(chassis);
    if full.contains("heatsink") {
        for (tech, base) in &readings {
            match *base {
                "heatsink" | "singleheatsink" => {
                    return Ok(Critical::HeatSink(HeatSinkKind::Single));
                }
                "doubleheatsink" => {
                    let tech = tech.unwrap_or(TechBase::InnerSphere);
                    return Ok(Critical::HeatSink(HeatSinkKind::Double(tech)));
                }
                "laserheatsink" => return Ok(Critical::HeatSink(HeatSinkKind::Laser)),
                _ => {}
            }
        }
        bail!("unsupported heat sink {name}");
    }
    for (_, base) in &readings {
        match *base {
            "artemisiv" | "artemisivfcs" => return Ok(Critical::Artemis(built_by)),
            "artemisv" | "artemisvfcs" => return Ok(Critical::ArtemisV),
            "apollo" | "mrmapollofcs" | "apollofcs" => return Ok(Critical::Apollo),
            "searchlight" | "mountedsearchlight" => return Ok(Critical::Searchlight),
            "mga" | "machinegunarray" => {
                return Ok(Critical::MachineGunArray(
                    built_by,
                    MachineGunClass::Standard,
                ));
            }
            "lmga" | "lightmachinegunarray" => {
                return Ok(Critical::MachineGunArray(built_by, MachineGunClass::Light));
            }
            "hmga" | "heavymachinegunarray" => {
                return Ok(Critical::MachineGunArray(built_by, MachineGunClass::Heavy));
            }
            _ => {}
        }
        if let Some((_, item)) = SYSTEMS.iter().find(|(known, _)| known == base) {
            if System::named(item).is_some_and(System::built_by_either_base) {
                return Ok(Critical::BaseSystem(item, built_by));
            }
            return Ok(Critical::System(item));
        }
    }
    let (weapon, one_shot) = resolve_weapon(&readings, chassis)
        .with_context(|| format!("unsupported equipment {name}"))?
        .with_context(|| format!("unsupported equipment {name}"))?;
    Ok(Critical::Weapon {
        weapon,
        rear: false,
        one_shot,
    })
}

/// Resolve a ton (or half ton) of ammunition and the bin's munition.
fn parse_ammo(name: &str, chassis: TechBase) -> Result<Critical> {
    let mut lower = name
        .to_ascii_lowercase()
        .replace("(clan)", " ")
        .replace("(is)", " ");
    let thunder = lower
        .match_indices("thunder")
        .any(|(index, _)| !lower[index..].starts_with("thunderbolt"));
    ensure!(
        !thunder
            && !UNSUPPORTED_MUNITIONS
                .iter()
                .any(|munition| lower.contains(munition)),
        "unsupported ammunition {name}"
    );
    let mut half_ton = false;
    for phrase in ["- half", "half"] {
        if let Some(index) = lower.find(phrase) {
            half_ton = true;
            lower.replace_range(index..index + phrase.len(), " ");
        }
    }
    for phrase in ["- full", "full", "- standard", "- slug"] {
        if let Some(index) = lower.find(phrase) {
            lower.replace_range(index..index + phrase.len(), " ");
        }
    }
    let mut munition = None;
    for (phrase, flag, mode) in MUNITIONS {
        if let Some(index) = lower.find(phrase) {
            ensure!(munition.is_none(), "conflicting munitions in {name}");
            munition = Some((*flag, *mode));
            lower.replace_range(index..index + phrase.len(), " ");
        }
    }
    let cluster = lower.contains("cluster");
    lower = lower.replace("cluster", " ");
    let words: Vec<&str> = lower.split_whitespace().collect();
    let last = words.last().copied().unwrap_or_default();
    let atm = lower.contains("atm");
    let mml = lower.contains("mml");
    let atm_mode = match last {
        "er" if atm => Some(("ExtendedRange", AmmunitionMode::ExtendedRange)),
        "he" if atm => Some(("HighExplosive", AmmunitionMode::HighExplosive)),
        _ => None,
    };
    let mml_lrm = mml && last == "lrm";
    let kept = if atm_mode.is_some() || (mml && matches!(last, "lrm" | "srm")) {
        &words[..words.len() - 1]
    } else {
        &words[..]
    };
    let full = key(&kept.join(" "));
    let readings: Vec<(Option<TechBase>, String)> = readings(&full)
        .into_iter()
        .map(|(tech, base)| {
            let base = base.replace("ammo", "");
            let base = base.strip_suffix("pods").unwrap_or(&base).to_owned();
            (tech, base)
        })
        .collect();
    let weapon = first_weapon(
        readings.iter().map(|(tech, base)| (*tech, base.as_str())),
        chassis,
    )
    .with_context(|| format!("unsupported ammunition {name}"))?
    .with_context(|| format!("unsupported ammunition {name}"))?;
    ensure!(
        weapon.profile().ammunition_per_ton > 0,
        "{} takes no ammunition: {name}",
        weapon.name()
    );
    let mut modes = Vec::new();
    let mut mode = AmmunitionMode::Normal;
    if cluster {
        ensure!(munition.is_none(), "conflicting munitions in {name}");
        let flag = if weapon.is_artillery() {
            "Cluster"
        } else {
            "LBX/Cluster"
        };
        munition = Some((flag, AmmunitionMode::Cluster));
    }
    if mml_lrm {
        modes.push("MML_LRM");
        mode = match munition.map(|(_, mode)| mode) {
            None => AmmunitionMode::MmlLrm,
            Some(AmmunitionMode::Artemis) => AmmunitionMode::MmlLrmArtemis,
            Some(AmmunitionMode::Narc) => AmmunitionMode::MmlLrmNarc,
            Some(AmmunitionMode::Swarm) => AmmunitionMode::MmlLrmSwarm,
            Some(AmmunitionMode::Swarm1) => AmmunitionMode::MmlLrmSwarm1,
            Some(AmmunitionMode::SemiGuided) => AmmunitionMode::MmlLrmSemiGuided,
            Some(_) => bail!("unsupported MML munition {name}"),
        };
        modes.extend(munition.map(|(flag, _)| flag));
    } else {
        for (flag, selected) in munition.into_iter().chain(atm_mode) {
            modes.push(flag);
            mode = selected;
        }
    }
    if half_ton {
        modes.push("Halfton");
    }
    let per_ton = u16::from(weapon.profile_for_ammunition(mode).ammunition_per_ton);
    let rounds = if half_ton
        || matches!(
            mode,
            AmmunitionMode::Precision | AmmunitionMode::ArmorPiercing
        ) {
        per_ton / 2
    } else if mode == AmmunitionMode::Caseless {
        per_ton * 2
    } else {
        per_ton
    };
    Ok(Critical::Ammo {
        weapon,
        modes,
        rounds,
    })
}

/// Resolve a weapon from its key readings, returning whether its spelling marked a one-shot
/// launcher (`ISLRM20OS`).
fn resolve_weapon(
    readings: &[(Option<TechBase>, &str)],
    chassis: TechBase,
) -> Result<Option<(Weapon, bool)>> {
    if let Some(weapon) = first_weapon(readings.iter().copied(), chassis)? {
        return Ok(Some((weapon, false)));
    }
    ensure!(
        !readings.iter().any(|(_, base)| base.ends_with("ios")),
        "improved one-shot launchers are unsupported"
    );
    let launchers = readings
        .iter()
        .filter_map(|(tech, base)| Some((*tech, base.strip_suffix("os")?)));
    Ok(first_weapon(launchers, chassis)?.map(|weapon| (weapon, true)))
}

/// The weapon named by the first reading that names one.
///
/// A reading that names a weapon stompymux has only in the other technology base is an error,
/// reported when no other reading resolves.
fn first_weapon<'a>(
    readings: impl Iterator<Item = (Option<TechBase>, &'a str)>,
    chassis: TechBase,
) -> Result<Option<Weapon>> {
    let mut mismatch = None;
    for (tech, base) in readings {
        match pick_weapon(tech, base, chassis) {
            Ok(Some(weapon)) => return Ok(Some(weapon)),
            Ok(None) => {}
            Err(error) => mismatch = mismatch.or(Some(error)),
        }
    }
    mismatch.map_or(Ok(None), Err)
}

/// Choose the stompymux weapon for a technology-free key, preferring the named technology base.
///
/// An unprefixed weapon resolves to the chassis's technology base when it has the weapon, and
/// otherwise to the other base.
fn pick_weapon(tech: Option<TechBase>, base: &str, chassis: TechBase) -> Result<Option<Weapon>> {
    let base = WEAPON_ALIASES
        .iter()
        .find(|(alias, _)| *alias == base)
        .map_or(base, |(_, target)| *target);
    let lookup = |tech: TechBase| {
        Weapon::ALL.iter().copied().find(|weapon| {
            let (namespace, label) = weapon.name().split_once('.').unwrap_or_default();
            namespace == tech.namespace() && key(label) == base
        })
    };
    let Some(tech) = tech else {
        let other = match chassis {
            TechBase::InnerSphere => TechBase::Clan,
            TechBase::Clan => TechBase::InnerSphere,
        };
        return Ok(lookup(chassis).or_else(|| lookup(other)));
    };
    if let Some(weapon) = lookup(tech) {
        return Ok(Some(weapon));
    }
    let other = match tech {
        TechBase::InnerSphere => TechBase::Clan,
        TechBase::Clan => TechBase::InnerSphere,
    };
    if let Some(weapon) = lookup(other) {
        bail!(
            "stompymux has only the {} version ({})",
            other.label(),
            weapon.name()
        );
    }
    Ok(None)
}

/// Every way to read a key: after each technology prefix it starts with, then unprefixed.
/// `CLAntiMissileSystem` reads as Clan `antimissilesystem`, not as Clan `timissilesystem`.
fn readings(full: &str) -> Vec<(Option<TechBase>, &str)> {
    let mut readings: Vec<_> = [
        ("clan", TechBase::Clan),
        ("cl", TechBase::Clan),
        ("is", TechBase::InnerSphere),
    ]
    .into_iter()
    .filter_map(|(prefix, tech)| {
        let rest = full.strip_prefix(prefix).filter(|rest| !rest.is_empty())?;
        Some((Some(tech), rest))
    })
    .collect();
    readings.push((None, full));
    readings
}

/// Lowercase alphanumeric comparison key.
fn key(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resolve a name that must be a weapon.
    fn weapon(name: &str) -> (Weapon, bool, bool) {
        match parse(name).unwrap() {
            Critical::Weapon {
                weapon,
                rear,
                one_shot,
            } => (weapon, rear, one_shot),
            other => panic!("{name} resolved to {other:?}"),
        }
    }

    /// Resolve a name that must be ammunition, returning its slot item, modes and rounds.
    fn ammo(name: &str) -> (String, Vec<&'static str>, u16) {
        let critical = parse(name).unwrap();
        let item = critical.item().unwrap();
        match critical {
            Critical::Ammo { modes, rounds, .. } => (item, modes, rounds),
            other => panic!("{name} resolved to {other:?}"),
        }
    }

    #[test]
    fn weapon_spellings_resolve_by_technology_base() {
        for (name, expected) in [
            ("Medium Laser", "IS.MediumLaser"),
            ("ISERMediumLaser", "IS.ERMediumLaser"),
            ("CLERMediumLaser", "CL.ERMediumLaser"),
            ("Clan ER Medium Laser", "CL.ERMediumLaser"),
            ("Autocannon/20", "IS.AC/20"),
            ("LRM 20", "IS.LRM-20"),
            ("CLLRM20", "CL.LRM-20"),
            ("ISLBXAC10", "IS.LB10-XAC"),
            ("LB 10-X AC", "IS.LB10-XAC"),
            ("ISUltraAC5", "IS.UltraAC/5"),
            ("ISRotaryAC5", "IS.RotaryAC/5"),
            ("ISGaussRifle", "IS.GaussRifle"),
            ("ISMediumXPulseLaser", "IS.X-MediumPulseLaser"),
            ("ISAntiMissileSystem", "IS.Anti-MissileSystem"),
            ("CLATM12", "CL.ATM-12"),
            ("ATM 12", "CL.ATM-12"),
            ("Machine Gun", "IS.MachineGun"),
            ("CLStreakSRM6", "CL.StreakSRM-6"),
        ] {
            assert_eq!(weapon(name).0.name(), expected, "{name}");
        }
        assert_eq!(
            weapon("Medium Laser (R)"),
            (Weapon::parse("IS.MediumLaser").unwrap(), true, false)
        );
        assert!(
            weapon("CLERMediumLaser (omnipod)")
                .0
                .name()
                .starts_with("CL.")
        );
        assert!(weapon("LRM 20 (OS)").2);
        // The key spelling marks a one-shot launcher too.
        assert!(weapon("ISStreakSRM2OS").2);
        assert!(weapon("Rocket Launcher 10").2);
        for (name, expected) in [
            ("CLAPGaussRifle", "CL.APGaussRifle"),
            ("CLHAG20", "CL.HAG/20"),
            ("Clan HAG 40", "CL.HAG/40"),
            ("CLPlasmaCannon", "CL.PlasmaCannon"),
            ("CLLightMG", "CL.LightMachineGun"),
            ("ISLightMG", "IS.LightMachineGun"),
            ("ISBPod", "IS.B-Pod"),
            ("CLBPod", "CL.B-Pod"),
            ("Anti-BattleArmor Pod", "IS.B-Pod"),
        ] {
            assert_eq!(weapon(name).0.name(), expected, "{name}");
        }
    }

    /// An unprefixed name resolves to the chassis's technology base when it has the weapon
    /// (TechManual p. 341 and p. 343 list both light machine guns).
    #[test]
    fn unprefixed_weapons_follow_the_chassis_technology_base() {
        for (chassis, expected) in [
            (TechBase::InnerSphere, "IS.LightMachineGun"),
            (TechBase::Clan, "CL.LightMachineGun"),
        ] {
            let Critical::Weapon { weapon, .. } = parse_for("Light Machine Gun", chassis).unwrap()
            else {
                panic!("a light machine gun is a weapon");
            };
            assert_eq!(weapon.name(), expected);
        }
        let Critical::Weapon { weapon, .. } = parse_for("Medium Laser", TechBase::Clan).unwrap()
        else {
            panic!("a medium laser is a weapon");
        };
        assert_eq!(weapon.name(), "IS.MediumLaser");
    }

    #[test]
    fn ammunition_spellings_resolve_rounds_and_modes() {
        assert_eq!(ammo("IS Ammo AC/20"), ("Ammo_IS.AC/20".into(), vec![], 5));
        assert_eq!(ammo("IS Ammo LRM-20"), ("Ammo_IS.LRM-20".into(), vec![], 6));
        assert_eq!(
            ammo("IS Machine Gun Ammo - Half"),
            ("Ammo_IS.MachineGun".into(), vec!["Halfton"], 100)
        );
        assert_eq!(ammo("IS Ammo MG - Full").2, 200);
        assert_eq!(ammo("ISAMS Ammo").0, "Ammo_IS.Anti-MissileSystem");
        assert_eq!(ammo("Clan Gauss Ammo").0, "Ammo_CL.GaussRifle");
        assert_eq!(ammo("IS Ultra AC/10 Ammo").0, "Ammo_IS.UltraAC/10");
        assert_eq!(ammo("IS Streak SRM 4 Ammo").0, "Ammo_IS.StreakSRM-4");
        assert_eq!(
            ammo("IS LB 10-X Cluster Ammo"),
            ("Ammo_IS.LB10-XAC".into(), vec!["LBX/Cluster"], 10)
        );
        assert_eq!(
            ammo("IS Ammo LRM-20 Artemis-capable").1,
            vec!["Artemis/Mine"]
        );
        assert_eq!(ammo("Clan Ammo ATM-12 ER").1, vec!["ExtendedRange"]);
        assert_eq!(ammo("IS Ammo MML-5 LRM").1, vec!["MML_LRM"]);
        assert!(ammo("IS Ammo MML-5 SRM").1.is_empty());
        assert_eq!(ammo("ISNarc Pods").0, "Ammo_IS.NarcBeacon");
        assert_eq!(
            ammo("Clan Ammo LRM-15 (Clan) Artemis V-capable"),
            ("Ammo_CL.LRM-15".into(), vec!["Artemis/Mine"], 8)
        );
    }

    #[test]
    fn construction_equipment_resolves_to_slot_items() {
        for (name, expected) in [
            ("Shoulder", Critical::Fixed("ShoulderOrHip")),
            ("Foot Actuator", Critical::Fixed("HandOrFootActuator")),
            ("Fusion Engine", Critical::Fixed("Engine")),
            ("Gyro", Critical::Fixed("Gyro")),
            ("-Empty-", Critical::Empty),
            ("Heat Sink", Critical::HeatSink(HeatSinkKind::Single)),
            (
                "ISDoubleHeatSink",
                Critical::HeatSink(HeatSinkKind::Double(TechBase::InnerSphere)),
            ),
            (
                "CLDoubleHeatSink",
                Critical::HeatSink(HeatSinkKind::Double(TechBase::Clan)),
            ),
            (
                "Jump Jet",
                Critical::BaseSystem("JumpJet", TechBase::InnerSphere),
            ),
            ("ISCASE", Critical::System("Case")),
            ("Claw", Critical::System("Claw")),
            ("IS Ferro-Fibrous", Critical::System("FerroFibrous")),
            ("Clan Endo Steel", Critical::System("EndoSteel")),
            (
                "ISGuardianECMSuite",
                Critical::BaseSystem("Ecm", TechBase::InnerSphere),
            ),
            ("ISArtemisIV", Critical::Artemis(TechBase::InnerSphere)),
            ("ISApollo (OMNIPOD)", Critical::Apollo),
            ("Talons (OMNIPOD)", Critical::System("Talons")),
            ("Spikes", Critical::System("Spikes")),
            (
                "ISLMGA",
                Critical::MachineGunArray(TechBase::InnerSphere, MachineGunClass::Light),
            ),
            (
                "Clan Heavy Machine Gun Array (OMNIPOD)",
                Critical::MachineGunArray(TechBase::Clan, MachineGunClass::Heavy),
            ),
            (
                "ISMGA",
                Critical::MachineGunArray(TechBase::InnerSphere, MachineGunClass::Standard),
            ),
            ("CLArtemisV", Critical::ArtemisV),
            ("CLLightTAG", Critical::System("LightTAG")),
        ] {
            assert_eq!(parse(name).unwrap(), expected, "{name}");
        }
    }

    /// Every system both bases build names the base that built it: the one its MegaMek name
    /// states, or else the chassis's. Systems only one base builds stay unprefixed.
    #[test]
    fn systems_both_bases_build_state_their_base() {
        let item = |name: &str, chassis| parse_for(name, chassis).unwrap().item().unwrap();
        for (name, chassis, expected) in [
            ("MGA", TechBase::InnerSphere, "IS.MGA"),
            ("MGA", TechBase::Clan, "CL.MGA"),
            ("ISMGA", TechBase::Clan, "IS.MGA"),
            ("CLLMGA", TechBase::InnerSphere, "CL.MGA"),
            ("Jump Jet", TechBase::Clan, "CL.JumpJet"),
            ("CLECMSuite", TechBase::Clan, "CL.Ecm"),
            ("GuardianECM", TechBase::Clan, "IS.Ecm"),
            ("BeagleActiveProbe", TechBase::Clan, "IS.BeagleProbe"),
            ("CLActiveProbe", TechBase::Clan, "CL.BeagleProbe"),
            ("CLMASC", TechBase::Clan, "CL.Masc"),
            (
                "ISTargeting Computer",
                TechBase::InnerSphere,
                "IS.TargetingComputer",
            ),
            ("CASE II", TechBase::Clan, "CL.CASE-II"),
            ("ArtemisIV", TechBase::Clan, "CL.ArtemisIV"),
            ("ISCASE", TechBase::InnerSphere, "Case"),
            ("CLLightTAG", TechBase::Clan, "LightTAG"),
        ] {
            assert_eq!(item(name, chassis), expected, "{name} on {chassis:?}");
        }
        // Each MegaMek key for such a system reads its stated base; the Guardian ECM suite and
        // Beagle probe are Inner Sphere names whatever the prefix.
        for (key, stock) in SYSTEMS {
            if !System::named(stock).is_some_and(System::built_by_either_base) {
                continue;
            }
            let named_is = key.starts_with("guardian") || key.starts_with("beagle");
            let expected = if named_is { "IS" } else { "CL" };
            assert_eq!(
                item(&format!("CL{key}"), TechBase::InnerSphere),
                format!("{expected}.{stock}"),
                "{key}"
            );
        }
    }

    #[test]
    fn unsupported_equipment_is_refused() {
        for name in [
            "ISImprovedHeavyGaussRifle",
            "CLHeavyFlamer",
            "IS Ammo LRM-20 Thunder",
            "CLImpGaussAmmo",
            "Chameleon Light Polarization Shield",
            "Shoulder (armored)",
            "ISStreakSRM6 (IOS)",
            "Medium Laser (T)",
            "ISCASE (R)",
            "IS Ammo AC/20:Shots5#",
            "IS1 Compact Heat Sink",
            "Improved Jump Jet",
        ] {
            assert!(parse(name).is_err(), "{name}");
        }
    }
}
