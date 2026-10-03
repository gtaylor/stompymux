//! Resolves MegaMek equipment names, as written in `.mtf` critical slots and `.blk` equipment
//! blocks, to stompymux template items. Anything stompymux has no item for is an error rather
//! than a silent omission.
//!
//! MegaMek spells one piece of equipment many ways (`Medium Laser`, `ISMediumLaser`,
//! `IS Medium Laser`), so names are compared as lowercase alphanumeric keys with an optional
//! technology prefix. MegaMek writes Clan equipment with a `CL` or `Clan` prefix; unprefixed
//! weapons are its Inner Sphere (or shared) versions.
use anyhow::{Context, Result, bail, ensure};
use stompymux_rs::{BattleAmmunitionMode, BattleWeapon};

/// Technology base of a chassis or a piece of equipment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechBase {
    InnerSphere,
    Clan,
}

impl TechBase {
    /// The namespace stompymux weapon and ammunition names carry.
    fn namespace(self) -> &'static str {
        match self {
            Self::InnerSphere => "IS",
            Self::Clan => "CL",
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
        weapon: BattleWeapon,
        rear: bool,
        one_shot: bool,
    },
    /// One bin holding a ton (or half a ton) of ammunition.
    Ammo {
        weapon: BattleWeapon,
        modes: Vec<&'static str>,
        rounds: u16,
    },
    /// Slot-occupying equipment such as jump jets, CASE or an ECM suite.
    System(&'static str),
    /// An Artemis IV fire-control system, linked to a launcher once its section is known.
    Artemis,
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
            Self::Artemis => "ArtemisIV".to_owned(),
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
    ("rac10", "rotaryac10"),
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
    ("snppc", "snubnosedppc"),
];

/// Munition phrases MegaMek writes into ammunition names, and the stompymux bin mode each sets.
/// Longer phrases come first so `swarm-i` is not read as `swarm`.
const MUNITIONS: &[(&str, &str, BattleAmmunitionMode)] = &[
    (
        "artemis-capable",
        "Artemis/Mine",
        BattleAmmunitionMode::Artemis,
    ),
    ("narc-capable", "Narc/Smoke", BattleAmmunitionMode::Narc),
    ("swarm-i", "Swarm1", BattleAmmunitionMode::Swarm1),
    ("swarm", "Swarm", BattleAmmunitionMode::Swarm),
    ("inferno", "Inferno", BattleAmmunitionMode::Inferno),
    ("precision", "Precision", BattleAmmunitionMode::Precision),
    ("armor-piercing", "AP", BattleAmmunitionMode::ArmorPiercing),
    ("flechette", "Flechette", BattleAmmunitionMode::Flechette),
    ("incendiary", "Incendiary", BattleAmmunitionMode::Incendiary),
    ("caseless", "Caseless", BattleAmmunitionMode::Caseless),
    ("semi-guided", "Sguided", BattleAmmunitionMode::SemiGuided),
    ("smoke", "Smoke", BattleAmmunitionMode::Smoke),
];

/// MegaMek munitions stompymux has no equivalent for, checked before [`MUNITIONS`] so that
/// `Thunder-Inferno` is refused rather than read as `Inferno`.
const UNSUPPORTED_MUNITIONS: &[&str] = &[
    "artemis v-capable",
    "fascam",
    "tandem-charge",
    "tracer",
    "illumination",
    "heat-seeking",
    "listen-kill",
    "anti-tsm",
    "dead-fire",
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

/// Resolve one MegaMek critical slot or equipment line.
pub fn parse(raw: &str) -> Result<Critical> {
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
        parse_ammo(name)?
    } else {
        parse_equipment(name)?
    };
    match critical {
        Critical::Weapon { weapon, .. } => Ok(Critical::Weapon {
            weapon,
            rear,
            one_shot: one_shot || weapon.is_rocket(),
        }),
        _ if rear => bail!("only weapons can be rear-mounted: {raw}"),
        _ if one_shot => bail!("only launchers can be one-shot: {raw}"),
        critical => Ok(critical),
    }
}

/// The technology base a slot's system implies, for the systems whose Inner Sphere and Clan
/// versions differ in mass or reach while stompymux reads their technology from the chassis:
/// CASE II, ECM suites and active probes. `None` when the slot names no such system or does
/// not say which base built it.
pub fn system_tech(raw: &str) -> Option<TechBase> {
    let name = raw.split(['(', ':']).next().unwrap_or_default();
    for (tech, base) in readings(&key(name)) {
        match base {
            "caseii" | "ecmsuite" | "activeprobe" if tech.is_some() => return tech,
            "guardianecm" | "guardianecmsuite" | "beagleactiveprobe" => {
                return Some(TechBase::InnerSphere);
            }
            _ => {}
        }
    }
    None
}

/// Resolve a non-ammunition item.
fn parse_equipment(name: &str) -> Result<Critical> {
    let full = key(name);
    if let Some((_, item)) = FIXED.iter().find(|(known, _)| *known == full) {
        return Ok(Critical::Fixed(item));
    }
    if full.ends_with("engine") {
        return Ok(Critical::Fixed("Engine"));
    }
    let readings = readings(&full);
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
            "artemisiv" | "artemisivfcs" => return Ok(Critical::Artemis),
            "searchlight" | "mountedsearchlight" => return Ok(Critical::Searchlight),
            _ => {}
        }
        if let Some((_, item)) = SYSTEMS.iter().find(|(known, _)| known == base) {
            return Ok(Critical::System(item));
        }
    }
    let (weapon, one_shot) = resolve_weapon(&readings)
        .with_context(|| format!("unsupported equipment {name}"))?
        .with_context(|| format!("unsupported equipment {name}"))?;
    Ok(Critical::Weapon {
        weapon,
        rear: false,
        one_shot,
    })
}

/// Resolve a ton (or half ton) of ammunition and the bin's munition.
fn parse_ammo(name: &str) -> Result<Critical> {
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
        "er" if atm => Some(("ExtendedRange", BattleAmmunitionMode::ExtendedRange)),
        "he" if atm => Some(("HighExplosive", BattleAmmunitionMode::HighExplosive)),
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
    let weapon = first_weapon(readings.iter().map(|(tech, base)| (*tech, base.as_str())))
        .with_context(|| format!("unsupported ammunition {name}"))?
        .with_context(|| format!("unsupported ammunition {name}"))?;
    ensure!(
        weapon.profile().ammunition_per_ton > 0,
        "{} takes no ammunition: {name}",
        weapon.name()
    );
    let mut modes = Vec::new();
    let mut mode = BattleAmmunitionMode::Normal;
    if cluster {
        ensure!(munition.is_none(), "conflicting munitions in {name}");
        let flag = if weapon.is_artillery() {
            "Cluster"
        } else {
            "LBX/Cluster"
        };
        munition = Some((flag, BattleAmmunitionMode::Cluster));
    }
    if mml_lrm {
        modes.push("MML_LRM");
        mode = match munition.map(|(_, mode)| mode) {
            None => BattleAmmunitionMode::MmlLrm,
            Some(BattleAmmunitionMode::Artemis) => BattleAmmunitionMode::MmlLrmArtemis,
            Some(BattleAmmunitionMode::Narc) => BattleAmmunitionMode::MmlLrmNarc,
            Some(BattleAmmunitionMode::Swarm) => BattleAmmunitionMode::MmlLrmSwarm,
            Some(BattleAmmunitionMode::Swarm1) => BattleAmmunitionMode::MmlLrmSwarm1,
            Some(BattleAmmunitionMode::SemiGuided) => BattleAmmunitionMode::MmlLrmSemiGuided,
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
            BattleAmmunitionMode::Precision | BattleAmmunitionMode::ArmorPiercing
        ) {
        per_ton / 2
    } else if mode == BattleAmmunitionMode::Caseless {
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
fn resolve_weapon(readings: &[(Option<TechBase>, &str)]) -> Result<Option<(BattleWeapon, bool)>> {
    if let Some(weapon) = first_weapon(readings.iter().copied())? {
        return Ok(Some((weapon, false)));
    }
    ensure!(
        !readings.iter().any(|(_, base)| base.ends_with("ios")),
        "improved one-shot launchers are unsupported"
    );
    let launchers = readings
        .iter()
        .filter_map(|(tech, base)| Some((*tech, base.strip_suffix("os")?)));
    Ok(first_weapon(launchers)?.map(|weapon| (weapon, true)))
}

/// The weapon named by the first reading that names one.
///
/// A reading that names a weapon stompymux has only in the other technology base is an error,
/// reported when no other reading resolves.
fn first_weapon<'a>(
    readings: impl Iterator<Item = (Option<TechBase>, &'a str)>,
) -> Result<Option<BattleWeapon>> {
    let mut mismatch = None;
    for (tech, base) in readings {
        match pick_weapon(tech, base) {
            Ok(Some(weapon)) => return Ok(Some(weapon)),
            Ok(None) => {}
            Err(error) => mismatch = mismatch.or(Some(error)),
        }
    }
    mismatch.map_or(Ok(None), Err)
}

/// Choose the stompymux weapon for a technology-free key, preferring the named technology base.
///
/// Unprefixed MegaMek weapons are its Inner Sphere versions; a Clan-only weapon written without
/// a prefix still resolves to the Clan one.
fn pick_weapon(tech: Option<TechBase>, base: &str) -> Result<Option<BattleWeapon>> {
    let base = WEAPON_ALIASES
        .iter()
        .find(|(alias, _)| *alias == base)
        .map_or(base, |(_, target)| *target);
    let lookup = |tech: TechBase| {
        BattleWeapon::ALL.iter().copied().find(|weapon| {
            let (namespace, label) = weapon.name().split_once('.').unwrap_or_default();
            namespace == tech.namespace() && key(label) == base
        })
    };
    let Some(tech) = tech else {
        return Ok(lookup(TechBase::InnerSphere).or_else(|| lookup(TechBase::Clan)));
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
    fn weapon(name: &str) -> (BattleWeapon, bool, bool) {
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
            (BattleWeapon::parse("IS.MediumLaser").unwrap(), true, false)
        );
        assert!(
            weapon("CLERMediumLaser (omnipod)")
                .0
                .name()
                .starts_with("CL.")
        );
        assert!(weapon("LRM 20 (OS)").2);
        assert!(weapon("Rocket Launcher 10").2);
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
            ("Jump Jet", Critical::System("JumpJet")),
            ("ISCASE", Critical::System("Case")),
            ("Claw", Critical::System("Claw")),
            ("IS Ferro-Fibrous", Critical::System("FerroFibrous")),
            ("Clan Endo Steel", Critical::System("EndoSteel")),
            ("ISGuardianECMSuite", Critical::System("Ecm")),
            ("ISArtemisIV", Critical::Artemis),
        ] {
            assert_eq!(parse(name).unwrap(), expected, "{name}");
        }
    }

    #[test]
    fn unsupported_equipment_is_refused() {
        for name in [
            "ISImprovedHeavyGaussRifle",
            "CLHeavyFlamer",
            "Clan Ammo LRM-15 (Clan) Artemis V-capable",
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
