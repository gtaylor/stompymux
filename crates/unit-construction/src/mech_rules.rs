//! TechManual BattleMech construction rules applied to a template and its resolved loadout:
//! movement and engine rating, jump jets, heat sinks, structure and armor technology slots,
//! maximum armor, myomer and boosters, tonnage-dependent equipment, mounting restrictions,
//! technology base and total mass. Template checks, the template loader and the live
//! revalidation of a constructed Mech all run these rules, so a Mech that passes is one a
//! record sheet could describe.
use super::construction::{Component, MIXED_TECH, SPEED_PER_MP};
use super::document::ENGINE_RATING;
use super::engine::declared_engine_rating;
use super::tech_base::{TechBase, TechnologyInventory, check_technology_base};
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

/// Physical weapons: critical slots one item fills and the actuators its arm needs
/// (TM pp. 220, 237; Tactical Operations: Advanced Units & Equipment melee weapons).
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
        // Retractable blade: one slot plus one per twenty tons.
        System::RetractableBlade => (per(20) + 1, false, false, false),
        System::DualSaw => (7, false, false, false),
        System::WreckingBall => (5, false, false, false),
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

    /// Critical slots, and tons, of MASC: 5% of an Inner Sphere Mech's tonnage or 4% of a
    /// Clan Mech's, rounded to the nearest whole number (TM p. 52).
    pub fn masc_slots(&self) -> usize {
        let percent = if self.has_special("Clan") { 4 } else { 5 };
        usize::from((self.tons * percent + 50) / 100).max(1)
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
    /// the tonnage of the direct-fire weapons divided by 4 (Inner Sphere) or 5 (Clan),
    /// rounded up (TM p. 238).
    fn validate_targeting_computer(&self, loadout: &MechLoadout) -> Result<()> {
        let computer: Vec<_> = loadout
            .systems
            .iter()
            .filter(|part| part.system == System::TargetingComputer)
            .map(|part| part.location)
            .collect();
        if computer.is_empty() {
            return Ok(());
        }
        let weapons: u32 = loadout
            .weapons
            .iter()
            .filter(|mount| mount.weapon.is_direct_fire())
            .map(|mount| mount.weapon.mass())
            .sum();
        let size = |divisor: u32| weapons.div_ceil(1024 * divisor) as usize;
        let expected = if self.has_special("Clan") {
            size(5)
        } else {
            size(4)
        };
        // A mixed-technology Mech may carry either base's computer.
        let other = if self.has_special("Clan") {
            size(4)
        } else {
            size(5)
        };
        ensure!(
            (computer.len() == expected
                || (self.has_special(MIXED_TECH) && computer.len() == other))
                && contiguous(&computer),
            "A targeting computer for {:.1} tons of direct-fire weapons needs {expected} contiguous critical slots in one location; found {}",
            f64::from(weapons) / 1024.0,
            computer.len()
        );
        Ok(())
    }

    /// Physical weapons sit in biped arms with the actuators they need, one per location,
    /// each filling its tonnage-dependent slot count (TM pp. 220, 237; Tactical Operations:
    /// Advanced Units & Equipment melee weapons).
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
            ensure!(
                chassis == MechChassis::Biped
                    && matches!(section, MechSection::LeftArm | MechSection::RightArm),
                "Physical weapons may only be mounted in a biped's arms, not {name}"
            );
            let [(system, slots)] = items.as_slice() else {
                bail!("{name} mounts more than one physical weapon");
            };
            let weapon = physical_weapon(*system, self.tons).expect("filtered physical weapon");
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

    /// Artemis IV or V serves every applicable launcher or none, from the launcher's location
    /// (TM p. 206); Inner Sphere CASE goes in torsos (TM p. 210); rear-firing weapons sit in
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
        // Artemis IV fills one slot per launcher, Artemis V two (Tactical Operations: Advanced
        // Units & Equipment p. 217).
        let controller_slots = |section: Option<MechSection>| -> (usize, usize) {
            let count = |system| {
                loadout
                    .systems
                    .iter()
                    .filter(|part| {
                        part.system == system && section.is_none_or(|s| part.location.section == s)
                    })
                    .count()
            };
            (count(System::ArtemisIv), count(System::ArtemisV))
        };
        if controller_slots(None) != (0, 0) {
            for section in MechSection::ALL {
                let (four, five) = controller_slots(Some(section));
                let name = chassis.section_name(section);
                ensure!(
                    five.is_multiple_of(2),
                    "Artemis V fills two slots per launcher; {name} has {five}"
                );
                let controllers = four + five / 2;
                let launchers = loadout
                    .weapons
                    .iter()
                    .filter(|mount| {
                        mount.weapon.is_artemis_launcher() && mount.criticals[0].section == section
                    })
                    .count();
                ensure!(
                    controllers == launchers,
                    "Artemis IV must serve every LRM, SRM and MML launcher from its location: {name} has {launchers} launchers and {controllers} Artemis systems"
                );
            }
        }
        if !self.has_special("Clan") {
            ensure!(
                loadout
                    .systems
                    .iter()
                    .all(|part| part.system != System::Case || torso(part.location.section)),
                "Inner Sphere CASE may only be mounted in torso locations"
            );
        }
        for mount in &loadout.weapons {
            let primary = mount.criticals[0].section;
            let name = mount.weapon.name();
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
    /// equipment.
    fn validate_technology_base(&self, loadout: &MechLoadout) -> Result<()> {
        let clan = self.has_special("Clan");
        let mixed_tech = self.has_special(MIXED_TECH);
        ensure!(
            clan || mixed_tech || !self.has_technology(super::Technology::Watchdog),
            "The Watchdog suite is Clan technology on an Inner Sphere Mech; declare mixed_tech in [construction]"
        );
        check_technology_base(TechnologyInventory {
            unit: "Mech",
            base: TechBase::of(clan),
            mixed_tech,
            other_base_components: Component::ALL
                .into_iter()
                .filter(|component| self.has_special(component.flag(!clan)))
                .collect(),
            weapons: loadout.weapons.iter().map(|mount| mount.weapon),
            systems: loadout.systems.iter().map(|part| part.system),
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
