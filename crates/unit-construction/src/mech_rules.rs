//! TechManual BattleMech construction rules applied to a template and its resolved loadout:
//! movement and engine rating, jump jets, heat sinks, structure and armor technology slots,
//! maximum armor, myomer and boosters, tonnage-dependent equipment, mounting restrictions,
//! technology base and total mass. Template checks, the template loader and the live
//! revalidation of a constructed Mech all run these rules, so a Mech that passes is one a
//! record sheet could describe.
use super::construction::{Component, MIXED_TECH, SPEED_PER_MP};
use super::document::ENGINE_RATING;
use super::engine::declared_engine_rating;
use super::tech_base::{ChassisTechnology, TechBase, TechnologyInventory, check_technology_base};
use super::{
    Engine, MechChassis, MechLoadout, MechSection, MechTemplate, System, rated_output,
    running_mp_of_walking,
};
use anyhow::{Result, bail, ensure};
use std::collections::BTreeMap;

/// Largest engine rating: 400 for the TechManual Master Engine Table, extended to 500 by the
/// large engines of Tactical Operations: Advanced Units & Equipment.
pub const MAXIMUM_ENGINE_RATING: u32 = 500;

/// Most armor a Mech head may carry (TM p. 54).
pub const MAXIMUM_HEAD_ARMOR: u16 = 9;

/// Whole movement points of a stored speed, refusing speeds that are not whole MP.
fn whole_movement_points(speed: f64, label: &str) -> Result<u32> {
    let points = (speed / SPEED_PER_MP).round();
    ensure!(
        points >= 0.0 && (points * SPEED_PER_MP - speed).abs() < 1e-6,
        "{label} of {speed} km/h is not a whole number of movement points"
    );
    Ok(points as u32)
}

/// Whether every slot of a run lies in one section in consecutive positions.
fn contiguous(slots: &[super::CriticalLocation]) -> bool {
    slots
        .windows(2)
        .all(|pair| pair[0].section == pair[1].section && pair[0].slot + 1 == pair[1].slot)
}

/// Physical weapons: critical slots one item fills and the actuators its arm needs (TM pp. 220,
/// 237, 243, 249; Tactical Operations: Advanced Units & Equipment melee weapons).
struct PhysicalWeapon {
    slots: usize,
    /// The arm needs shoulder, upper and lower arm actuators.
    full_arm: bool,
    /// The arm needs its hand actuator.
    hand: bool,
    /// The weapon replaces the hand actuator, which the arm may not keep.
    replaces_hand: bool,
}

/// Construction facts of a physical weapon system, or `None` for other systems.
fn physical_weapon(system: System, tons: u16) -> Option<PhysicalWeapon> {
    let per = |divisor: u16| usize::from(tons.div_ceil(divisor));
    let (slots, full_arm, hand, replaces_hand) = match system {
        // Hatchet: tonnage / 15 rounded up, full arm and hand actuators (TM p. 220).
        System::Axe => (per(15), true, true, false),
        // Sword: tonnage / 15 rounded up slots, full arm and hand actuators (TM p. 237).
        System::Sword => (per(15), true, true, false),
        // Vibroblades follow the sword mounting rules in three fixed sizes.
        System::SmallVibroblade => (1, true, true, false),
        System::MediumVibroblade => (2, true, true, false),
        System::LargeVibroblade => (4, true, true, false),
        // Mace: tonnage / 10 rounded up, in an arm with a hand actuator.
        System::Mace => (per(10), true, true, false),
        // Claws: tonnage / 15 rounded up, replacing the hand; upper and lower actuators stay.
        System::Claw => (per(15), true, false, true),
        // Lance: tonnage / 20 rounded up, shoulder, upper and lower arm actuators.
        System::Lance => (per(20), true, false, false),
        // Flail: four slots, replacing the hand; upper and lower actuators stay.
        System::Flail => (4, true, false, true),
        // Chain whip: two slots in an arm.
        System::ChainWhip => (2, false, false, false),
        // Retractable blade: one slot plus one per twenty tons, in an arm with shoulder, upper
        // and lower arm actuators; it needs no hand actuator (TM p. 237).
        System::RetractableBlade => (per(20) + 1, true, false, false),
        // Dual saw: seven slots in an arm whose hand actuator it replaces (TM p. 243).
        System::DualSaw => (7, false, false, true),
        // Wrecking ball: five slots in an arm whose hand actuator it replaces (TM p. 249).
        System::WreckingBall => (5, false, false, true),
        // Backhoe: six slots in an arm whose hand actuator it replaces (TM pp. 241, 344).
        System::Backhoe => (6, false, false, true),
        _ => return None,
    };
    Some(PhysicalWeapon {
        slots,
        full_arm,
        hand,
        replaces_hand,
    })
}

/// Every limb carries its shoulder or hip and upper actuator, and every leg its lower and
/// foot actuators too. Only a biped arm may leave out its lower arm and hand actuators, and
/// one without its lower arm actuator has no hand actuator either (TechManual p. 45).
fn validate_actuators(loadout: &MechLoadout, chassis: MechChassis) -> Result<()> {
    for section in [
        MechSection::LeftArm,
        MechSection::RightArm,
        MechSection::LeftLeg,
        MechSection::RightLeg,
    ] {
        let has = |system| {
            loadout
                .systems
                .iter()
                .any(|part| part.system == system && part.location.section == section)
        };
        let name = chassis.section_name(section);
        let arm = !chassis.is_leg(section);
        let (lower, hand) = (has(System::LowerActuator), has(System::HandOrFootActuator));
        ensure!(
            has(System::ShoulderOrHip) && has(System::UpperActuator),
            "{name} lacks its {} actuators; only lower arm and hand actuators may be removed (TechManual p. 45)",
            if arm {
                "shoulder and upper arm"
            } else {
                "hip and upper leg"
            }
        );
        ensure!(
            arm || (lower && hand),
            "{name} lacks its lower leg or foot actuator; only lower arm and hand actuators may be removed (TechManual p. 45)"
        );
        ensure!(
            lower || !hand,
            "{name} keeps its hand actuator without a lower arm actuator (TechManual p. 45)"
        );
    }
    Ok(())
}

/// Talons fill two slots in every leg and nowhere else; a location holds at most one spike
/// cluster (Tactical Operations: Advanced Units & Equipment p. 100).
fn validate_talons_and_spikes(loadout: &MechLoadout, chassis: MechChassis) -> Result<()> {
    let count = |system, section| {
        loadout
            .systems
            .iter()
            .filter(|part| part.system == system && part.location.section == section)
            .count()
    };
    if loadout
        .systems
        .iter()
        .any(|part| part.system == System::Talons)
    {
        for section in MechSection::ALL {
            let expected = if chassis.is_leg(section) { 2 } else { 0 };
            ensure!(
                count(System::Talons, section) == expected,
                "Talons fill two slots in every leg and nowhere else; {} has {}",
                chassis.section_name(section),
                count(System::Talons, section)
            );
        }
    }
    for section in MechSection::ALL {
        ensure!(
            count(System::Spikes, section) <= 1,
            "{} mounts more than one spike cluster",
            chassis.section_name(section)
        );
    }
    Ok(())
}

/// Each machine gun array links two to four machine guns of one size class in its location,
/// and a gun belongs to one array at most (TechManual p. 228).
fn validate_machine_gun_arrays(loadout: &MechLoadout, chassis: MechChassis) -> Result<()> {
    super::machine_gun_array::check_machine_gun_arrays(loadout, |section| {
        chassis.section_name(section)
    })
}

impl MechTemplate {
    /// Running MP: the walking MP x 1.5, rounded up (TM p. 48).
    pub fn running_mp(&self) -> u32 {
        running_mp_of_walking(self.walk_mp)
    }

    /// Walking MP the template states (TM p. 48).
    pub fn walking_mp(&self) -> u32 {
        self.walk_mp
    }

    /// State a new walking MP, with the running speed that follows from it. A declared engine
    /// rating follows the new walking MP.
    pub fn set_walking_mp(&mut self, walking_mp: u32) {
        self.walk_mp = walking_mp;
        self.max_speed = f64::from(self.running_mp()) * SPEED_PER_MP;
        self.restate_engine_rating();
    }

    /// Engine rating: tonnage x walking MP (TM p. 48).
    pub fn engine_rating(&self) -> u32 {
        rated_output(self.tons, self.walk_mp)
    }

    /// The engine rating the template's `[construction]` table declares, if any.
    pub fn declared_engine_rating(&self) -> Result<Option<u32>> {
        declared_engine_rating(&self.attributes)
    }

    /// Bring a declared engine rating back in line with the tonnage and walking MP after an
    /// edit to either; a template that declares none keeps deriving it.
    pub fn restate_engine_rating(&mut self) {
        let rating = self.engine_rating();
        if let Some(declared) = self.attributes.get_mut(ENGINE_RATING) {
            *declared = rating.to_string();
        }
    }

    /// Jumping MP declared by the template.
    pub fn jumping_mp(&self) -> Result<u32> {
        whole_movement_points(self.jump_speed, "Jumping speed")
    }

    /// Whether the installed `system` is Clan technology: the base its slots' `IS.` or `CL.`
    /// prefix states, or the chassis's base when they state none.
    pub fn installation_is_clan(&self, system: System) -> bool {
        let declared = self
            .sections
            .values()
            .flat_map(|section| section.criticals.values())
            .filter_map(|critical| System::installation(&critical.equipment))
            .find(|(installed, _)| *installed == system)
            .and_then(|(_, base)| base);
        declared.map_or(self.has_special("Clan"), |base| base == TechBase::Clan)
    }

    /// Critical slots, and tons, of MASC: 5% of the tonnage for Inner Sphere MASC or 4% for
    /// Clan MASC, rounded to the nearest whole number (TM p. 52).
    pub fn masc_slots(&self) -> usize {
        let percent = if self.installation_is_clan(System::Masc) {
            4
        } else {
            5
        };
        usize::from((self.tons * percent + 50) / 100).max(1)
    }

    /// State a standard cargo bay of `capacity` (1/1024-ton units, in half-ton steps) and fill
    /// one free critical slot per ton of it, rounded up, with `Cargo` entries (TM p. 239),
    /// replacing the cargo slots already placed. Free slots are taken in section order. Fails,
    /// leaving the template unchanged, when the Mech has too few free slots.
    pub fn install_cargo_bay(&mut self, capacity: u32) -> Result<()> {
        ensure!(
            capacity.is_multiple_of(512),
            "A cargo bay is built in half-ton steps (TM p. 239)"
        );
        let chassis = self.chassis()?;
        let mut sections = self.sections.clone();
        for layout in sections.values_mut() {
            layout
                .criticals
                .retain(|_, critical| System::named(&critical.equipment) != Some(System::Cargo));
        }
        let mut needed = capacity.div_ceil(1024);
        for (&section, layout) in sections.iter_mut() {
            for slot in 0..chassis.critical_slots(section) {
                if needed == 0 {
                    break;
                }
                if layout.criticals.contains_key(&slot) {
                    continue;
                }
                layout.criticals.insert(
                    slot,
                    super::CriticalDefinition {
                        equipment: "Cargo".into(),
                        data: "-".into(),
                        modes: Vec::new(),
                    },
                );
                needed -= 1;
            }
        }
        ensure!(
            needed == 0,
            "A cargo bay of {} tons needs {needed} more free critical slots (TM p. 239)",
            super::format_tons(capacity)
        );
        self.sections = sections;
        if capacity == 0 {
            self.attributes.remove(super::CARGO_TONS);
        } else {
            self.attributes.insert(
                super::CARGO_TONS.into(),
                (f64::from(capacity / 512) / 2.0).to_string(),
            );
        }
        Ok(())
    }

    /// Check the TechManual construction rules a template and its resolved loadout must
    /// satisfy beyond the section layout [`MechTemplate::validate_loadout`] checks.
    pub fn validate_construction(&self, loadout: &MechLoadout) -> Result<()> {
        let chassis = self.chassis()?;
        self.validate_movement()?;
        self.validate_engine(loadout)?;
        validate_actuators(loadout, chassis)?;
        self.validate_jump_jets(loadout, chassis)?;
        self.validate_heat_sinks(loadout)?;
        self.validate_technology_slots(loadout, chassis)?;
        self.validate_maximum_armor()?;
        self.validate_myomer_and_boosters(loadout)?;
        self.validate_targeting_computer(loadout)?;
        self.validate_physical_weapons(loadout, chassis)?;
        validate_talons_and_spikes(loadout, chassis)?;
        validate_machine_gun_arrays(loadout, chassis)?;
        self.validate_installations(loadout, chassis)?;
        self.validate_cargo_bay(loadout)?;
        self.validate_weapon_mounting(loadout, chassis)?;
        self.validate_technology_base(loadout)?;
        self.validate_total_mass(loadout)
    }

    /// The Mech walks at least one MP, and its engine rating, tonnage x walking MP and the
    /// rating the template declares, if it declares one, lies within the Master Engine Table (TM p. 48; Tactical Operations: Advanced Units &
    /// Equipment large engines).
    fn validate_movement(&self) -> Result<()> {
        let walking = self.walking_mp();
        ensure!(walking >= 1, "A Mech needs a walking MP of at least 1");
        let rating = rated_output(self.tons, walking);
        if let Some(declared) = self.declared_engine_rating()? {
            ensure!(
                declared == rating,
                "The declared engine rating {declared} is not tonnage x walking MP ({} x {walking} = {rating}) (TM p. 48)",
                self.tons
            );
        }
        ensure!(
            (10..=MAXIMUM_ENGINE_RATING).contains(&rating),
            "Engine rating {rating} is outside 10-{MAXIMUM_ENGINE_RATING}"
        );
        self.jumping_mp()?;
        Ok(())
    }

    /// The installed engine slots match the engine the template declares (TM p. 48).
    fn validate_engine(&self, loadout: &MechLoadout) -> Result<()> {
        let installed = Engine::resolve(loadout, self.clan_engine())?;
        let declared = if self.has_special("XLEngine_Tech") {
            Engine::Xl
        } else if self.has_special("LightEngine_Tech") {
            Engine::Light
        } else if self.has_special("XXL_Tech") {
            Engine::Xxl
        } else if self.has_special("CompactEngine_Tech") {
            Engine::Compact
        } else {
            Engine::Standard
        };
        ensure!(
            installed == declared,
            "Installed {installed:?} engine slots do not match the declared {declared:?} engine"
        );
        let rating = rated_output(self.tons, self.walk_mp);
        let large = rating > 400;
        ensure!(
            !large || installed != Engine::Compact,
            "Compact engines are not built above rating 400"
        );
        let center = super::engine::center_engine_slots(loadout)?;
        let expected = match (installed, large) {
            (Engine::Compact, _) => 3,
            (_, true) => 8,
            (_, false) => 6,
        };
        ensure!(
            center == expected,
            "A rating {rating} engine fills {expected} center torso slots; found {center}"
        );
        Ok(())
    }

    /// One jump jet per jumping MP, at most the walking MP (improved jets: running MP), in
    /// torso and leg locations only (TM p. 51; Tactical Operations: Advanced Units & Equipment
    /// improved jump jets).
    fn validate_jump_jets(&self, loadout: &MechLoadout, chassis: MechChassis) -> Result<()> {
        let improved = self.has_special("ImprovedJJ_Tech");
        let jets = loadout.jump_jet_groups(improved)?;
        let jumping = self.jumping_mp()?;
        ensure!(
            jets.len() as u32 == jumping,
            "Jumping MP {jumping} does not match {} installed jump jets",
            jets.len()
        );
        let ceiling = if improved {
            self.running_mp()
        } else {
            self.walking_mp()
        };
        ensure!(
            jumping <= ceiling,
            "Jumping MP {jumping} exceeds the {} MP of {ceiling}",
            if improved { "running" } else { "walking" }
        );
        ensure!(
            jets.iter().flatten().all(|location| {
                location.section != MechSection::Head
                    && (chassis.is_leg(location.section)
                        || !matches!(
                            location.section,
                            MechSection::LeftArm | MechSection::RightArm
                        ))
            }),
            "Jump jets may only be mounted in torsos and legs"
        );
        Ok(())
    }

    /// At least ten heat sinks; the engine holds up to its rating / 25 (rounded down) of them
    /// and every other sink fills critical slots (TM pp. 52-53).
    fn validate_heat_sinks(&self, loadout: &MechLoadout) -> Result<()> {
        let sinks = u32::from(self.heat_sinks);
        ensure!(
            sinks >= 10,
            "A Mech needs at least 10 heat sinks, not {sinks}"
        );
        let external = loadout.heat_sink_groups(self.heat_sink_slots())?.len() as u32;
        let capacity = rated_output(self.tons, self.walk_mp) / 25;
        ensure!(
            external <= sinks && sinks - external <= capacity,
            "{sinks} heat sinks need {} in critical slots beside the {capacity} the engine holds; {external} are installed",
            sinks.saturating_sub(capacity)
        );
        Ok(())
    }

    /// Endo steel, ferro-fibrous and stealth armor fill exactly their slot counts, and only
    /// when the template declares them (TM pp. 47, 56; stealth armor TM p. 206).
    fn validate_technology_slots(&self, loadout: &MechLoadout, chassis: MechChassis) -> Result<()> {
        let count = |system| {
            loadout
                .systems
                .iter()
                .filter(|part| part.system == system)
                .count()
        };
        let base = |clan: bool| if clan { 7 } else { 14 };
        for (system, declared, slots, label) in [
            (
                System::EndoSteel,
                self.has_special("EndoSteel_Tech"),
                base(self.clan_structure()),
                "Endo steel",
            ),
            (
                System::FerroFibrous,
                self.has_special("FerroFibrous_Tech"),
                base(self.clan_armor()),
                "Ferro-fibrous armor",
            ),
            (
                System::LightFerroFibrous,
                self.has_special("LtFerroFibrous_Tech"),
                7,
                "Light ferro-fibrous armor",
            ),
            (
                System::HeavyFerroFibrous,
                self.has_special("HvyFerroFibrous_Tech"),
                21,
                "Heavy ferro-fibrous armor",
            ),
            (
                System::StealthArmor,
                self.has_special("StealthArmor_Tech"),
                12,
                "Stealth armor",
            ),
        ] {
            let found = count(system);
            let expected = if declared { slots } else { 0 };
            ensure!(
                found == expected,
                "{label} needs {expected} critical slots; found {found}"
            );
        }
        if !self.has_special("StealthArmor_Tech") {
            return Ok(());
        }
        for section in [
            MechSection::LeftArm,
            MechSection::RightArm,
            MechSection::LeftTorso,
            MechSection::RightTorso,
            MechSection::LeftLeg,
            MechSection::RightLeg,
        ] {
            let found = loadout
                .systems
                .iter()
                .filter(|part| {
                    part.system == System::StealthArmor && part.location.section == section
                })
                .count();
            ensure!(
                found == 2,
                "Stealth armor needs two slots in {}",
                chassis.section_name(section)
            );
        }
        ensure!(
            count(System::Ecm) + count(System::AngelEcm) > 0,
            "Stealth armor needs an ECM suite"
        );
        Ok(())
    }

    /// Armor may not exceed twice a location's internal structure, nine points on the head,
    /// with front and rear torso armor sharing the limit (TM p. 54).
    fn validate_maximum_armor(&self) -> Result<()> {
        let chassis = self.chassis()?;
        for (&section, layout) in &self.sections {
            let limit = if section == MechSection::Head {
                MAXIMUM_HEAD_ARMOR
            } else {
                layout.internal * 2
            };
            let armor = layout.armor + layout.rear;
            ensure!(
                armor <= limit,
                "{} carries {armor} armor; the maximum is {limit}",
                chassis.section_name(section)
            );
        }
        Ok(())
    }

    /// MASC fills 5% (Inner Sphere) or 4% (Clan) of the tonnage in one contiguous run, triple
    /// strength myomer fills six slots on an Inner Sphere Mech, and the two are never combined
    /// (TM p. 52). A supercharger fills one slot of a location holding engine slots (Tactical
    /// Operations: Advanced Units & Equipment p. 223).
    fn validate_myomer_and_boosters(&self, loadout: &MechLoadout) -> Result<()> {
        let slots = |system| -> Vec<_> {
            loadout
                .systems
                .iter()
                .filter(|part| part.system == system)
                .map(|part| part.location)
                .collect()
        };
        let masc = slots(System::Masc);
        if !masc.is_empty() {
            ensure!(
                masc.len() == self.masc_slots() && contiguous(&masc),
                "MASC needs {} contiguous critical slots in one location; found {}",
                self.masc_slots(),
                masc.len()
            );
        }
        let myomer = slots(System::TripleStrengthMyomer);
        let declared = self.has_special("TripleMyomerTech");
        ensure!(
            myomer.len() == if declared { 6 } else { 0 },
            "Triple strength myomer needs 6 critical slots and the triple_strength myomer choice; found {}",
            myomer.len()
        );
        ensure!(
            !declared || !self.has_special("Clan") || self.has_special(MIXED_TECH),
            "Clan Mechs may not mount triple strength myomer unless they are mixed technology"
        );
        ensure!(
            !declared || masc.is_empty(),
            "MASC and triple strength myomer may not be combined"
        );
        let superchargers = slots(System::Supercharger);
        ensure!(
            superchargers.len() == usize::from(self.has_special("SuperCharger_Tech")),
            "A supercharger needs the SuperCharger_Tech special and one critical slot; found {}",
            superchargers.len()
        );
        for location in superchargers {
            ensure!(
                loadout
                    .systems
                    .iter()
                    .any(|part| part.system == System::Engine
                        && part.location.section == location.section),
                "A supercharger must share a location with engine slots"
            );
        }
        Ok(())
    }

    /// A targeting computer weighs, and fills as many contiguous slots in one location as,
    /// the tonnage of the direct-fire weapons divided by 4 for an Inner Sphere computer or 5
    /// for a Clan one, rounded up (TM p. 238).
    fn validate_targeting_computer(&self, loadout: &MechLoadout) -> Result<()> {
        let computer: Vec<_> = loadout
            .systems
            .iter()
            .filter(|part| part.system == System::TargetingComputer)
            .collect();
        if computer.is_empty() {
            return Ok(());
        }
        let clan = computer[0].is_clan(self.has_special("Clan"));
        ensure!(
            computer
                .iter()
                .all(|part| part.is_clan(self.has_special("Clan")) == clan),
            "A targeting computer is built from one technology base"
        );
        let weapons: u32 = loadout
            .weapons
            .iter()
            .filter(|mount| mount.weapon.is_direct_fire())
            .map(|mount| mount.weapon.mass())
            .sum();
        let expected = super::mass::targeting_computer_tons(
            loadout.weapons.iter().map(|mount| mount.weapon),
            clan,
        ) as usize;
        let locations: Vec<_> = computer.iter().map(|part| part.location).collect();
        ensure!(
            computer.len() == expected && contiguous(&locations),
            "A{} targeting computer for {:.1} tons of direct-fire weapons needs {expected} contiguous critical slots in one location; found {}",
            if clan { " Clan" } else { "n Inner Sphere" },
            f64::from(weapons) / 1024.0,
            computer.len()
        );
        Ok(())
    }

    /// Physical weapons sit in biped arms with the actuators they need, one per location,
    /// each filling its tonnage-dependent slot count (TM pp. 220, 237, 241, 249; Tactical
    /// Operations: Advanced Units & Equipment melee weapons). A quad mounts none. TM pp. 241
    /// and 243 would let it put a backhoe or a dual saw in a side torso, but no Total Warfare or
    /// Tactical Operations rule lets a quad attack with either, so construction refuses them.
    fn validate_physical_weapons(&self, loadout: &MechLoadout, chassis: MechChassis) -> Result<()> {
        let mut by_section: BTreeMap<MechSection, Vec<(System, usize)>> = BTreeMap::new();
        for part in &loadout.systems {
            if physical_weapon(part.system, self.tons).is_none() {
                continue;
            }
            let items = by_section.entry(part.location.section).or_default();
            match items.iter_mut().find(|(system, _)| *system == part.system) {
                Some((_, slots)) => *slots += 1,
                None => items.push((part.system, 1)),
            }
        }
        for (section, items) in by_section {
            let name = chassis.section_name(section);
            let [(system, slots)] = items.as_slice() else {
                bail!("{name} mounts more than one physical weapon");
            };
            let weapon = physical_weapon(*system, self.tons).expect("filtered physical weapon");
            if chassis == MechChassis::Quad {
                let item = match system {
                    System::DualSaw => "dual saw",
                    System::Backhoe => "backhoe",
                    _ => bail!(
                        "A quad mounts no physical weapon; {system:?} in {name} is refused (TM pp. 220, 237, 249)"
                    ),
                };
                bail!(
                    "A quad may not mount a {item}: no rule lets a four-legged Mech attack with one, so {name}'s {item} is refused"
                );
            }
            ensure!(
                matches!(section, MechSection::LeftArm | MechSection::RightArm),
                "Physical weapons may only be mounted in a biped's arms, not {name}"
            );
            ensure!(
                *slots == weapon.slots,
                "{system:?} on a {}-ton Mech fills {} critical slots; found {slots}",
                self.tons,
                weapon.slots
            );
            let actuator = |system| {
                loadout
                    .systems
                    .iter()
                    .any(|part| part.system == system && part.location.section == section)
            };
            ensure!(
                !weapon.full_arm
                    || (actuator(System::ShoulderOrHip)
                        && actuator(System::UpperActuator)
                        && actuator(System::LowerActuator)),
                "{system:?} needs shoulder, upper and lower arm actuators in {name}"
            );
            ensure!(
                !weapon.hand || actuator(System::HandOrFootActuator),
                "{system:?} needs a hand actuator in {name}"
            );
            ensure!(
                !weapon.replaces_hand || !actuator(System::HandOrFootActuator),
                "{system:?} replaces the hand actuator in {name}"
            );
        }
        Ok(())
    }

    /// Multi-slot electronics fill their whole slot count as one item, in contiguous slots of
    /// one location (TM p. 45): the Guardian ECM suite and Beagle probe two slots (their Clan
    /// counterparts one, the Watchdog suite two), the C3 master five, C3i two, Bloodhound
    /// three (TM pp. 341-343), the lift hoist three (TM p. 344) and the Angel ECM suite two
    /// (Tactical Operations: Advanced Units & Equipment p. 217). The null-signature system fills
    /// one slot in every location but the head (ibid. p. 148). Lift hoists and communications
    /// equipment follow their own placement rules too.
    fn validate_installations(&self, loadout: &MechLoadout, chassis: MechChassis) -> Result<()> {
        let clan = self.has_special("Clan");
        let context = super::SystemContext {
            clan,
            watchdog: self.has_technology(super::Technology::Watchdog),
            tons: self.tons,
            engine_rating: 0,
            legs: if chassis == MechChassis::Quad { 4 } else { 2 },
        };
        // Slots of each electronic system by location and technology base.
        let mut runs: BTreeMap<(MechSection, System, bool), Vec<u8>> = BTreeMap::new();
        for part in &loadout.systems {
            if !matches!(
                part.system,
                System::BeagleProbe
                    | System::Ecm
                    | System::AngelEcm
                    | System::C3Master
                    | System::C3i
                    | System::BloodhoundProbe
                    | System::LiftHoist
            ) {
                continue;
            }
            runs.entry((part.location.section, part.system, part.is_clan(clan)))
                .or_default()
                .push(part.location.slot);
        }
        for ((section, system, clan), mut slots) in runs {
            let size = system
                .installation_slots(super::SystemContext { clan, ..context })
                .unwrap_or(1);
            slots.sort_unstable();
            // Contiguous runs, each a whole number of installations.
            let mut run = 1;
            for (index, slot) in slots.iter().enumerate() {
                let ends = slots.get(index + 1).is_none_or(|next| *next != slot + 1);
                if !ends {
                    run += 1;
                    continue;
                }
                ensure!(
                    run % size == 0,
                    "{system:?} fills {size} contiguous critical slots per installation; {} holds a run of {run} (TM p. 45)",
                    chassis.section_name(section)
                );
                run = 1;
            }
        }
        self.validate_lift_hoists(loadout, chassis)?;
        self.validate_communications_equipment(loadout, chassis)?;
        let null_signature = |section| {
            loadout
                .systems
                .iter()
                .filter(|part| {
                    part.system == System::NullSignature && part.location.section == section
                })
                .count()
        };
        if MechSection::ALL
            .into_iter()
            .any(|section| null_signature(section) > 0)
        {
            for section in MechSection::ALL {
                let expected = usize::from(section != MechSection::Head);
                ensure!(
                    null_signature(section) == expected,
                    "The null-signature system fills one critical slot in every location but the head; {} has {} (Tactical Operations: Advanced Units & Equipment p. 148)",
                    chassis.section_name(section),
                    null_signature(section)
                );
            }
        }
        Ok(())
    }

    /// A Mech mounts at most two lift hoists, in its arms and torsos (TM p. 245); a quad's
    /// front legs are legs.
    fn validate_lift_hoists(&self, loadout: &MechLoadout, chassis: MechChassis) -> Result<()> {
        let hoists: Vec<_> = loadout
            .systems
            .iter()
            .filter(|part| part.system == System::LiftHoist)
            .collect();
        for part in &hoists {
            let section = part.location.section;
            ensure!(
                section != MechSection::Head && !chassis.is_leg(section),
                "A Mech mounts lift hoists in its arms and torsos, not {} (TM p. 245)",
                chassis.section_name(section)
            );
        }
        // Each hoist fills three slots, which the installation check counts first.
        ensure!(
            hoists.len() <= 6,
            "A Mech mounts at most two lift hoists; found {} (TM p. 245)",
            hoists.len() / 3
        );
        Ok(())
    }

    /// A Mech adds one set of communications equipment, one to fifteen whole tons, in one
    /// contiguous run of a single location, a ton to each slot (TM p. 212).
    fn validate_communications_equipment(
        &self,
        loadout: &MechLoadout,
        chassis: MechChassis,
    ) -> Result<()> {
        let slots: Vec<_> = loadout
            .systems
            .iter()
            .filter(|part| part.system == System::CommunicationsEquipment)
            .map(|part| part.location)
            .collect();
        if slots.is_empty() {
            return Ok(());
        }
        ensure!(
            contiguous(&slots),
            "A Mech's communications equipment is one set in contiguous slots of one location (TM p. 212)"
        );
        ensure!(
            slots.len() <= 15,
            "A set of communications equipment weighs at most 15 tons; {} slots fill {} in {} (TM p. 212)",
            slots.len(),
            slots.len(),
            chassis.section_name(slots[0].section)
        );
        for location in &slots {
            let data = self
                .sections
                .get(&location.section)
                .and_then(|section| section.criticals.get(&location.slot))
                .map_or("-", |critical| critical.data.as_str());
            ensure!(
                data == "-",
                "A Mech's communications equipment weighs a ton per slot and states no tons (TM p. 212)"
            );
        }
        Ok(())
    }

    /// A BattleMech's standard cargo bay fills one critical slot per ton of its capacity,
    /// rounded up (TechManual p. 239, Transport Bay table): `cargo_tons = 1.5` needs two
    /// `Cargo` slots, which may sit in any locations.
    fn validate_cargo_bay(&self, loadout: &MechLoadout) -> Result<()> {
        let capacity =
            super::cargo_mass(self.attributes.get(super::CARGO_TONS).map(String::as_str))?;
        let expected = capacity.div_ceil(1024) as usize;
        let found = loadout
            .systems
            .iter()
            .filter(|part| part.system == System::Cargo)
            .count();
        ensure!(
            found == expected,
            "A cargo bay of {} tons fills {expected} Cargo critical slots, one per ton rounded up; found {found} (TechManual p. 239)",
            super::format_tons(capacity)
        );
        Ok(())
    }

    /// Artemis IV or V serves every applicable launcher or none, each linked to the launcher it
    /// guides in its own location (TM p. 206), as the MRM Apollo system serves every MRM
    /// launcher (Tactical Operations: Advanced Units & Equipment p. 142); CASE goes in torsos (TM p. 210); an
    /// internal combustion Mech mounts no standard flamer (TM p. 218); rear-firing weapons sit in
    /// torsos, legs or the head (TM p. 46); a heavy Gauss rifle sits in the torsos, splitting
    /// only between a side and the center torso (TM p. 218); and only ranged weapons of eight
    /// or more slots split, across two adjacent locations other than the legs (TM p. 47).
    fn validate_weapon_mounting(&self, loadout: &MechLoadout, chassis: MechChassis) -> Result<()> {
        let torso = |section| {
            matches!(
                section,
                MechSection::LeftTorso | MechSection::RightTorso | MechSection::CenterTorso
            )
        };
        // Each Artemis or Apollo system links the first slot of the launcher it guides in its
        // own location; Artemis V fills two slots per launcher (Tactical Operations: Advanced
        // Units & Equipment p. 217).
        super::fire_control::check_fire_control_links(
            loadout,
            &self.sections,
            |section| chassis.section_name(section),
            2,
        )?;
        // CASE as an item is Inner Sphere CASE wherever it is mounted; Clan CASE is built into
        // Clan structure (TM p. 210).
        ensure!(
            loadout
                .systems
                .iter()
                .all(|part| part.system != System::Case || torso(part.location.section)),
            "Inner Sphere CASE may only be mounted in torso locations"
        );
        // Only fusion and fission units mount standard flamers; an internal combustion engine
        // takes vehicle flamers (TechManual p. 218).
        let combustion = self.has_special("ICEEngine_Tech");
        for mount in &loadout.weapons {
            let primary = mount.criticals[0].section;
            let name = mount.weapon.name();
            ensure!(
                !combustion
                    || !matches!(
                        mount.weapon,
                        super::Weapon::Flamer | super::Weapon::ClanFlamer
                    ),
                "{name} needs a fusion engine; an internal combustion Mech mounts vehicle flamers instead (TechManual p. 218)"
            );
            ensure!(
                !mount.rear_mount
                    || torso(primary)
                    || primary == MechSection::Head
                    || chassis.is_leg(primary),
                "{name} may only fire to the rear from a torso, leg or head location"
            );
            let sections: Vec<_> = {
                let mut sections: Vec<_> =
                    mount.criticals.iter().map(|slot| slot.section).collect();
                sections.dedup();
                sections
            };
            if mount.weapon == super::Weapon::HeavyGaussRifle {
                ensure!(
                    sections.iter().all(|section| torso(*section))
                        && (sections.len() == 1 || sections.contains(&MechSection::CenterTorso)),
                    "A heavy Gauss rifle may only be mounted in the torsos, split between a side and the center torso"
                );
            }
            if sections.len() > 1 {
                ensure!(
                    mount.weapon.supports_split_mount()
                        && sections.iter().all(|section| !chassis.is_leg(*section)),
                    "{name} may not be split across {}",
                    sections
                        .iter()
                        .map(|section| chassis.section_name(*section))
                        .collect::<Vec<_>>()
                        .join(" and ")
                );
            }
        }
        Ok(())
    }

    /// A Mech builds its engine, structure and armor and mounts its weapons and equipment from
    /// its own technology base unless the template declares mixed technology (TM p. 46;
    /// Tactical Operations: Advanced Units & Equipment p. 189). The Watchdog suite is Clan
    /// equipment. Compact, heavy-duty and XL gyros (TM p. 50) and the small cockpit (TM
    /// p. 52) are Inner Sphere chassis choices, and composite structure is Inner Sphere
    /// structure (Tactical Operations: Advanced Units & Equipment p. 154).
    fn validate_technology_base(&self, loadout: &MechLoadout) -> Result<()> {
        let clan = self.has_special("Clan");
        let mixed_tech = self.has_special(MIXED_TECH);
        ensure!(
            clan || mixed_tech || !self.has_technology(super::Technology::Watchdog),
            "The Watchdog suite is Clan technology on an Inner Sphere Mech; declare mixed_tech in [construction]"
        );
        ensure!(
            !self.has_technology(super::Technology::CompositeStructure) || !self.clan_structure(),
            "Composite structure is Inner Sphere technology; a Clan chassis builds it only as Inner Sphere structure (structure_tech = \"inner_sphere\") on a mixed_tech design (Tactical Operations: Advanced Units & Equipment p. 154)"
        );
        let mut chassis_technologies = Vec::new();
        let gyro = match super::Gyro::from_definition(self) {
            super::Gyro::Standard => None,
            super::Gyro::Xl => Some("XL gyro"),
            super::Gyro::Compact => Some("compact gyro"),
            super::Gyro::Hardened => Some("heavy-duty gyro"),
        };
        if let Some(name) = gyro {
            chassis_technologies.push(ChassisTechnology {
                name,
                base: TechBase::InnerSphere,
                citation: "TechManual p. 50",
            });
        }
        if self.has_technology(super::Technology::SmallCockpit) {
            chassis_technologies.push(ChassisTechnology {
                name: "small cockpit",
                base: TechBase::InnerSphere,
                citation: "TechManual p. 52",
            });
        }
        // CASE as a BattleMech item is Inner Sphere CASE: Clan CASE comes with Clan internal
        // structure in every location rather than being installed (TechManual p. 210), whatever
        // the chassis's own base.
        ensure!(
            !self.clan_structure()
                || loadout
                    .systems
                    .iter()
                    .all(|part| part.system != System::Case),
            "A Mech with Clan internal structure has CASE built into every location and installs no Case item (TechManual p. 210)"
        );
        check_technology_base(TechnologyInventory {
            unit: "Mech",
            base: TechBase::of(clan),
            mixed_tech,
            other_base_components: Component::ALL
                .into_iter()
                .filter(|component| self.has_special(component.flag(!clan)))
                .collect(),
            chassis_technologies,
            weapons: loadout.weapons.iter().map(|mount| mount.weapon),
            systems: loadout
                .systems
                .iter()
                .map(|part| (part.system, part.tech_base)),
        })
    }

    /// Every component together weighs the Mech's tonnage: none over, and no more than the
    /// half-ton rounding of the armor under (TM pp. 46, 57).
    fn validate_total_mass(&self, loadout: &MechLoadout) -> Result<()> {
        let total = self.construction_mass(loadout)?.total;
        let tons = u32::from(self.tons) * 1024;
        ensure!(
            total <= tons,
            "Components weigh {:.2} tons, over the {}-ton chassis",
            f64::from(total) / 1024.0,
            self.tons
        );
        ensure!(
            total + 512 >= tons,
            "Components weigh {:.2} tons, under the {}-ton chassis",
            f64::from(total) / 1024.0,
            self.tons
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{MechTemplate, check_template};

    const JENNER: &str = include_str!("../tests/fixtures/JR7-D.toml");

    /// The rejection a Jenner variant earns, or `None` when it constructs.
    fn rejection(source: &str) -> Option<String> {
        check_template(&MechTemplate::parse("JR7-D", source).unwrap()).rejection
    }

    /// Assert that `source` is refused for a reason containing `expected`.
    fn refused(source: &str, expected: &str) {
        assert_ne!(source, JENNER, "the variant must differ from the Jenner");
        let rejection = rejection(source).unwrap_or_default();
        assert!(rejection.contains(expected), "{expected}: {rejection}");
    }

    /// A Jenner whose SRM-4 moves to right torso slot 4 with `controllers` beside it and
    /// `left` in the left torso, sixteen armor points lighter to pay for a ton of fire control.
    fn guided_jenner(controllers: &str, left: &str) -> String {
        JENNER
            .replace("    { at = 11, item = \"IS.SRM-4\" },\n", "")
            .replace(
                "    { at = \"2-3\", item = \"JumpJet\" },\n]",
                &format!(
                    "    {{ at = \"2-3\", item = \"JumpJet\" }},\n    {{ at = 4, item = \"IS.SRM-4\" }},\n{controllers}]"
                ),
            )
            .replace(
                "slots = [\n    { at = \"1-2\", item = \"JumpJet\" },\n]",
                &format!("slots = [\n    {{ at = \"1-2\", item = \"JumpJet\" }},\n{left}]"),
            )
            .replace("[sections.left_leg]\narmor = 6", "[sections.left_leg]\narmor = 0")
            .replace("[sections.right_leg]\narmor = 6", "[sections.right_leg]\narmor = 0")
            .replace("armor = 4\nomit", "armor = 2\nomit")
    }

    /// Every Artemis entry names the first slot of the launcher it guides in its own
    /// location, no two installations share a launcher, and Artemis V fills two slots per
    /// launcher (TechManual p. 206; Tactical Operations: Advanced Units & Equipment p. 217).
    #[test]
    fn fire_control_links_are_explicit_and_unique() {
        let artemis =
            |at: u8, link: &str| format!("    {{ at = {at}, item = \"ArtemisIV\"{link} }},\n");
        assert_eq!(
            rejection(&guided_jenner(&artemis(5, ", link = 4"), "")),
            None
        );
        refused(
            &guided_jenner(&artemis(5, ""), ""),
            "The Artemis IV in Right_Torso slot 5 has no launcher link",
        );
        refused(
            &guided_jenner(&artemis(5, ", link = 1"), ""),
            "The Artemis IV in Right_Torso slot 5 links slot 1, which is not the first slot of an LRM, SRM or MML launcher in Right_Torso",
        );
        refused(
            &guided_jenner("", &artemis(4, ", link = 4")),
            "The Artemis IV in Left_Torso slot 4 links slot 4, which is not the first slot of an LRM, SRM or MML launcher in Left_Torso",
        );
        refused(
            &guided_jenner(
                &format!("{}{}", artemis(5, ", link = 4"), artemis(6, ", link = 4")),
                "",
            ),
            "2 fire-control entries link the",
        );
        let mixed = |controllers: &str| {
            guided_jenner(controllers, "").replace(
                "engine_rating = 245",
                "engine_rating = 245\nmixed_tech = true",
            )
        };
        let five = |at: u8| format!("    {{ at = {at}, item = \"ArtemisV\", link = 4 }},\n");
        refused(&mixed(&five(5)), "1 fire-control entries link the");
        // The pair passes every rule but mass: Artemis V weighs half a ton more than the
        // armor freed for it.
        refused(
            &mixed(&format!("{}{}", five(5), five(6))),
            "over the 35-ton chassis",
        );
    }

    #[test]
    fn the_jenner_record_sheet_constructs() {
        assert_eq!(rejection(JENNER), None);
    }

    /// The template states walking MP, running MP is walking MP x 1.5 rounded up, and the
    /// rating, tonnage x walking MP, stays within 10-500 (TM p. 48).
    #[test]
    fn the_engine_rating_follows_the_walking_mp() {
        let jenner = MechTemplate::parse("JR7-D", JENNER).unwrap();
        assert_eq!((jenner.walking_mp(), jenner.running_mp()), (7, 11));
        assert_eq!(jenner.max_speed, 11.0 * 10.75);
        let mut still = jenner.clone();
        still.walk_mp = 0;
        let rejection = check_template(&still).rejection.unwrap();
        assert!(
            rejection.contains("walking MP of at least 1"),
            "{rejection}"
        );
        let mut fast = jenner.clone();
        fast.set_walking_mp(15);
        let rejection = check_template(&fast).rejection.unwrap();
        assert!(rejection.contains("Engine rating 525"), "{rejection}");
    }

    /// A declared engine rating must be tonnage x walking MP (TM p. 48) and follows a walking
    /// MP edit; a template without one derives it.
    #[test]
    fn a_declared_engine_rating_matches_the_walking_mp() {
        let jenner = MechTemplate::parse("JR7-D", JENNER).unwrap();
        assert_eq!(jenner.declared_engine_rating().unwrap(), Some(245));
        refused(
            &JENNER.replace("engine_rating = 245", "engine_rating = 250"),
            "declared engine rating 250 is not tonnage x walking MP (35 x 7 = 245)",
        );
        let derived =
            MechTemplate::parse("JR7-D", &JENNER.replace("engine_rating = 245\n", "")).unwrap();
        assert_eq!(derived.declared_engine_rating().unwrap(), None);
        assert_eq!(check_template(&derived).rejection, None);
        let mut slower = jenner;
        slower.set_walking_mp(6);
        assert_eq!(slower.declared_engine_rating().unwrap(), Some(210));
    }

    /// One jet per jumping MP, never more than the walking MP, in torsos and legs (TM p. 51).
    #[test]
    fn jump_jets_set_the_jumping_mp() {
        refused(
            &JENNER.replace("jump_mp = 5", "jump_mp = 4"),
            "Jumping MP 4 does not match 5 installed jump jets",
        );
        refused(
            &JENNER
                .replace("jump_mp = 5", "jump_mp = 6")
                .replace(
                    "    { at = 11, item = \"IS.SRM-4\" },",
                    "    { at = 11, item = \"JumpJet\" },",
                )
                .replace("walk_mp = 7", "walk_mp = 5")
                .replace("engine_rating = 245", "engine_rating = 175"),
            "exceeds the walking MP of 5",
        );
        refused(
            &JENNER
                .replace("    { at = \"1-2\", item = \"JumpJet\" },\n", "    { at = 1, item = \"JumpJet\" },\n")
                .replacen(
                    "    { at = \"3-4\", item = \"IS.MediumLaser\" },",
                    "    { at = 3, item = \"IS.MediumLaser\" },\n    { at = 4, item = \"JumpJet\" },",
                    1,
                ),
            "Jump jets may only be mounted in torsos and legs",
        );
    }

    /// Ten sinks at least, the engine holding rating / 25 of them (TM pp. 52-53); a Clan
    /// chassis defaults to ten double sinks.
    #[test]
    fn heat_sinks_are_counted_in_sinks() {
        refused(
            &JENNER.replace("walk_mp = 7", "walk_mp = 7\nheat_sinks = 9"),
            "at least 10 heat sinks",
        );
        refused(
            &JENNER.replace("walk_mp = 7", "walk_mp = 7\nheat_sinks = 12"),
            "12 heat sinks need 3 in critical slots",
        );
        let clan = MechTemplate::parse(
            "JR7-D",
            &JENNER.replace("tech_base = \"inner_sphere\"", "tech_base = \"clan\""),
        )
        .unwrap();
        assert_eq!((clan.heat_sinks, clan.heat_dissipation()), (10, 20));
    }

    /// Endo steel fills exactly its slots, and only when declared (TM p. 47).
    #[test]
    fn structure_slots_are_exact() {
        refused(
            &JENNER.replace(
                "[sections.left_leg]\narmor = 6\n",
                "[sections.left_leg]\narmor = 6\nslots = [\n    { at = 5, item = \"EndoSteel\" },\n]\n",
            ),
            "Endo steel needs 0 critical slots; found 1",
        );
    }

    /// Armor stops at twice the structure and nine points on the head (TM p. 54).
    #[test]
    fn armor_is_capped_per_location() {
        refused(
            &JENNER.replace("[sections.head]\narmor = 7", "[sections.head]\narmor = 10"),
            "Head carries 10 armor; the maximum is 9",
        );
        refused(
            &JENNER.replace("armor = 10\nrear = 3", "armor = 20\nrear = 3"),
            "Center_Torso carries 23 armor; the maximum is 22",
        );
    }

    /// MASC fills 5% of an Inner Sphere Mech's tonnage, rounded normally (TM p. 52).
    #[test]
    fn masc_fills_its_tonnage_share() {
        let masc = |slots: &str| {
            JENNER.replace(
                "    { at = \"1-2\", item = \"JumpJet\" },\n]",
                &format!("    {{ at = \"1-2\", item = \"JumpJet\" }},\n    {{ at = \"{slots}\", item = \"Masc\" }},\n]"),
            )
        };
        refused(&masc("3-5"), "MASC needs 2 contiguous critical slots");
        let template = MechTemplate::parse("JR7-D", JENNER).unwrap();
        assert_eq!(template.masc_slots(), 2);
    }

    /// A targeting computer weighs a ton per four tons of direct-fire weapons (TM p. 238).
    #[test]
    fn targeting_computers_follow_their_weapons() {
        refused(
            &JENNER.replace(
                "    { at = \"1-2\", item = \"JumpJet\" },\n]",
                "    { at = \"1-2\", item = \"JumpJet\" },\n    { at = \"3-4\", item = \"TargetingComputer\" },\n]",
            ),
            "needs 1 contiguous critical slots",
        );
    }

    /// Rear-firing weapons sit in torsos, legs or the head (TM p. 46), and Inner Sphere CASE in
    /// a torso (TM p. 210).
    #[test]
    fn mounting_locations_follow_the_book() {
        refused(
            &JENNER.replacen(
                "{ at = \"3-4\", item = \"IS.MediumLaser\" }",
                "{ at = \"3-4\", item = \"IS.MediumLaser\", modes = [\"RearMount\"] }",
                1,
            ),
            "may only fire to the rear",
        );
        refused(
            &JENNER.replacen(
                "    { at = \"3-4\", item = \"IS.MediumLaser\" },",
                "    { at = \"3-4\", item = \"IS.MediumLaser\" },\n    { at = 5, item = \"Case\" },",
                1,
            ),
            "Inner Sphere CASE may only be mounted in torso locations",
        );
    }

    /// A Jenner with `entry` added to its left torso.
    fn with_left_torso(entry: &str) -> String {
        JENNER.replace(
            "    { at = \"1-2\", item = \"JumpJet\" },\n]",
            &format!("    {{ at = \"1-2\", item = \"JumpJet\" }},\n{entry}]"),
        )
    }

    /// Multi-slot electronics fill their whole slot count as one item in contiguous slots of
    /// one location (TM pp. 45, 341-343), and the null-signature system one slot in every
    /// location but the head (Tactical Operations: Advanced Units & Equipment p. 148).
    #[test]
    fn multi_slot_electronics_are_complete_installations() {
        let entry = |at: &str, item: &str| format!("    {{ at = {at}, item = \"{item}\" }},\n");
        refused(
            &with_left_torso(&entry("3", "BeagleProbe")),
            "BeagleProbe fills 2 contiguous critical slots per installation; Left_Torso holds a run of 1",
        );
        refused(
            &with_left_torso(&format!(
                "{}{}",
                entry("3", "BeagleProbe"),
                entry("5", "BeagleProbe")
            )),
            "a run of 1",
        );
        refused(
            &with_left_torso(&entry("\"3-6\"", "C3Master")),
            "C3Master fills 5 contiguous critical slots per installation; Left_Torso holds a run of 4",
        );
        refused(
            &with_left_torso(&entry("\"3-4\"", "BloodhoundProbe")),
            "BloodhoundProbe fills 3 contiguous critical slots",
        );
        // Complete installations pass this rule and stop at the mass.
        for (at, item) in [
            ("\"3-4\"", "BeagleProbe"),
            ("\"3-6\"", "Ecm"),
            ("\"3-5\"", "BloodhoundProbe"),
        ] {
            let rejection = rejection(&with_left_torso(&entry(at, item))).unwrap_or_default();
            assert!(
                !rejection.contains("per installation"),
                "{item}: {rejection}"
            );
            assert!(
                rejection.contains("over the 35-ton chassis"),
                "{item}: {rejection}"
            );
        }
        // The Clan active probe is a one-slot item.
        let mixed = with_left_torso(&entry("3", "CL.BeagleProbe")).replacen(
            "[construction]\n",
            "[construction]\nmixed_tech = true\n",
            1,
        );
        let rejection = rejection(&mixed).unwrap_or_default();
        assert!(!rejection.contains("per installation"), "{rejection}");
        refused(
            &with_left_torso(&entry("3", "NullSig_Device")),
            "The null-signature system fills one critical slot in every location but the head; Left_Arm has 0",
        );
    }

    /// The Jenner's right arm with `omit` and `slots` in place of its own.
    fn right_arm(omit: &str, slots: &str) -> String {
        JENNER.replace(
            "[sections.right_arm]\narmor = 4\nomit = [\"lower_actuator\", \"hand_actuator\"]\nslots = [\n    { at = \"3-4\", item = \"IS.MediumLaser\" },\n]",
            &format!("[sections.right_arm]\narmor = 4\n{omit}slots = [\n{slots}]"),
        )
    }

    /// A retractable blade needs shoulder, upper and lower arm actuators but no hand (TM
    /// p. 237); a dual saw and a wrecking ball replace the hand actuator of the arm they sit
    /// in (TM pp. 243, 249).
    #[test]
    fn melee_weapons_need_their_arm_actuators() {
        let blade = "    { at = \"5-7\", item = \"Retractable_Blade\" },\n";
        refused(
            &right_arm("omit = [\"lower_actuator\", \"hand_actuator\"]\n", blade),
            "RetractableBlade needs shoulder, upper and lower arm actuators in Right_Arm",
        );
        let bladed =
            rejection(&right_arm("omit = [\"hand_actuator\"]\n", blade)).unwrap_or_default();
        assert!(!bladed.contains("actuator"), "{bladed}");
        for (item, system, at) in [
            ("Dual_Saw", "DualSaw", "\"5-11\""),
            ("Wrecking_Ball", "WreckingBall", "\"5-9\""),
        ] {
            let entry = format!("    {{ at = {at}, item = \"{item}\" }},\n");
            refused(
                &right_arm("", &entry),
                &format!("{system} replaces the hand actuator in Right_Arm"),
            );
            let handless =
                rejection(&right_arm("omit = [\"hand_actuator\"]\n", &entry)).unwrap_or_default();
            assert!(!handless.contains("actuator"), "{item}: {handless}");
        }
    }

    /// A quad mounts no physical weapon. TM pp. 241 and 243 would put a backhoe or a dual saw
    /// in a side torso, but no rule lets a quad attack with either, so construction refuses
    /// them; other physical weapons belong in arms a quad lacks (TM pp. 220, 237, 249).
    #[test]
    fn quads_mount_no_physical_weapons() {
        let quad = JENNER
            .replace("movement = \"biped\"", "movement = \"quad\"")
            .replace("[sections.left_arm]", "[sections.front_left_leg]")
            .replace("[sections.right_arm]", "[sections.front_right_leg]")
            .replace("[sections.left_leg]", "[sections.rear_left_leg]")
            .replace("[sections.right_leg]", "[sections.rear_right_leg]")
            .replace("omit = [\"lower_actuator\", \"hand_actuator\"]\n", "")
            .replace(
                "{ at = \"3-4\", item = \"IS.MediumLaser\" }",
                "{ at = \"5-6\", item = \"IS.MediumLaser\" }",
            );
        let mounted = |item: &str, at: &str| {
            quad.replace(
                "    { at = \"1-2\", item = \"JumpJet\" },\n]",
                &format!("    {{ at = \"1-2\", item = \"JumpJet\" }},\n    {{ at = {at}, item = \"{item}\" }},\n]"),
            )
        };
        refused(
            &mounted("Backhoe", "\"3-8\""),
            "A quad may not mount a backhoe: no rule lets a four-legged Mech attack with one, so Left_Torso's backhoe is refused",
        );
        refused(
            &mounted("Dual_Saw", "\"3-9\""),
            "A quad may not mount a dual saw: no rule lets a four-legged Mech attack with one, so Left_Torso's dual saw is refused",
        );
        refused(
            &mounted("Wrecking_Ball", "\"3-7\""),
            "A quad mounts no physical weapon; WreckingBall in Left_Torso is refused",
        );
    }

    /// A Mech's communications equipment is one set, a ton to each contiguous slot of one
    /// location (TM p. 212); lift hoists fill three slots each, at most two, in the arms and
    /// torsos (TM pp. 245, 344).
    #[test]
    fn communications_equipment_and_lift_hoists_follow_their_placement() {
        let entry = |at: &str, item: &str| format!("    {{ at = {at}, item = \"{item}\" }},\n");
        // Each complete placement passes these rules and stops at the mass.
        for (at, item) in [
            ("\"3-5\"", "Communications_Equipment"),
            ("\"3-5\"", "Lift_Hoist"),
        ] {
            let placed = rejection(&with_left_torso(&entry(at, item))).unwrap_or_default();
            assert!(
                placed.contains("over the 35-ton chassis"),
                "{item}: {placed}"
            );
        }
        refused(
            &with_left_torso(&format!(
                "{}{}",
                entry("3", "Communications_Equipment"),
                entry("5", "Communications_Equipment")
            )),
            "communications equipment is one set in contiguous slots of one location",
        );
        refused(
            &with_left_torso(
                "    { at = \"3-4\", item = \"Communications_Equipment\", tons = 2 },\n",
            ),
            "weighs a ton per slot and states no tons",
        );
        refused(
            &with_left_torso(&entry("\"3-4\"", "Lift_Hoist")),
            "LiftHoist fills 3 contiguous critical slots per installation",
        );
        refused(
            &with_left_torso(&entry("\"3-11\"", "Lift_Hoist")),
            "A Mech mounts at most two lift hoists; found 3",
        );
        refused(
            &JENNER.replace(
                "[sections.left_leg]\narmor = 6\n",
                "[sections.left_leg]\narmor = 6\nslots = [{ at = \"5-6\", item = \"Lift_Hoist\" }]\n",
            ),
            "LiftHoist fills 3 contiguous critical slots",
        );
    }

    /// A backhoe replaces its arm's hand actuator and fills six slots (TM pp. 241, 344).
    #[test]
    fn backhoes_replace_the_hand_actuator() {
        let entry = "    { at = \"5-10\", item = \"Backhoe\" },\n";
        refused(
            &right_arm("", entry),
            "Backhoe replaces the hand actuator in Right_Arm",
        );
        let handless =
            rejection(&right_arm("omit = [\"hand_actuator\"]\n", entry)).unwrap_or_default();
        assert!(!handless.contains("actuator"), "{handless}");
        refused(
            &right_arm(
                "omit = [\"hand_actuator\"]\n",
                "    { at = \"5-9\", item = \"Backhoe\" },\n",
            ),
            "Backhoe on a 35-ton Mech fills 6 critical slots; found 5",
        );
    }

    /// A Mech's cargo bay fills a critical slot per ton of capacity, rounded up (TM p. 239).
    #[test]
    fn cargo_bays_fill_a_slot_per_ton() {
        // A ton and a half of cargo in the left leg, paid for by 24 points of arm, leg and
        // head armor.
        let cargo = |slots: &str| {
            JENNER
                .replace("walk_mp = 7", "walk_mp = 7\ncargo_tons = 1.5")
                .replace("armor = 4\nomit", "armor = 0\nomit")
                .replace(
                    "[sections.left_leg]\narmor = 6\n",
                    &format!("[sections.left_leg]\narmor = 0\nslots = [{slots}]\n"),
                )
                .replace(
                    "[sections.right_leg]\narmor = 6",
                    "[sections.right_leg]\narmor = 0",
                )
                .replace("[sections.head]\narmor = 7", "[sections.head]\narmor = 3")
        };
        refused(
            &cargo("{ at = 5, item = \"Cargo\" }"),
            "A cargo bay of 1.5 tons fills 2 Cargo critical slots, one per ton rounded up; found 1",
        );
        assert_eq!(
            rejection(&cargo("{ at = \"5-6\", item = \"Cargo\" }")),
            None
        );
        // Installing the bay places its slots in the first free slots.
        let mut installed = MechTemplate::parse("JR7-D", &cargo("")).unwrap();
        installed.install_cargo_bay(3 * 512).unwrap();
        assert_eq!(check_template(&installed).rejection, None);
        installed.install_cargo_bay(0).unwrap();
        assert!(!installed.attributes.contains_key(crate::CARGO_TONS));
        assert!(installed.install_cargo_bay(100 * 1024).is_err());
        refused(
            &JENNER.replace(
                "[sections.left_leg]\narmor = 6\n",
                "[sections.left_leg]\narmor = 6\nslots = [{ at = 5, item = \"Cargo\" }]\n",
            ),
            "A cargo bay of 0 tons fills 0 Cargo critical slots",
        );
    }

    /// Only fusion and fission units mount standard flamers (TM p. 218).
    #[test]
    fn combustion_mechs_mount_no_standard_flamers() {
        let flamer = JENNER.replacen(
            "{ at = \"3-4\", item = \"IS.MediumLaser\" }",
            "{ at = 3, item = \"IS.Flamer\" }",
            1,
        );
        let combustion =
            flamer.replacen("[construction]\n", "[construction]\nengine = \"ice\"\n", 1);
        refused(&combustion, "IS.Flamer needs a fusion engine");
        assert!(
            !rejection(&flamer)
                .unwrap_or_default()
                .contains("fusion engine")
        );
    }

    /// Clan internal structure carries CASE in every location, so a Mech built on it installs
    /// no Case item, whatever its chassis base; Inner Sphere structure takes CASE as an item in
    /// a torso, on a mixed-technology Clan chassis too (TM p. 210).
    #[test]
    fn clan_case_follows_the_internal_structure() {
        let case = JENNER.replacen(
            "    { at = \"1-2\", item = \"JumpJet\" },\n]",
            "    { at = \"1-2\", item = \"JumpJet\" },\n    { at = 3, item = \"Case\" },\n]",
            1,
        );
        let mixed = |source: &str, structure: &str| {
            source.replacen(
                "[construction]\n",
                &format!("[construction]\nmixed_tech = true\nstructure_tech = \"{structure}\"\n"),
                1,
            )
        };
        let built_in = "A Mech with Clan internal structure has CASE built into every location";
        refused(&mixed(&case, "clan"), built_in);
        let clan_chassis = |source: &str| {
            source
                .replace(
                    "tech_base = \"inner_sphere\"",
                    "tech_base = \"clan\"\nheat_sinks = \"single\"",
                )
                .replace("IS.MediumLaser", "CL.ERSmallLaser")
                .replace("IS.SRM-4", "CL.SRM-4")
        };
        refused(&clan_chassis(&case), built_in);
        let inner_sphere_structure = mixed(&clan_chassis(&case), "inner_sphere");
        let installed = rejection(&inner_sphere_structure).unwrap_or_default();
        assert!(!installed.contains("CASE"), "{installed}");
        assert!(!rejection(&case).unwrap_or_default().contains("CASE"));
    }

    /// Weapons come from the chassis technology base unless the design is mixed technology.
    #[test]
    fn weapon_technology_base_matches_the_chassis() {
        let clan = JENNER.replace("IS.MediumLaser", "CL.ERSmallLaser");
        refused(
            &clan,
            "CL.ERSmallLaser is Clan technology on an Inner Sphere Mech",
        );
        let mixed = clan.replacen("[construction]\n", "[construction]\nmixed_tech = true\n", 1);
        // Lighter lasers leave the design two tons under its 35.
        refused(&mixed, "under the 35-ton chassis");
    }

    /// The engine, structure and armor come from the chassis technology base unless the design
    /// is mixed technology (TM p. 46), and so does equipment only one base builds.
    #[test]
    fn components_and_equipment_follow_the_chassis_technology_base() {
        let clan_engine = JENNER.replacen(
            "[construction]\n",
            "[construction]\nengine_tech = \"clan\"\n",
            1,
        );
        refused(&clan_engine, "The engine is not Inner Sphere technology");
        let mixed =
            clan_engine.replacen("[construction]\n", "[construction]\nmixed_tech = true\n", 1);
        assert_eq!(rejection(&mixed), None);
        refused(
            &JENNER.replacen("item = \"IS.MediumLaser\"", "item = \"LightTAG\"", 1),
            "LightTag is Clan technology on an Inner Sphere Mech",
        );
    }

    /// Compact, heavy-duty and XL gyros (TM p. 50) and the small cockpit (TM p. 52) are
    /// Inner Sphere chassis choices, so a Clan Mech uses them only as a mixed-technology design;
    /// composite structure is always Inner Sphere structure (Tactical Operations: Advanced
    /// Units & Equipment p. 154).
    #[test]
    fn inner_sphere_chassis_choices_need_mixed_technology_on_a_clan_mech() {
        let technology = |source: &str| {
            let template = MechTemplate::parse("JR7-D", source).unwrap();
            let loadout = crate::MechLoadout::resolve(&template).unwrap();
            template
                .validate_technology_base(&loadout)
                .err()
                .map(|error| error.to_string())
        };
        // The Jenner's head sink stays a single sink. A small cockpit fills head slots 1-4, so
        // that sink moves to slot 5.
        let clan = |choice: &str| {
            let source = JENNER.replace(
                "tech_base = \"inner_sphere\"",
                &format!("tech_base = \"clan\"\nheat_sinks = \"single\"\n{choice}"),
            );
            if !choice.starts_with("cockpit") {
                return source;
            }
            source.replace(
                "{ at = 4, item = \"HeatSink\" }",
                "{ at = 5, item = \"HeatSink\" }",
            )
        };
        let mixed = |source: String| {
            source.replacen("[construction]\n", "[construction]\nmixed_tech = true\n", 1)
        };
        for (choice, name, page) in [
            ("gyro = \"compact\"", "compact gyro", "TechManual p. 50"),
            (
                "gyro = \"heavy_duty\"",
                "heavy-duty gyro",
                "TechManual p. 50",
            ),
            ("cockpit = \"small\"", "small cockpit", "TechManual p. 52"),
        ] {
            let refusal = technology(&clan(choice)).unwrap_or_default();
            assert!(
                refusal.contains(&format!(
                    "The {name} is Inner Sphere technology on a Clan Mech"
                )) && refusal.contains(page),
                "{refusal}"
            );
            assert_eq!(technology(&mixed(clan(choice))), None, "{choice}");
        }
        let composite = clan("structure = \"composite\"");
        let refusal = technology(&composite).unwrap_or_default();
        assert!(
            refusal.contains("Composite structure is Inner Sphere technology"),
            "{refusal}"
        );
        assert!(technology(&mixed(composite.clone())).is_some());
        let inner_sphere_structure = mixed(composite.replace(
            "structure = \"composite\"",
            "structure = \"composite\"\nstructure_tech = \"inner_sphere\"",
        ));
        assert_eq!(technology(&inner_sphere_structure), None);
    }

    /// A slot may state the base of equipment both bases build, which then sizes it and must
    /// match the chassis unless the design is mixed technology.
    #[test]
    fn stated_technology_bases_size_shared_equipment() {
        let with = |item: &str, at: &str| {
            JENNER.replace(
                "    { at = \"1-2\", item = \"JumpJet\" },\n]",
                &format!(
                    "    {{ at = \"1-2\", item = \"JumpJet\" }},\n    {{ at = \"{at}\", item = \"{item}\" }},\n]"
                ),
            )
        };
        refused(
            &with("CL.Ecm", "3"),
            "CL.Ecm is Clan technology on an Inner Sphere Mech",
        );
        let mixed = |source: String| {
            source.replacen("[construction]\n", "[construction]\nmixed_tech = true\n", 1)
        };
        // Clan MASC fills 4% of the 35 tons, rounded: one slot (TM p. 52).
        let clan_masc = MechTemplate::parse("JR7-D", &mixed(with("CL.Masc", "3"))).unwrap();
        assert_eq!(clan_masc.masc_slots(), 1);
        assert!(clan_masc.installation_is_clan(crate::System::Masc));
        let inner_masc = MechTemplate::parse("JR7-D", &with("Masc", "3-4")).unwrap();
        assert_eq!(inner_masc.masc_slots(), 2);
        refused(
            &mixed(with("CL.TargetingComputer", "3-4")),
            "A Clan targeting computer for 4.0 tons of direct-fire weapons needs 1",
        );
    }

    /// Talons fill two slots in each leg, weighing a ton per fifteen tons of Mech; a location
    /// holds one spike cluster (Tactical Operations: Advanced Units & Equipment p. 100).
    #[test]
    fn talons_fill_every_leg_and_spikes_one_per_location() {
        let mixed = |source: String| {
            source.replacen("[construction]\n", "[construction]\nmixed_tech = true\n", 1)
        };
        let leg = |section: &str, slots: &str| {
            format!("[sections.{section}]\narmor = 6\nslots = [\n{slots}]\n")
        };
        let talons = "    { at = \"5-6\", item = \"Talons\" },\n";
        let one_leg =
            mixed(JENNER.replace("[sections.left_leg]\narmor = 6\n", &leg("left_leg", talons)));
        refused(
            &one_leg,
            "Talons fill two slots in every leg and nowhere else; Right_Leg has 0",
        );
        let both = one_leg.replace(
            "[sections.right_leg]\narmor = 6\n",
            &leg("right_leg", talons),
        );
        let template = MechTemplate::parse("JR7-D", &both).unwrap();
        let loadout = crate::MechLoadout::resolve(&template).unwrap();
        // A 35-ton Mech's talons weigh 3 tons in all.
        assert_eq!(
            template.construction_mass(&loadout).unwrap().total
                - MechTemplate::parse("JR7-D", JENNER)
                    .unwrap()
                    .construction_mass(
                        &crate::MechLoadout::resolve(
                            &MechTemplate::parse("JR7-D", JENNER).unwrap()
                        )
                        .unwrap()
                    )
                    .unwrap()
                    .total,
            3 * 1024
        );
        let error = rejection(&both.replace("mixed_tech = true\n", "")).unwrap_or_default();
        assert!(error.contains("Talons is Clan technology"), "{error}");
        refused(
            &JENNER.replace(
                "[sections.left_leg]\narmor = 6\n",
                &leg(
                    "left_leg",
                    "    { at = 5, item = \"Spikes\" },\n    { at = 6, item = \"Spikes\" },\n",
                ),
            ),
            "Left_Leg mounts more than one spike cluster",
        );
    }

    /// A machine gun array links two to four machine guns in its location (TechManual p. 228), and
    /// weighs half a ton, or a quarter ton built by the Clans.
    #[test]
    fn machine_gun_arrays_link_two_to_four_guns() {
        let torso = |slots: &str| {
            JENNER.replace(
                "slots = [\n    { at = 1, item = \"Ammo_IS.SRM-4\", rounds = 25 },\n    { at = \"2-3\", item = \"JumpJet\" },\n]",
                &format!("slots = [\n    {{ at = 1, item = \"Ammo_IS.SRM-4\", rounds = 25 }},\n    {{ at = \"2-3\", item = \"JumpJet\" }},\n{slots}]"),
            )
        };
        refused(
            &torso(
                "    { at = 4, item = \"IS.MachineGun\" },\n    { at = 5, item = \"IS.MGA\", links = [4] },\n",
            ),
            "links 1 slots",
        );
        let pair = torso(
            "    { at = \"4-5\", item = \"IS.MachineGun\" },\n    { at = 6, item = \"IS.MGA\", links = [4, 5] },\n",
        );
        // Everything else passes: two guns and the array add 2.5 tons to a full Jenner.
        refused(&pair, "over the 35-ton chassis");
        let template = MechTemplate::parse("JR7-D", &pair).unwrap();
        let loadout = crate::MechLoadout::resolve(&template).unwrap();
        let clan = MechTemplate::parse("JR7-D", &pair.replace("IS.MGA", "CL.MGA")).unwrap();
        let clan_loadout = crate::MechLoadout::resolve(&clan).unwrap();
        assert_eq!(
            template.construction_mass(&loadout).unwrap().total
                - clan.construction_mass(&clan_loadout).unwrap().total,
            256
        );
    }

    /// Only a biped arm's lower arm and hand actuators may be removed, the hand with the lower
    /// arm (TM p. 45).
    #[test]
    fn only_lower_arm_and_hand_actuators_may_be_omitted() {
        for omit in [
            "[\"shoulder\", \"upper_actuator\", \"lower_actuator\", \"hand_actuator\"]",
            "[\"upper_actuator\", \"lower_actuator\", \"hand_actuator\"]",
        ] {
            let source = JENNER.replacen(
                "omit = [\"lower_actuator\", \"hand_actuator\"]",
                &format!("omit = {omit}"),
                1,
            );
            let error = format!("{:#}", MechTemplate::parse("JR7-D", &source).unwrap_err());
            assert!(
                error.contains("may omit only its lower arm and hand"),
                "{error}"
            );
        }
        let lower_only = JENNER.replacen(
            "omit = [\"lower_actuator\", \"hand_actuator\"]",
            "omit = [\"lower_actuator\"]",
            1,
        );
        let error = format!(
            "{:#}",
            MechTemplate::parse("JR7-D", &lower_only).unwrap_err()
        );
        assert!(error.contains("must omit its hand actuator too"), "{error}");
        let leg = JENNER.replace(
            "[sections.left_leg]\narmor = 6\n",
            "[sections.left_leg]\narmor = 6\nomit = [\"hand_actuator\"]\n",
        );
        let error = format!("{:#}", MechTemplate::parse("JR7-D", &leg).unwrap_err());
        assert!(
            error.contains("Left_Leg keeps all its actuators"),
            "{error}"
        );
        // A section listing its slots explicitly is held to the same rule.
        let mut shoulderless = MechTemplate::parse("JR7-D", JENNER).unwrap();
        shoulderless
            .sections
            .get_mut(&crate::MechSection::LeftArm)
            .unwrap()
            .criticals
            .remove(&0);
        let rejection = check_template(&shoulderless).rejection.unwrap_or_default();
        assert!(
            rejection.contains("Left_Arm lacks its shoulder and upper arm actuators"),
            "{rejection}"
        );
    }

    /// Every component together weighs the tonnage (TM pp. 46, 57).
    #[test]
    fn total_mass_matches_the_tonnage() {
        refused(
            &JENNER.replace(
                "[sections.left_leg]\narmor = 6",
                "[sections.left_leg]\narmor = 15",
            ),
            "over the 35-ton chassis",
        );
        refused(
            &JENNER.replace(
                "    { at = 1, item = \"Ammo_IS.SRM-4\", rounds = 25 },\n",
                "",
            ),
            "under the 35-ton chassis",
        );
    }
}
