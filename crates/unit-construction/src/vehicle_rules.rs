//! Combat vehicle and VTOL construction rules from TechManual pp. 95-107: motive-type tonnage
//! limits, structure and engine choices, heat sinks, the Maximum Armor Factor, item slots and
//! the tonnage budget. Stationary installations are not combat vehicles and only pass anatomy
//! and mass arithmetic.
use super::construction::{Component, MIXED_TECH};
use super::tech_base::{TechBase, TechnologyInventory, check_technology_base};
use super::{
    AmmunitionMode, Engine, System, Technology, VehicleLoadout, VehicleMass, VehicleMovement,
    VehiclePowerplant, VehicleSection, VehicleTemplate, Weapon,
    vehicle_engine::VEHICLE_MAXIMUM_ENGINE_RATING,
};
use anyhow::{Result, bail, ensure};
use serde::Serialize;

/// Most armor points a VTOL rotor may carry (TechManual p. 105).
pub const ROTOR_MAXIMUM_ARMOR: u16 = 2;

/// Heat vehicular stealth armor generates while active, which a combat vehicle's heat sinks
/// must cover (Tactical Operations: Advanced Units & Equipment p. 92, Advanced Armor Table).
pub const STEALTH_ARMOR_HEAT: u32 = 10;

/// What a combat vehicle's construction spends against its limits, in 1/1024-ton units for
/// mass. Only a vehicle that obeys every rule produces one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleConstruction {
    /// Component masses with full ammunition bins.
    pub mass: VehicleMass,
    /// Item slots the weapons, equipment, ammunition and cargo occupy.
    pub item_slots: u16,
    /// Item slots the chassis offers after engine and armor adjustments.
    pub item_slot_limit: u16,
    /// Armor points on every location.
    pub armor: u32,
    /// Maximum Armor Factor for the tonnage.
    pub maximum_armor: u32,
    /// Heat of every energy weapon firing at once, which the heat sinks must match.
    pub energy_heat: u32,
    /// Heat of active stealth armor, which the heat sinks must cover besides the energy
    /// weapons; zero without stealth armor.
    pub stealth_heat: u32,
    /// Legal but notable facts, such as unspent tonnage.
    pub warnings: Vec<String>,
}

impl VehicleMovement {
    /// Heaviest combat vehicle of this motive type (TechManual p. 96, Combat Vehicle Motive
    /// Types Table). Stationary installations are not combat vehicles and have no limit.
    pub fn maximum_tonnage(self) -> Option<u16> {
        match self {
            Self::Hover => Some(50),
            Self::Vtol => Some(30),
            Self::Wheeled => Some(80),
            Self::Tracked => Some(100),
            Self::Stationary => None,
        }
    }
}

/// Maximum Armor Factor: 3.5 times the tonnage plus 40, rounded down (TechManual p. 105).
pub fn vehicle_maximum_armor(tons: u16) -> u32 {
    u32::from(tons) * 7 / 2 + 40
}

/// Whether a resolved system claims an item slot. `clan_structure` is the vehicle's internal
/// structure base. A combat vehicle counts an Artemis or MRM "Apollo" fire-control system as
/// part of the launcher it serves (TechManual p. 206; Tactical Operations: Advanced Units &
/// Equipment p. 142); auxiliary VTOL fuel tanks are fuel, not items. CASE costs one item slot
/// on Inner Sphere structure, while Clan structure builds it in for none (TechManual p. 210).
pub fn occupies_vehicle_slot(system: System, clan_structure: bool) -> bool {
    match system {
        System::ArtemisIv | System::ArtemisV | System::ApolloFcs | System::FuelTank => false,
        System::Case => !clan_structure,
        _ => true,
    }
}

/// Why a combat vehicle may not mount `system` at all, or `None` when it may. BattleMech
/// critical-slot entries for actuators, the engine, gyro, cockpit, life support, sensors,
/// heat sinks and the slot-filling structure and armor types have no vehicle counterpart: a
/// vehicle records its engine, heat sinks, structure and armor in the template (TechManual
/// pp. 99-105), and its cargo bay in `cargo_tons` (TechManual p. 239). Jump jets, MASC and
/// triple-strength myomer are Mech systems (TechManual
/// pp. 102-103); CASE II and the null-signature system are not available to combat vehicles
/// (Tactical Operations: Advanced Units & Equipment pp. 111, 148); and the hatchet, sword,
/// retractable blade, claws, flail, lance, mace, chain whip, vibroblades, talons and spikes
/// are BattleMech melee weapons (TechManual pp. 220, 236-237; ibid. pp. 100-104).
pub fn vehicle_refusal(system: System) -> Option<&'static str> {
    use System::*;
    Some(match system {
        ShoulderOrHip | UpperActuator | LowerActuator | HandOrFootActuator | Engine | Gyro
        | Cockpit | LifeSupport | Sensors | HeatSink | FerroFibrous | EndoSteel
        | HeavyFerroFibrous | LightFerroFibrous | StealthArmor | LaserReflective => {
            "is a BattleMech critical-slot entry; a combat vehicle states its engine, heat sinks, structure and armor in the template (TechManual pp. 99-105)"
        }
        Cargo => {
            "is a BattleMech cargo-bay slot; a combat vehicle's cargo bay is its cargo_tons alone, in one item slot (TechManual p. 239)"
        }
        JumpJet => "is a jump jet; combat vehicles cannot jump (TechManual p. 102)",
        Masc => "is MASC, which combat vehicles cannot mount (TechManual p. 103)",
        TripleStrengthMyomer => {
            "is triple-strength myomer, which combat vehicles cannot use (TechManual p. 103)"
        }
        CaseIi => {
            "is CASE II, available only to BattleMechs, IndustrialMechs and fighters (Tactical Operations: Advanced Units & Equipment p. 111)"
        }
        NullSignature => {
            "is the null-signature system, available only to BattleMechs (Tactical Operations: Advanced Units & Equipment p. 148)"
        }
        Axe | Sword | RetractableBlade => {
            "is a melee weapon only humanoid BattleMechs and IndustrialMechs carry (TechManual pp. 220, 236-237)"
        }
        Claw | Flail | Lance | Mace | ChainWhip | SmallVibroblade | MediumVibroblade
        | LargeVibroblade | Talons | Spikes => {
            "is a melee weapon only BattleMechs and IndustrialMechs carry (Tactical Operations: Advanced Units & Equipment pp. 100-104)"
        }
        _ => return None,
    })
}

impl VehicleTemplate {
    /// Item slots the chassis offers: 5 plus 1 per 5 full tons (TechManual p. 98), less the
    /// slots an XL or light engine claims and plus the one a compact engine frees (TechManual
    /// p. 101, Combat Vehicle Engine Space Table), less the 4 an Inner Sphere or 2 a Clan XXL
    /// engine claims (Tactical Operations: Advanced Units & Equipment p. 120, Advanced Engine
    /// Master Table), less the slots ferro-fibrous armor claims
    /// (TechManual p. 105, Combat Vehicle Armor Table) and the slots the advanced armors claim
    /// in the CV column of the Advanced Armor Table: 1 for hardened and for Inner Sphere or Clan
    /// laser-reflective armor, 2 for vehicular stealth armor (Tactical Operations: Advanced
    /// Units & Equipment p. 92).
    pub fn item_slot_limit(&self) -> u16 {
        let base = 5 + i32::from(self.tons) / 5;
        let engine = match self.engine_family() {
            Some(Engine::Xl) if self.clan_engine() => -1,
            Some(Engine::Xl) => -2,
            Some(Engine::Xxl) if self.clan_engine() => -2,
            Some(Engine::Xxl) => -4,
            Some(Engine::Light) => -1,
            Some(Engine::Compact) => 1,
            _ => 0,
        };
        let armor = if self.has_special("FerroFibrous_Tech") {
            if self.clan_armor() { -1 } else { -2 }
        } else if self.has_special("HvyFerroFibrous_Tech") {
            -3
        } else if self.has_special("LtFerroFibrous_Tech")
            || self.has_technology(Technology::HardenedArmor)
            || self.has_technology(Technology::LaserReflectiveArmor)
        {
            -1
        } else if self.has_special("StealthArmor_Tech") {
            -2
        } else {
            0
        };
        u16::try_from((base + engine + armor).max(0)).unwrap_or(0)
    }

    /// Item slots the loadout occupies: one per weapon and per equipment item, one per
    /// ammunition type however many tons it holds, one for the standard cargo bay that
    /// `cargo_tons` describes (TechManual pp. 98, 106), one per other type of transport bay and
    /// one per cargo container (TechManual pp. 107, 239). Heat sinks and power amplifiers take
    /// none (TechManual pp. 104, 106).
    pub fn item_slots(&self, loadout: &VehicleLoadout) -> u16 {
        let weapons = loadout.weapons.len();
        let clan_structure = self.clan_structure();
        let systems = loadout
            .systems
            .iter()
            .filter(|part| occupies_vehicle_slot(part.system, clan_structure))
            .count();
        let mut ammunition: Vec<(Weapon, AmmunitionMode)> = Vec::new();
        for bin in &loadout.ammunition {
            if !ammunition.contains(&(bin.weapon, bin.mode)) {
                ammunition.push((bin.weapon, bin.mode));
            }
        }
        let cargo = super::cargo_mass(self.attributes.get(super::CARGO_TONS).map(String::as_str))
            .is_ok_and(|mass| mass > 0);
        let bays = self
            .transport_bays()
            .map_or(0, |bays| bays.item_slots() as usize);
        let total = weapons + systems + ammunition.len() + usize::from(cargo) + bays;
        u16::try_from(total).unwrap_or(u16::MAX)
    }

    /// Heat of every energy weapon firing at once that needs heat sinks: the fewest heat sinks
    /// the vehicle may carry (TechManual p. 104).
    pub fn energy_heat(loadout: &VehicleLoadout) -> u32 {
        loadout
            .weapons
            .iter()
            .filter(|mount| Self::needs_heat_sinks(mount.weapon))
            .map(|mount| u32::from(mount.weapon.profile().heat))
            .sum()
    }

    /// Whether a weapon's heat needs heat sinks on a combat vehicle: every energy weapon but
    /// the vehicle flamer, which burns chemical fuel and needs none, though it counts as an
    /// energy weapon in play (TechManual pp. 104, 218), and the laser anti-missile system,
    /// which counts as an energy weapon for a unit without a heat scale (Tactical
    /// Operations: Advanced Units & Equipment p. 131).
    pub fn needs_heat_sinks(weapon: Weapon) -> bool {
        (weapon.is_energy() && weapon != Weapon::VehicleFlamer)
            || matches!(weapon, Weapon::LaserAms | Weapon::ClanLaserAms)
    }

    /// The fusion engine family, or `None` for an internal combustion engine.
    fn engine_family(&self) -> Option<Engine> {
        match self.engine().ok()?.powerplant {
            VehiclePowerplant::Combustion => None,
            VehiclePowerplant::Fusion(family) => Some(family),
        }
    }

    /// Check the template against the TechManual combat vehicle construction rules and report
    /// what it spends. Overweight designs are refused (TechManual p. 97); unspent tonnage is
    /// cargo space or wasted weight (TechManual pp. 97, 106) and is only a warning.
    pub fn construction_rules(&self) -> Result<VehicleConstruction> {
        let mass = self.mass()?;
        self.transport_bays()?;
        let loadout = VehicleLoadout::resolve(self)?;
        let armor: u32 = self
            .sections
            .values()
            .filter(|section| section.internal > 0)
            .map(|section| u32::from(section.armor) + u32::from(section.rear))
            .sum();
        let mut report = VehicleConstruction {
            mass,
            item_slots: self.item_slots(&loadout),
            item_slot_limit: self.item_slot_limit(),
            armor,
            maximum_armor: vehicle_maximum_armor(self.tons),
            energy_heat: Self::energy_heat(&loadout),
            stealth_heat: if self.has_special("StealthArmor_Tech") {
                STEALTH_ARMOR_HEAT
            } else {
                0
            },
            warnings: Vec::new(),
        };
        if self.movement == VehicleMovement::Stationary {
            return Ok(report);
        }
        self.check_chassis()?;
        self.check_clan_availability()?;
        self.check_advanced_armor(&loadout)?;
        self.check_equipment(&loadout)?;
        ensure!(
            report.armor <= report.maximum_armor,
            "{} armor points exceed the Maximum Armor Factor of {} for {} tons (TechManual p. 105)",
            report.armor,
            report.maximum_armor,
            self.tons
        );
        ensure!(
            report.item_slots <= report.item_slot_limit,
            "{} items exceed the {} item slots of this chassis (TechManual pp. 98, 101, 105)",
            report.item_slots,
            report.item_slot_limit
        );
        let sinks = u32::from(self.heat_sink_capacity());
        ensure!(
            sinks >= report.energy_heat,
            "{sinks} heat sinks cannot vent the {} heat of the energy weapons (TechManual p. 104)",
            report.energy_heat
        );
        ensure!(
            sinks >= report.energy_heat + report.stealth_heat,
            "{sinks} heat sinks cannot vent the {} heat of the energy weapons and the {} of the stealth armor (Tactical Operations: Advanced Units & Equipment p. 92)",
            report.energy_heat,
            report.stealth_heat
        );
        let tonnage = u32::from(self.tons) * 1024;
        ensure!(
            mass.design_total <= tonnage,
            "Components weigh {} tons, more than the vehicle's {} tons (TechManual p. 97)",
            format_tons(mass.design_total),
            self.tons
        );
        if mass.design_total < tonnage {
            report.warnings.push(format!(
                "{} tons unspent: cargo space or wasted weight (TechManual p. 97)",
                format_tons(tonnage - mass.design_total)
            ));
        }
        Ok(report)
    }

    /// Motive type, tonnage, structure, engine, heat sink and armor choices.
    fn check_chassis(&self) -> Result<()> {
        if let Some(maximum) = self.movement.maximum_tonnage() {
            ensure!(
                self.tons <= maximum,
                "{:?} combat vehicles weigh at most {maximum} tons (TechManual p. 96)",
                self.movement
            );
        }
        ensure!(
            !self.has_special("EndoSteel_Tech")
                && !self.has_technology(Technology::CompositeStructure)
                && !self.has_technology(Technology::ReinforcedStructure),
            "Combat vehicles use standard internal structure only (TechManual p. 98)"
        );
        let engine = self.engine()?;
        if let Some(declared) = self.declared_engine_rating()? {
            ensure!(
                i64::from(declared) == i64::from(engine.weight_rating),
                "The declared engine rating {declared} is not the Final Engine Rating {}: {} tons x cruising MP {} less suspension factor {}, rounded up to a multiple of 5 and at least 10 (TechManual p. 100)",
                engine.weight_rating,
                self.tons,
                self.walk_mp,
                engine.suspension
            );
        }
        ensure!(
            engine.weight_rating <= VEHICLE_MAXIMUM_ENGINE_RATING,
            "Engine Rating {} exceeds the combat vehicle maximum of {VEHICLE_MAXIMUM_ENGINE_RATING} (TechManual p. 100)",
            engine.weight_rating
        );
        ensure!(
            !self.has_special("DoubleHS") && !self.has_technology(Technology::LaserHeatSinks),
            "Combat vehicles use single heat sinks only (TechManual p. 104)"
        );
        let jump = self
            .attributes
            .get("jump_speed")
            .and_then(|speed| speed.parse::<f64>().ok())
            .unwrap_or(0.0);
        ensure!(
            jump <= 0.0,
            "Combat vehicles cannot jump (TechManual p. 102)"
        );
        ensure!(
            !self.has_special("TripleMyomerTech"),
            "Combat vehicles cannot use triple-strength myomer (TechManual p. 103)"
        );
        for (&section, layout) in &self.sections {
            ensure!(
                layout.rear == 0,
                "{} carries rear armor; combat vehicle locations have a single armor value (TechManual p. 105)",
                section.name()
            );
        }
        if let Some(rotor) = self.sections.get(&VehicleSection::Rotor) {
            ensure!(
                rotor.armor <= ROTOR_MAXIMUM_ARMOR,
                "The rotor carries {} armor points, more than {ROTOR_MAXIMUM_ARMOR} (TechManual p. 105)",
                rotor.armor
            );
        }
        Ok(())
    }

    /// Engine and armor types the Clans do not build. A Clan-made engine is a standard or
    /// Clan XL fusion engine or an ICE, never a light or compact fusion engine (TechManual
    /// p. 100, Combat Vehicle Engine Space Table); Clan-made armor is standard or Clan
    /// ferro-fibrous, never light or heavy ferro-fibrous (TechManual p. 105). An Inner Sphere XL
    /// engine on a Clan vehicle is an engine of the other technology base, which only a
    /// `mixed_tech` design may build (TechManual p. 46). A mixed-technology design picks a
    /// component's base with the `engine_tech` and `armor_tech` keys, so the types checked here
    /// still follow the base each component is built from.
    fn check_clan_availability(&self) -> Result<()> {
        if self.clan_engine() {
            let family = self.engine_family();
            ensure!(
                !matches!(family, Some(Engine::Light | Engine::Compact)),
                "Clan combat vehicles may not use {} fusion engines; Clan engines are ICE, standard or Clan XL fusion (TechManual p. 100)",
                if family == Some(Engine::Light) {
                    "light"
                } else {
                    "compact"
                }
            );
        }
        if self.clan_armor() {
            for (flag, armor) in [
                ("LtFerroFibrous_Tech", "light ferro-fibrous"),
                ("HvyFerroFibrous_Tech", "heavy ferro-fibrous"),
            ] {
                ensure!(
                    !self.has_special(flag),
                    "Clan combat vehicles may not use {armor} armor; Clan vehicles use standard or Clan ferro-fibrous armor (TechManual p. 105)"
                );
            }
        }
        Ok(())
    }

    /// The advanced armors of the Advanced Armor Table (Tactical Operations: Advanced Units &
    /// Equipment p. 92) that combat vehicles may carry. Template validation already keeps
    /// hardened armor off VTOL and hover vehicles. Vehicular stealth armor is Inner Sphere technology and needs a
    /// working ECM suite of any type (Tactical Operations: Advanced Units & Equipment p. 93);
    /// its heat is checked with the heat sinks. Laser-reflective armor has no vehicle
    /// restriction. The table's ferro-lamellor, reactive and modular armors have no stompymux
    /// armor type.
    fn check_advanced_armor(&self, loadout: &VehicleLoadout) -> Result<()> {
        if self.has_special("StealthArmor_Tech") {
            ensure!(
                !self.clan_armor(),
                "Vehicular stealth armor is Inner Sphere technology (Tactical Operations: Advanced Units & Equipment p. 93)"
            );
            ensure!(
                loadout
                    .systems
                    .iter()
                    .any(|part| matches!(part.system, System::Ecm | System::AngelEcm)),
                "Vehicular stealth armor needs an ECM suite (Tactical Operations: Advanced Units & Equipment p. 92)"
            );
        }
        Ok(())
    }

    /// Equipment the motive type or location forbids, and equipment of the other technology
    /// base on a vehicle that does not declare mixed technology, judged by the base each slot
    /// states (TechManual p. 46). A VTOL's turret is a chin turret, which follows the standard
    /// turret rules (TacOps AUE p. 160).
    fn check_equipment(&self, loadout: &VehicleLoadout) -> Result<()> {
        let clan = self.has_special("Clan");
        check_technology_base(TechnologyInventory {
            unit: "vehicle",
            base: TechBase::of(clan),
            mixed_tech: self.has_special(MIXED_TECH),
            other_base_components: Component::ALL
                .into_iter()
                .filter(|component| self.has_special(component.flag(!clan)))
                .collect(),
            chassis_technologies: Vec::new(),
            weapons: loadout.weapons.iter().map(|mount| mount.weapon),
            systems: loadout
                .systems
                .iter()
                .map(|part| (part.system, part.tech_base)),
        })?;
        let rotor = loadout
            .weapons
            .iter()
            .map(|mount| mount.criticals[0].section)
            .chain(loadout.systems.iter().map(|part| part.location.section))
            .chain(loadout.ammunition.iter().map(|bin| bin.location.section))
            .any(|section| section == VehicleSection::Rotor);
        ensure!(
            !rotor,
            "No equipment may be mounted in the rotor (TechManual pp. 98, 106)"
        );
        // Each Artemis or Apollo entry links the first slot of the launcher it guides in its
        // own location; every vehicle entry guides its own launcher, Artemis V included.
        super::fire_control::check_fire_control_links(
            loadout,
            &self.sections,
            VehicleSection::name,
            1,
        )?;
        super::machine_gun_array::check_machine_gun_arrays(loadout, VehicleSection::name)?;
        loadout.check_machine_gun_bins(
            |location| format!("{} slot {}", location.section.name(), location.slot + 1),
            "TechManual p. 107",
        )?;
        let computers = loadout
            .systems
            .iter()
            .filter(|part| part.system == System::TargetingComputer)
            .count();
        ensure!(
            computers <= 1,
            "A combat vehicle mounts one targeting computer, a single item sized by its direct-fire weapons; found {computers} entries (TechManual p. 238)"
        );
        // Only fusion and fission units mount standard flamers; a combustion engine takes
        // vehicle flamers (TechManual p. 218).
        if self.engine()?.powerplant == VehiclePowerplant::Combustion {
            for mount in &loadout.weapons {
                ensure!(
                    !matches!(mount.weapon, Weapon::Flamer | Weapon::ClanFlamer),
                    "{} needs a fusion engine; an internal combustion vehicle mounts vehicle flamers instead (TechManual p. 218)",
                    mount.weapon.name()
                );
            }
        }
        let vtol = self.is_vtol();
        let mut physical = Vec::new();
        let mut hoists = 0;
        let mut communications = 0;
        for part in &loadout.systems {
            let section = part.location.section;
            let here = format!(
                "{:?} in {} slot {}",
                part.system,
                section.name(),
                u16::from(part.location.slot) + 1
            );
            if let Some(reason) = vehicle_refusal(part.system) {
                bail!("{here} {reason}");
            }
            match part.system {
                // Ground vehicles mount one dual saw in each of the front, rear and turret
                // locations (TechManual p. 243).
                System::DualSaw => ensure!(
                    !vtol
                        && matches!(
                            section,
                            VehicleSection::Front | VehicleSection::Rear | VehicleSection::Turret
                        ),
                    "{here}: a combat vehicle mounts a dual saw only in its front, rear or turret, and a VTOL none (TechManual p. 243)"
                ),
                // A ground vehicle mounts a wrecking ball only in its turret (TechManual p. 249).
                System::WreckingBall => ensure!(
                    !vtol && section == VehicleSection::Turret,
                    "{here}: a combat vehicle mounts a wrecking ball only in its turret, and a VTOL none (TechManual p. 249)"
                ),
                // Ground vehicles mount one backhoe in each of the front, rear and turret
                // (TechManual p. 241).
                System::Backhoe => ensure!(
                    !vtol
                        && matches!(
                            section,
                            VehicleSection::Front | VehicleSection::Rear | VehicleSection::Turret
                        ),
                    "{here}: a combat vehicle mounts a backhoe only in its front, rear or turret, and a VTOL none (TechManual p. 241)"
                ),
                System::LiftHoist => hoists += 1,
                System::CommunicationsEquipment => {
                    communications += 1;
                    let tons = self.communications_tons(part).unwrap_or(0);
                    ensure!(
                        (1..=15).contains(&tons),
                        "{here}: communications equipment is a set of 1 to 15 whole tons stated with tons, not {tons} (TechManual p. 212)"
                    );
                }
                _ => {}
            }
            // A location holds one of a vehicle's backhoe, dual saw and wrecking ball
            // (TechManual pp. 241, 243, 249).
            if matches!(
                part.system,
                System::Backhoe | System::DualSaw | System::WreckingBall
            ) {
                ensure!(
                    !physical.contains(&section),
                    "{here}: a location holds one backhoe, dual saw or wrecking ball (TechManual pp. 241, 243, 249)"
                );
                physical.push(section);
            }
        }
        ensure!(
            hoists <= 4,
            "A combat vehicle mounts at most four lift hoists; found {hoists} (TechManual p. 245)"
        );
        ensure!(
            communications <= 1,
            "A unit adds one set of communications equipment, a single vehicle item; found {communications} (TechManual p. 212)"
        );
        Ok(())
    }
}

/// Read-only construction verdict for a vehicle template, the vehicle counterpart of
/// [`super::TemplateCheck`]: whether the game would construct it, why not, and what it spends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleTemplateCheck {
    pub name: String,
    pub reference: String,
    pub movement: VehicleMovement,
    pub vtol: bool,
    pub constructible: bool,
    pub rejection: Option<String>,
    /// Cruising MP the template states (TechManual p. 100).
    pub cruising_mp: u32,
    /// Flanking MP: cruising MP x 1.5, rounded up (TechManual p. 100).
    pub flanking_mp: u32,
    /// Final Engine Rating: tonnage x cruising MP less the suspension factor, rounded up to a
    /// multiple of 5 and at least 10 (TechManual p. 100); zero for a stationary installation.
    pub engine_rating: i32,
    /// The engine rating the template's `[construction]` table declares, which construction
    /// requires to equal `engine_rating`; absent when the template derives it.
    pub declared_engine_rating: Option<u32>,
    pub weapons: usize,
    pub ammunition_bins: usize,
    /// Spending against the construction limits, present when the vehicle is constructible.
    pub construction: Option<VehicleConstruction>,
}

/// Validate a parsed vehicle template as construction would, without changing it.
pub fn check_vehicle_template(template: &VehicleTemplate) -> VehicleTemplateCheck {
    let mut report = VehicleTemplateCheck {
        name: template.name.clone(),
        reference: template.reference.clone(),
        movement: template.movement,
        vtol: template.is_vtol(),
        constructible: false,
        rejection: None,
        cruising_mp: template.walk_mp,
        flanking_mp: super::running_mp_of_walking(template.walk_mp),
        engine_rating: template.engine().map_or(0, |engine| engine.weight_rating),
        declared_engine_rating: template.declared_engine_rating().ok().flatten(),
        weapons: 0,
        ammunition_bins: 0,
        construction: None,
    };
    let checked = template
        .validate_construction()
        .and_then(|()| VehicleLoadout::resolve(template))
        .and_then(|loadout| Ok((loadout, template.construction_rules()?)));
    let (loadout, construction) = match checked {
        Ok(checked) => checked,
        Err(error) => {
            report.rejection = Some(format!("{error:#}"));
            return report;
        }
    };
    report.constructible = true;
    report.weapons = loadout.weapons.len();
    report.ammunition_bins = loadout.ammunition.len();
    report.construction = Some(construction);
    report
}

/// Render a 1/1024-ton mass as tons for messages.
pub fn format_tons(mass: u32) -> String {
    let tons = f64::from(mass) / 1024.0;
    format!("{tons:.2}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nathaniel's Yellow Jacket from the TechManual worked example (pp. 96-107): a 30-ton
    /// ICE VTOL with a Gauss rifle and two tons of ammunition.
    const YELLOW_JACKET: &str = r#"
name = "Yellow Jacket"
class = "vtol"
movement = "vtol"
tons = 30
walk_mp = 6

[construction]
engine = "ice"

[sections.left_side]
armor = 13

[sections.right_side]
armor = 13

[sections.front_side]
armor = 20
slots = [
    { at = 1, item = "IS.GaussRifle" },
    { at = "2-3", item = "Ammo_IS.GaussRifle", rounds = 8 },
]

[sections.aft_side]
armor = 8

[sections.rotor]
armor = 2
"#;

    /// Patrick's Zhukov from the same worked example: a 75-ton ICE tank with two turreted
    /// AC/10s and a front SRM-6.
    const ZHUKOV: &str = r#"
name = "Zhukov"
class = "vehicle"
movement = "track"
tons = 75
walk_mp = 3

[construction]
engine = "ice"

[sections.left_side]
armor = 32

[sections.right_side]
armor = 32

[sections.front_side]
armor = 44
slots = [
    { at = 1, item = "IS.SRM-6" },
    { at = 2, item = "Ammo_IS.SRM-6", rounds = 15 },
]

[sections.aft_side]
armor = 24

[sections.turret]
armor = 44
slots = [
    { at = "1-2", item = "IS.AC/10" },
    { at = "3-4", item = "Ammo_IS.AC/10", rounds = 10 },
]
"#;

    fn parse(source: &str) -> VehicleTemplate {
        VehicleTemplate::parse("test", source).unwrap()
    }

    fn rejection(template: &VehicleTemplate) -> String {
        format!("{:#}", template.construction_rules().unwrap_err())
    }

    /// The worked examples weigh exactly their tonnage under the TechManual arithmetic.
    #[test]
    fn worked_examples_construct_at_their_tonnage() {
        let jacket = parse(YELLOW_JACKET);
        let report = jacket.construction_rules().unwrap();
        // ICE 40 (2 t), structure 3, rotor 3, controls 1.5, armor 3.5, Gauss 15, ammo 2.
        assert_eq!(jacket.engine().unwrap().weight_rating, 40);
        assert_eq!(report.mass.engine, 2 * 1024);
        assert_eq!(report.mass.cockpit, 3 * 512);
        assert_eq!(report.mass.design_total, 30 * 1024);
        assert_eq!((report.item_slots, report.item_slot_limit), (2, 11));
        assert_eq!((report.armor, report.maximum_armor), (56, 145));
        assert!(report.warnings.is_empty());
        let zhukov = parse(ZHUKOV);
        let report = zhukov.construction_rules().unwrap();
        // ICE 225 (20 t), structure 7.5, controls 4, armor 11, turret 2.5, weapons 27, ammo 3.
        assert_eq!(report.mass.engine, 20 * 1024);
        assert_eq!(report.mass.cockpit, 4 * 1024);
        assert_eq!(report.mass.turret, 5 * 512);
        assert_eq!(report.mass.design_total, 75 * 1024);
        // Two AC/10s and an SRM-6, plus one slot per ammunition type.
        assert_eq!((report.item_slots, report.item_slot_limit), (5, 20));
        assert_eq!(zhukov.sections[&VehicleSection::Turret].internal, 8);
        assert!(check_vehicle_template(&zhukov).constructible);
    }

    /// A vehicle targeting computer is one item in one slot that weighs the direct-fire weapon
    /// tonnage over 4 (Inner Sphere) or 5 (Clan), rounded up; missiles do not count, and a
    /// second entry is refused (TechManual p. 238).
    #[test]
    fn targeting_computer_is_one_item_sized_by_its_weapons() {
        let zhukov = parse(ZHUKOV);
        let bare = zhukov.construction_rules().unwrap();
        let computer = parse(&ZHUKOV.replace(
            "[sections.aft_side]\narmor = 24\n",
            "[sections.aft_side]\narmor = 24\nslots = [\n    { at = 1, item = \"TargetingComputer\" },\n]\n",
        ));
        let loadout = VehicleLoadout::resolve(&computer).unwrap();
        let part = &loadout.systems[0];
        // Two AC/10s weigh 24 tons; the SRM-6 is a missile launcher and does not count.
        assert_eq!(computer.system_mass(&loadout, part), 6 * 1024);
        let mass = computer.mass().unwrap();
        assert_eq!(mass.equipment, bare.mass.equipment + 6 * 1024);
        assert_eq!(computer.item_slots(&loadout), bare.item_slots + 1);
        let weapons = loadout.weapons.iter().map(|mount| mount.weapon);
        assert_eq!(
            super::super::targeting_computer_tons(weapons.clone(), false),
            6
        );
        assert_eq!(super::super::targeting_computer_tons(weapons, true), 5);
        let doubled = parse(&ZHUKOV.replace(
            "[sections.aft_side]\narmor = 24\n",
            "[sections.aft_side]\narmor = 24\nslots = [\n    { at = \"1-2\", item = \"TargetingComputer\" },\n]\n",
        ));
        assert!(
            rejection(&doubled).contains("mounts one targeting computer"),
            "{}",
            rejection(&doubled)
        );
    }

    /// A vehicle location lists more than twelve items, and an array listed past the twelfth
    /// links the guns its `links` name (TechManual p. 228).
    #[test]
    fn machine_gun_arrays_link_guns_past_the_twelfth_item() {
        let zhukov = parse(&ZHUKOV.replace(
            "    { at = \"3-4\", item = \"Ammo_IS.AC/10\", rounds = 10 },\n",
            "    { at = \"3-4\", item = \"Ammo_IS.AC/10\", rounds = 10 },\n    { at = \"13-14\", item = \"IS.MachineGun\" },\n    { at = 15, item = \"IS.MGA\", links = [13, 14] },\n",
        ));
        let loadout = VehicleLoadout::resolve(&zhukov).unwrap();
        let arrays = loadout.machine_gun_arrays();
        assert_eq!(arrays.len(), 1);
        let slots: Vec<_> = arrays[0]
            .weapon_indices
            .iter()
            .map(|&index| loadout.weapons[index].criticals[0].slot)
            .collect();
        assert_eq!(slots, [12, 13]);
        super::super::machine_gun_array::check_machine_gun_arrays(&loadout, VehicleSection::name)
            .unwrap();
    }

    /// Unspent tonnage is cargo or waste, reported as a warning; overweight is refused.
    #[test]
    fn tonnage_budget_warns_when_light_and_refuses_when_heavy() {
        let light = parse(&ZHUKOV.replace(
            "armor = 44\nslots = [\n    { at = \"1-2\"",
            "armor = 36\nslots = [\n    { at = \"1-2\"",
        ));
        let report = light.construction_rules().unwrap();
        assert_eq!(
            report.warnings,
            ["0.5 tons unspent: cargo space or wasted weight (TechManual p. 97)"]
        );
        let heavy = parse(&ZHUKOV.replace("armor = 24", "armor = 32"));
        assert!(
            rejection(&heavy)
                .contains("Components weigh 75.5 tons, more than the vehicle's 75 tons")
        );
        let check = check_vehicle_template(&heavy);
        assert!(!check.constructible && check.construction.is_none());
    }

    /// Every rule names itself when it refuses a template.
    #[test]
    fn construction_rules_refuse_illegal_vehicles() {
        let jacket = parse(YELLOW_JACKET);
        let mut heavy_vtol = jacket.clone();
        heavy_vtol.tons = 31;
        let mut rotor_armor = jacket.clone();
        rotor_armor
            .sections
            .get_mut(&VehicleSection::Rotor)
            .unwrap()
            .armor = 3;
        let mut rear = jacket.clone();
        rear.sections.get_mut(&VehicleSection::Front).unwrap().rear = 1;
        let mut double = jacket.clone();
        double
            .attributes
            .insert("specials".into(), "ICEEngine_Tech DoubleHS".into());
        let mut endo = jacket.clone();
        endo.attributes
            .insert("specials".into(), "ICEEngine_Tech EndoSteel_Tech".into());
        let mut jumping = jacket.clone();
        jumping
            .attributes
            .insert("jump_speed".into(), "32.25".into());
        let mut armored = jacket.clone();
        armored
            .sections
            .get_mut(&VehicleSection::Front)
            .unwrap()
            .armor = 110;
        let mut fast = parse(ZHUKOV);
        fast.walk_mp = 6;
        for (template, expected) in [
            (heavy_vtol, "weigh at most 30 tons (TechManual p. 96)"),
            (rotor_armor, "more than 2 (TechManual p. 105)"),
            (rear, "single armor value (TechManual p. 105)"),
            (double, "single heat sinks only (TechManual p. 104)"),
            (endo, "standard internal structure only (TechManual p. 98)"),
            (jumping, "cannot jump (TechManual p. 102)"),
            (armored, "exceed the Maximum Armor Factor of 145"),
            (fast, "Engine Rating 450 exceeds"),
        ] {
            let error = rejection(&template);
            assert!(error.contains(expected), "{expected}: {error}");
        }
        let lasers = parse(&YELLOW_JACKET.replace(
            "{ at = 1, item = \"IS.GaussRifle\" },",
            "{ at = 1, item = \"IS.LargeLaser\" },\n    { at = 4, item = \"IS.LargeLaser\" },",
        ));
        assert!(rejection(&lasers).contains("0 heat sinks cannot vent the 16 heat"));
        let rotor_gun = parse(&YELLOW_JACKET.replace(
            "[sections.rotor]\narmor = 2\n",
            "[sections.rotor]\narmor = 2\nslots = [{ at = 1, item = \"IS.MachineGun\" }]\n",
        ));
        assert!(rejection(&rotor_gun).contains("No equipment may be mounted in the rotor"));
        let crowded = parse(&YELLOW_JACKET.replace(
            "[sections.aft_side]\narmor = 8\n",
            "[sections.aft_side]\narmor = 8\nslots = [{ at = \"1-10\", item = \"IS.MachineGun\" }]\n",
        ));
        assert!(rejection(&crowded).contains("12 items exceed the 11 item slots"));
    }

    /// Ferro-fibrous armor and XL and XXL engines claim item slots; a compact engine frees one
    /// (TechManual pp. 101, 105; Tactical Operations: Advanced Units & Equipment p. 120).
    #[test]
    fn item_slots_follow_engine_and_armor_choices() {
        let jacket = parse(YELLOW_JACKET);
        for (specials, limit) in [
            ("ICEEngine_Tech", 11),
            ("XLEngine_Tech", 9),
            ("Clan XLEngine_Tech", 10),
            ("LightEngine_Tech", 10),
            ("CompactEngine_Tech", 12),
            ("XXL_Tech", 7),
            ("Clan XXL_Tech", 9),
            ("ICEEngine_Tech FerroFibrous_Tech", 9),
            ("Clan ICEEngine_Tech FerroFibrous_Tech", 10),
            ("ICEEngine_Tech LtFerroFibrous_Tech", 10),
            ("ICEEngine_Tech HvyFerroFibrous_Tech", 8),
            ("ICEEngine_Tech HardenedArmor_Tech", 10),
            ("ICEEngine_Tech LaserRefArmor_Tech", 10),
            ("Clan ICEEngine_Tech LaserRefArmor_Tech", 10),
            ("ICEEngine_Tech StealthArmor_Tech", 9),
        ] {
            let mut template = jacket.clone();
            template
                .attributes
                .insert("specials".into(), specials.into());
            assert_eq!(template.item_slot_limit(), limit, "{specials}");
        }
    }

    /// Clan vehicles build no light or compact fusion engines and no light or heavy
    /// ferro-fibrous armor (TechManual pp. 100, 105); an Inner Sphere XL engine on a Clan
    /// chassis needs a mixed-technology design (TechManual p. 46).
    #[test]
    fn clan_vehicles_use_clan_engines_and_armor() {
        let clan = |construction: &str| {
            parse(&ZHUKOV.replace(
                "[construction]\nengine = \"ice\"\n",
                &format!("[construction]\ntech_base = \"clan\"\n{construction}"),
            ))
        };
        for (construction, expected) in [
            ("engine = \"light\"\n", "may not use light fusion engines"),
            (
                "engine = \"compact\"\n",
                "may not use compact fusion engines",
            ),
            (
                "armor = \"light_ferro_fibrous\"\n",
                "may not use light ferro-fibrous armor",
            ),
            (
                "armor = \"heavy_ferro_fibrous\"\n",
                "may not use heavy ferro-fibrous armor",
            ),
            (
                "engine = \"xl\"\nengine_tech = \"inner_sphere\"\n",
                "The engine is not Clan technology",
            ),
        ] {
            let error = rejection(&clan(construction));
            assert!(error.contains(expected), "{construction}: {error}");
        }
        // Standard, Clan XL and ICE engines with standard or Clan ferro-fibrous armor pass the
        // availability check; any refusal is some other rule.
        for construction in [
            "",
            "engine = \"xl\"\n",
            "engine = \"ice\"\narmor = \"ferro_fibrous\"\n",
        ] {
            assert!(
                clan(construction).check_clan_availability().is_ok(),
                "{construction}"
            );
        }
        // A mixed-technology Clan design may build an Inner Sphere light engine, but not a
        // Clan one, which does not exist.
        let mixed = clan("mixed_tech = true\nengine = \"light\"\nengine_tech = \"inner_sphere\"\n");
        assert!(mixed.check_clan_availability().is_ok());
        let clan_light = clan("mixed_tech = true\nengine = \"light\"\n");
        assert!(clan_light.check_clan_availability().is_err());
        // Inner Sphere vehicles keep every TechManual engine and armor type.
        let inner_sphere = parse(&ZHUKOV.replace(
            "engine = \"ice\"\n",
            "engine = \"light\"\narmor = \"heavy_ferro_fibrous\"\n",
        ));
        assert!(inner_sphere.check_clan_availability().is_ok());
    }

    /// Hardened armor stays off VTOLs and hovercraft; vehicular stealth armor needs an ECM
    /// suite, heat sinks for its 10 heat and an Inner Sphere technology base (Tactical
    /// Operations: Advanced Units & Equipment pp. 92-93).
    #[test]
    fn advanced_armors_follow_the_advanced_armor_table() {
        let armored = |source: &str, armor: &str| {
            parse(&source.replace(
                "engine = \"ice\"\n",
                &format!("engine = \"ice\"\narmor = \"{armor}\"\n"),
            ))
        };
        let hardened = YELLOW_JACKET.replace(
            "engine = \"ice\"\n",
            "engine = \"ice\"\narmor = \"hardened\"\n",
        );
        let error = VehicleTemplate::parse("test", &hardened).unwrap_err();
        assert!(format!("{error:#}").contains("cannot mount hardened armor"));
        let tank = armored(ZHUKOV, "hardened");
        assert!(
            tank.check_advanced_armor(&VehicleLoadout::resolve(&tank).unwrap())
                .is_ok()
        );
        let reflective = armored(YELLOW_JACKET, "laser_reflective");
        assert!(
            reflective
                .check_advanced_armor(&VehicleLoadout::resolve(&reflective).unwrap())
                .is_ok()
        );
        // Stealth armor without an ECM suite, then with one but no sinks for its heat.
        let stealth = armored(ZHUKOV, "stealth");
        assert!(rejection(&stealth).contains("needs an ECM suite"));
        let with_ecm = |source: &str| {
            source.replace(
                "{ at = 2, item = \"Ammo_IS.SRM-6\", rounds = 15 },",
                "{ at = 2, item = \"Ammo_IS.SRM-6\", rounds = 15 },\n    { at = 3, item = \"ECM\" },",
            )
        };
        let mut cold = armored(&with_ecm(ZHUKOV), "stealth");
        let error = rejection(&cold);
        assert!(
            error.contains("0 heat sinks cannot vent the 0 heat of the energy weapons and the 10 of the stealth armor"),
            "{error}"
        );
        cold.heat_sinks = Some(10);
        assert!(
            cold.check_advanced_armor(&VehicleLoadout::resolve(&cold).unwrap())
                .is_ok()
        );
        let clan = parse(&with_ecm(ZHUKOV).replace(
            "engine = \"ice\"\n",
            "tech_base = \"clan\"\nengine = \"ice\"\narmor = \"stealth\"\n",
        ));
        assert!(
            clan.check_advanced_armor(&VehicleLoadout::resolve(&clan).unwrap())
                .unwrap_err()
                .to_string()
                .contains("Inner Sphere technology")
        );
    }

    /// Transport bays weigh, take slots and cost as the Transport Bay table says (TechManual
    /// p. 239), alongside the standard cargo bay `cargo_tons` describes, and survive a
    /// document round trip; Mechs carry none.
    #[test]
    fn transport_bays_weigh_claim_slots_and_cost() {
        // Trade the Zhukov's SRM-6 and its ammunition (4 tons, 2 slots) for a 4-ton infantry
        // compartment, as the Hephaestus worked example allots infantry-cargo space (TechManual
        // p. 107).
        let carrier = ZHUKOV
            .replace(
                "slots = [\n    { at = 1, item = \"IS.SRM-6\" },\n    { at = 2, item = \"Ammo_IS.SRM-6\", rounds = 15 },\n]\n",
                "",
            )
            .replace(
                "[sections.left_side]",
                "[transport_bays]\ninfantry_compartment = 4\n\n[sections.left_side]",
            );
        let template = parse(&carrier);
        let report = template.construction_rules().unwrap();
        assert_eq!(report.mass.cargo, 4 * 1024);
        assert_eq!(report.mass.design_total, 75 * 1024);
        assert_eq!(report.item_slots, 4);
        let prices = crate::PartPrices::new();
        assert_eq!(
            crate::vehicle_template_base_cost(&prices, &template).unwrap(),
            crate::vehicle_template_base_cost(&prices, &parse(ZHUKOV)).unwrap()
                - crate::Weapon::Srm6.cost() * 175 / 100
        );
        // A foot infantry bay instead weighs 5 tons and costs 15,000 C-bills a ton; with a
        // 1-ton standard cargo bay beside it, each takes its own slot.
        let bay = parse(
            &carrier
                .replace("infantry_compartment = 4", "foot_infantry_bay = 1")
                .replace("walk_mp = 3\n", "walk_mp = 3\ncargo_tons = 1\n"),
        );
        assert_eq!(bay.mass().unwrap().cargo, 6 * 1024);
        assert_eq!(bay.item_slots(&VehicleLoadout::resolve(&bay).unwrap()), 5);
        assert_eq!(
            crate::vehicle_template_base_cost(&prices, &bay).unwrap(),
            crate::vehicle_template_base_cost(&prices, &template).unwrap() + 75_000 * 175 / 100
        );
        let rendered = bay.to_document().unwrap();
        assert!(
            rendered.contains("\n[transport_bays]\nfoot_infantry_bay = 1\n"),
            "{rendered}"
        );
        assert_eq!(VehicleTemplate::parse("test", &rendered).unwrap(), bay);
        let mech = crate::MechTemplate::parse(
            "test",
            "name = \"M\"\nclass = \"mech\"\n[transport_bays]\nliquid_cargo = 1\n",
        )
        .unwrap_err();
        assert!(
            format!("{mech:#}").contains("only for vehicles"),
            "{mech:#}"
        );
    }

    /// The Final Engine Rating rounds up to a multiple of 5 and is never below 10, and fusion
    /// shielding applies to the engine type's listed weight (TechManual p. 100).
    #[test]
    fn engine_rating_rounds_up_and_shields_the_listed_weight() {
        let mut template = parse(YELLOW_JACKET);
        // A 22-ton VTOL at 6 cruising MP: 132 - 140 is below 10, so the rating is 10.
        template.tons = 22;
        let engine = template.engine().unwrap();
        assert_eq!((engine.nominal_rating, engine.weight_rating), (132, 10));
        // A 21-ton VTOL at 9 cruising MP: 189 - 140 = 49, rounded up to 50.
        template.tons = 21;
        template.walk_mp = 9;
        assert_eq!(template.engine().unwrap().weight_rating, 50);
        // A 65-ton tank at 2 cruising MP has a 130 rating: the XL engine lists 2.5 tons and
        // weighs 4 with shielding, where shielding the standard weight first would give 3.5.
        let mut tank = parse(ZHUKOV);
        tank.attributes
            .insert("specials".into(), "XLEngine_Tech".into());
        tank.tons = 65;
        tank.walk_mp = 2;
        let engine = tank.engine().unwrap();
        assert_eq!(engine.weight_rating, 130);
        assert_eq!(engine.engine_mass, 4 * 1024);
    }

    /// A declared engine rating must be the Final Engine Rating the tonnage, cruising MP and
    /// suspension factor give (TechManual p. 100), and follows a cruising MP edit.
    #[test]
    fn a_declared_engine_rating_matches_the_final_engine_rating() {
        let declare = |rating: u32| {
            parse(&YELLOW_JACKET.replace(
                "engine = \"ice\"\n",
                &format!("engine = \"ice\"\nengine_rating = {rating}\n"),
            ))
        };
        // 30 tons x 6 cruising MP less the 140-point VTOL suspension factor.
        let jacket = declare(40);
        assert_eq!(jacket.declared_engine_rating().unwrap(), Some(40));
        assert!(jacket.construction_rules().is_ok());
        let report = check_vehicle_template(&jacket);
        assert_eq!(
            (report.engine_rating, report.declared_engine_rating),
            (40, Some(40))
        );
        assert!(
            rejection(&declare(45))
                .contains("declared engine rating 45 is not the Final Engine Rating 40")
        );
        let mut faster = jacket;
        faster.set_cruising_mp(9);
        assert_eq!(faster.declared_engine_rating().unwrap(), Some(130));
        assert_eq!(faster.engine().unwrap().weight_rating, 130);
        assert_eq!(parse(YELLOW_JACKET).declared_engine_rating().unwrap(), None);
    }

    /// ICE vehicles carry power amplifiers for energy weapons; fusion vehicles do not. CASE
    /// on Clan internal structure is built in, weighing nothing and taking no item slot, while
    /// on Inner Sphere structure it weighs half a ton in one slot, whatever the chassis's base
    /// (TechManual p. 210).
    #[test]
    fn amplifiers_and_case_follow_the_engine_and_structure() {
        let lasers = YELLOW_JACKET.replace(
            "{ at = 1, item = \"IS.GaussRifle\" },",
            "{ at = 1, item = \"IS.LargeLaser\" },\n    { at = 4, item = \"CASE\" },",
        );
        let ice = parse(&lasers);
        assert_eq!(ice.mass().unwrap().power_amplifiers, 512);
        let case = |template: &VehicleTemplate| {
            let loadout = VehicleLoadout::resolve(template).unwrap();
            let part = loadout
                .systems
                .iter()
                .find(|part| part.system == System::Case)
                .unwrap();
            (
                template.system_mass(&loadout, part),
                occupies_vehicle_slot(part.system, template.clan_structure()),
            )
        };
        assert_eq!(case(&ice), (512, true));
        let fusion = parse(&lasers.replace("engine = \"ice\"", "tech_base = \"clan\""));
        assert_eq!(fusion.mass().unwrap().power_amplifiers, 0);
        assert_eq!(case(&fusion), (0, false));
        let mixed = |source: &str, structure: &str| {
            parse(&source.replace(
                "[construction]\n",
                &format!("[construction]\nmixed_tech = true\nstructure_tech = \"{structure}\"\n"),
            ))
        };
        let clan = lasers.replace("engine = \"ice\"", "tech_base = \"clan\"");
        assert_eq!(case(&mixed(&clan, "inner_sphere")), (512, true));
        assert_eq!(case(&mixed(&lasers, "clan")), (0, false));
    }

    /// Beagle, ECM, CASE II and machine gun array masses follow each slot's own `IS.` or `CL.`
    /// prefix, and the chassis's base only when the slot states none, as Mech mass does: a
    /// Clan active probe weighs a ton on an Inner Sphere tank and a Beagle a ton and a half on
    /// a Clan one (TechManual pp. 204-238).
    #[test]
    fn either_base_systems_weigh_by_their_own_slot() {
        let inner_sphere = parse(ZHUKOV);
        let clan = parse(&ZHUKOV.replace("engine = \"ice\"", "tech_base = \"clan\""));
        assert!(clan.has_special("Clan") && !inner_sphere.has_special("Clan"));
        let mass = |template: &VehicleTemplate, system, tech_base| {
            let loadout = VehicleLoadout::resolve(template).unwrap();
            let part = super::super::SystemCritical {
                location: super::super::CriticalLocation {
                    section: VehicleSection::Front,
                    slot: 0,
                },
                system,
                tech_base,
                links: Vec::new(),
            };
            template.system_mass(&loadout, &part)
        };
        let (is, cl) = (
            Some(super::super::TechBase::InnerSphere),
            Some(super::super::TechBase::Clan),
        );
        for (system, inner_sphere_mass, clan_mass) in [
            (System::BeagleProbe, 1536, 1024),
            (System::Ecm, 1536, 1024),
            (System::CaseIi, 1024, 512),
            (System::MachineGunArray, 512, 256),
        ] {
            for template in [&inner_sphere, &clan] {
                assert_eq!(mass(template, system, is), inner_sphere_mass, "{system:?}");
                assert_eq!(mass(template, system, cl), clan_mass, "{system:?}");
            }
            assert_eq!(
                mass(&inner_sphere, system, None),
                inner_sphere_mass,
                "{system:?}"
            );
            assert_eq!(mass(&clan, system, None), clan_mass, "{system:?}");
        }
    }

    /// A vehicle flamer needs no heat sinks, though it counts as an energy weapon in play
    /// (TechManual p. 218): an ICE vehicle mounts one with none, while a standard or heavy
    /// flamer's heat still calls for them.
    #[test]
    fn vehicle_flamers_need_no_heat_sinks() {
        let flamer = YELLOW_JACKET.replace(
            "{ at = 1, item = \"IS.GaussRifle\" },\n    { at = \"2-3\", item = \"Ammo_IS.GaussRifle\", rounds = 8 },",
            "{ at = 1, item = \"IS.VehicleFlamer\" },\n    { at = 2, item = \"Ammo_IS.VehicleFlamer\", rounds = 20 },",
        );
        assert_ne!(flamer, YELLOW_JACKET);
        let template = parse(&flamer);
        assert_eq!(template.heat_sink_capacity(), 0);
        let loadout = VehicleLoadout::resolve(&template).unwrap();
        assert_eq!(loadout.weapons[0].weapon, Weapon::VehicleFlamer);
        assert_eq!(VehicleTemplate::energy_heat(&loadout), 0);
        assert_eq!(template.construction_rules().unwrap().energy_heat, 0);
        assert!(!VehicleTemplate::needs_heat_sinks(Weapon::VehicleFlamer));
        for weapon in [Weapon::Flamer, Weapon::HeavyFlamer, Weapon::MediumLaser] {
            assert!(VehicleTemplate::needs_heat_sinks(weapon), "{weapon:?}");
        }
        assert!(!VehicleTemplate::needs_heat_sinks(Weapon::GaussRifle));
        // A laser anti-missile system counts as an energy weapon for the heat sinks (Tactical
        // Operations: Advanced Units & Equipment p. 131).
        for weapon in [Weapon::LaserAms, Weapon::ClanLaserAms] {
            assert!(VehicleTemplate::needs_heat_sinks(weapon), "{weapon:?}");
        }
        let laser_ams = YELLOW_JACKET.replace(
            "{ at = 1, item = \"IS.GaussRifle\" },\n    { at = \"2-3\", item = \"Ammo_IS.GaussRifle\", rounds = 8 },",
            "{ at = 1, item = \"IS.LaserAMS\" },",
        );
        let template = parse(&laser_ams);
        let loadout = VehicleLoadout::resolve(&template).unwrap();
        assert_eq!(VehicleTemplate::energy_heat(&loadout), 7);
        assert!(rejection(&template).contains("heat sinks cannot vent the 7 heat"));
        // A combustion engine mounts vehicle flamers, never standard ones (TechManual p. 218).
        let standard = flamer
            .replace("IS.VehicleFlamer\" }", "IS.Flamer\" }")
            .replace(
                "{ at = 2, item = \"Ammo_IS.VehicleFlamer\", rounds = 20 },",
                "",
            );
        assert!(
            rejection(&parse(&standard))
                .contains("IS.Flamer needs a fusion engine; an internal combustion vehicle mounts vehicle flamers instead"),
            "{}",
            rejection(&parse(&standard))
        );
        // The heavy flamer is a vehicle weapon too, and its heat needs sinks.
        let heavy = flamer
            .replace("IS.VehicleFlamer\" }", "IS.HeavyFlamer\" }")
            .replace(
                "Ammo_IS.VehicleFlamer\", rounds = 20",
                "Ammo_IS.HeavyFlamer\", rounds = 10",
            );
        assert!(rejection(&parse(&heavy)).contains("heat sinks cannot vent"));
    }

    /// A dual saw is one item weighing its full seven tons on a vehicle (TechManual p. 344).
    #[test]
    fn a_vehicle_dual_saw_weighs_seven_tons() {
        let bare = parse(ZHUKOV).mass().unwrap().equipment;
        let saw = with_rear(ZHUKOV, r#"{ at = 1, item = "Dual_Saw" }"#);
        assert_eq!(saw.mass().unwrap().equipment, bare + 7 * 1024);
        let loadout = VehicleLoadout::resolve(&saw).unwrap();
        assert_eq!(
            saw.item_slots(&loadout),
            parse(ZHUKOV).item_slots(&VehicleLoadout::resolve(&parse(ZHUKOV)).unwrap()) + 1
        );
    }

    /// A vehicle mounts its own technology base's weapons unless it declares mixed
    /// technology (TechManual p. 46).
    #[test]
    fn weapons_follow_the_vehicle_technology_base() {
        let clan = YELLOW_JACKET.replace("IS.GaussRifle", "CL.GaussRifle");
        let refused = rejection(&parse(&clan));
        assert!(
            refused.contains("CL.GaussRifle is Clan technology on an Inner Sphere vehicle"),
            "{refused}"
        );
        let mixed = parse(&clan.replace("[construction]\n", "[construction]\nmixed_tech = true\n"));
        let accepted = mixed
            .construction_rules()
            .map(|_| ())
            .map_err(|error| error.to_string());
        assert!(
            !accepted
                .as_ref()
                .err()
                .is_some_and(|error| error.contains("technology")),
            "{accepted:?}"
        );
    }

    #[test]
    fn maximum_armor_and_tonnage_follow_the_tables() {
        // The Zhukov may carry 302 points and the Yellow Jacket 145 (TechManual p. 106).
        assert_eq!(vehicle_maximum_armor(75), 302);
        assert_eq!(vehicle_maximum_armor(30), 145);
        assert_eq!(VehicleMovement::Hover.maximum_tonnage(), Some(50));
        assert_eq!(VehicleMovement::Vtol.maximum_tonnage(), Some(30));
        assert_eq!(VehicleMovement::Wheeled.maximum_tonnage(), Some(80));
        assert_eq!(VehicleMovement::Tracked.maximum_tonnage(), Some(100));
        assert_eq!(VehicleMovement::Stationary.maximum_tonnage(), None);
        assert_eq!(format_tons(5 * 512), "2.5");
        assert_eq!(format_tons(1024), "1");
        assert_eq!(format_tons(256), "0.25");
    }

    /// An MRM "Apollo" system serves every MRM launcher from its location and, like Artemis,
    /// counts as part of the launcher rather than its own item (Tactical Operations: Advanced
    /// Units & Equipment p. 142; TechManual p. 206).
    #[test]
    fn apollo_serves_every_mrm_from_its_location() {
        // An MRM-10 in place of the SRM-6, with a ton of front armor freed for the system.
        let mrm = ZHUKOV
            .replace("IS.SRM-6", "IS.MRM-10")
            .replace("rounds = 15", "rounds = 24")
            .replace(
                "armor = 44\nslots = [\n    { at = 1",
                "armor = 28\nslots = [\n    { at = 1",
            );
        let plain = parse(&mrm).construction_rules().unwrap();
        let apollo = "\n    { at = 3, item = \"ApolloFCS\", link = 1 },";
        let guided_source = mrm.replace("rounds = 24 },", &format!("rounds = 24 }},{apollo}"));
        let guided = parse(&guided_source);
        let report = guided.construction_rules().unwrap();
        assert_eq!(report.item_slots, plain.item_slots);
        assert_eq!(report.mass.design_total, plain.mass.design_total + 1024);
        let loadout = VehicleLoadout::resolve(&guided).unwrap();
        assert_eq!(loadout.systems[0].system, System::ApolloFcs);
        let misplaced = parse(&mrm.replace(
            "{ at = \"3-4\", item = \"Ammo_IS.AC/10\", rounds = 10 },",
            &format!(
                "{{ at = \"3-4\", item = \"Ammo_IS.AC/10\", rounds = 10 }},{}",
                apollo.replace("at = 3", "at = 5")
            ),
        ));
        let refused = rejection(&misplaced);
        assert!(
            refused.contains(
                "The Apollo fire control system in Turret slot 5 links slot 1, which is not the first slot of an MRM launcher in Turret"
            ),
            "{refused}"
        );
    }

    /// The Zhukov source with `entry` alone in its rear location.
    fn with_rear_text(source: &str, entry: &str) -> String {
        source.replace(
            "[sections.aft_side]\narmor = 24\n",
            &format!("[sections.aft_side]\narmor = 24\nslots = [{entry}]\n"),
        )
    }

    /// A Zhukov with `entry` alone in its rear location.
    fn with_rear(source: &str, entry: &str) -> VehicleTemplate {
        parse(&with_rear_text(source, entry))
    }

    /// Combat vehicles mount no BattleMech-only equipment: Mech critical-slot entries, jump
    /// jets, MASC, triple-strength myomer, CASE II, the null-signature system or the Mech melee
    /// weapons (TechManual pp. 99-105, 220, 236-237; Tactical Operations: Advanced Units &
    /// Equipment pp. 100-104, 111, 148). A ground vehicle mounts a dual saw in its front, rear
    /// or turret, one per location, and a wrecking ball in its turret; a VTOL mounts neither
    /// (TechManual pp. 243, 249).
    #[test]
    fn vehicles_refuse_battlemech_equipment() {
        for (item, reason) in [
            ("HeatSink", "is a BattleMech critical-slot entry"),
            ("Gyro", "is a BattleMech critical-slot entry"),
            ("FerroFibrous", "is a BattleMech critical-slot entry"),
            ("JumpJet", "combat vehicles cannot jump"),
            ("Masc", "is MASC"),
            ("TripleStrengthMyomer", "is triple-strength myomer"),
            ("CASE-II", "is CASE II"),
            ("NullSig_Device", "is the null-signature system"),
            ("Axe", "only humanoid BattleMechs"),
            ("Retractable_Blade", "only humanoid BattleMechs"),
            ("Claw", "only BattleMechs and IndustrialMechs"),
            ("Medium_Vibroblade", "only BattleMechs and IndustrialMechs"),
            ("Spikes", "only BattleMechs and IndustrialMechs"),
        ] {
            let refused = rejection(&with_rear(
                ZHUKOV,
                &format!("{{ at = 1, item = \"{item}\" }}"),
            ));
            assert!(
                refused.contains("in Aft_Side slot 1") && refused.contains(reason),
                "{item}: {refused}"
            );
        }
        // The dual saw passes every equipment rule in the rear and stops at the tonnage.
        let saw = r#"{ at = 1, item = "Dual_Saw" }"#;
        let refused = rejection(&with_rear(ZHUKOV, saw));
        assert!(
            refused.contains("more than the vehicle's 75 tons"),
            "{refused}"
        );
        let refused = rejection(&parse(&ZHUKOV.replace(
            "[sections.left_side]\narmor = 32\n",
            &format!("[sections.left_side]\narmor = 32\nslots = [{saw}]\n"),
        )));
        assert!(
            refused.contains("only in its front, rear or turret"),
            "{refused}"
        );
        let pair = format!("{saw}, {}", saw.replace("at = 1", "at = 2"));
        let refused = rejection(&with_rear(ZHUKOV, &pair));
        assert!(
            refused.contains("a location holds one backhoe, dual saw or wrecking ball"),
            "{refused}"
        );
        let refused = rejection(&with_rear(ZHUKOV, r#"{ at = 1, item = "Wrecking_Ball" }"#));
        assert!(
            refused.contains("a wrecking ball only in its turret"),
            "{refused}"
        );
        let refused = rejection(&parse(&YELLOW_JACKET.replace(
            "[sections.aft_side]\narmor = 8\n",
            &format!("[sections.aft_side]\narmor = 8\nslots = [{saw}]\n"),
        )));
        assert!(refused.contains("and a VTOL none"), "{refused}");
    }

    /// A vehicle mounts one set of communications equipment as a single item of the tons it
    /// states, up to fifteen (TechManual p. 212); at most four lift hoists of three tons each
    /// (TechManual pp. 245, 344); and one backhoe of five tons in each of the front, rear and
    /// turret of a ground vehicle, never beside a dual saw or wrecking ball (TechManual
    /// p. 241).
    #[test]
    fn vehicles_mount_communications_hoists_and_backhoes_by_the_book() {
        let bare = parse(ZHUKOV);
        let bare_slots = bare.item_slots(&VehicleLoadout::resolve(&bare).unwrap());
        let bare_mass = bare.mass().unwrap().equipment;
        let comms = with_rear(
            ZHUKOV,
            r#"{ at = 1, item = "Communications_Equipment", tons = 4 }"#,
        );
        assert_eq!(comms.mass().unwrap().equipment, bare_mass + 4 * 1024);
        assert_eq!(
            comms.item_slots(&VehicleLoadout::resolve(&comms).unwrap()),
            bare_slots + 1
        );
        assert!(rejection(&comms).contains("more than the vehicle's 75 tons"));
        let document = comms.to_document().unwrap();
        assert!(
            document.contains(r#"{ at = 1, item = "Communications_Equipment", tons = 4 }"#),
            "{document}"
        );
        assert_eq!(parse(&document), comms);
        let misplaced = VehicleTemplate::parse(
            "test",
            &with_rear_text(ZHUKOV, r#"{ at = 1, item = "Lift_Hoist", tons = 3 }"#),
        )
        .unwrap_err();
        assert!(
            format!("{misplaced:#}").contains("only communications equipment takes tons"),
            "{misplaced:#}"
        );
        for tons in [0, 16] {
            let refused = rejection(&with_rear(
                ZHUKOV,
                &format!(r#"{{ at = 1, item = "Communications_Equipment", tons = {tons} }}"#),
            ));
            assert!(refused.contains("a set of 1 to 15 whole tons"), "{refused}");
        }
        let refused = rejection(&with_rear(
            ZHUKOV,
            r#"{ at = 1, item = "Communications_Equipment", tons = 1 }, { at = 2, item = "Communications_Equipment", tons = 1 }"#,
        ));
        assert!(
            refused.contains("one set of communications equipment"),
            "{refused}"
        );
        let hoist = with_rear(ZHUKOV, r#"{ at = 1, item = "Lift_Hoist" }"#);
        assert_eq!(hoist.mass().unwrap().equipment, bare_mass + 3 * 1024);
        let refused = rejection(&with_rear(ZHUKOV, r#"{ at = "1-5", item = "Lift_Hoist" }"#));
        assert!(
            refused.contains("at most four lift hoists; found 5"),
            "{refused}"
        );
        let backhoe = with_rear(ZHUKOV, r#"{ at = 1, item = "Backhoe" }"#);
        assert_eq!(backhoe.mass().unwrap().equipment, bare_mass + 5 * 1024);
        assert!(rejection(&backhoe).contains("more than the vehicle's 75 tons"));
        let refused = rejection(&with_rear(
            ZHUKOV,
            r#"{ at = 1, item = "Backhoe" }, { at = 2, item = "Dual_Saw" }"#,
        ));
        assert!(
            refused.contains("a location holds one backhoe, dual saw or wrecking ball"),
            "{refused}"
        );
        let refused = rejection(&parse(&YELLOW_JACKET.replace(
            "[sections.aft_side]\narmor = 8\n",
            "[sections.aft_side]\narmor = 8\nslots = [{ at = 1, item = \"Backhoe\" }]\n",
        )));
        assert!(
            refused.contains("a backhoe only in its front, rear or turret"),
            "{refused}"
        );
    }

    /// A slot's stated technology base must match a vehicle's chassis unless the design is
    /// mixed technology, and Clan CASE, built into Clan structure, claims no item slot while
    /// Inner Sphere CASE claims one (TechManual pp. 46, 210).
    #[test]
    fn vehicle_systems_follow_their_stated_technology_base() {
        let refused = rejection(&with_rear(ZHUKOV, r#"{ at = 1, item = "CL.Ecm" }"#));
        assert!(
            refused.contains("CL.Ecm is Clan technology on an Inner Sphere vehicle"),
            "{refused}"
        );
        assert!(occupies_vehicle_slot(System::Case, false));
        assert!(!occupies_vehicle_slot(System::Case, true));
        let case = r#"{ at = 1, item = "Case" }"#;
        let slots = |source: &str| {
            let template = parse(source);
            template.item_slots(&VehicleLoadout::resolve(&template).unwrap())
        };
        assert_eq!(slots(&with_rear_text(ZHUKOV, case)), slots(ZHUKOV) + 1);
        let clan = ZHUKOV.replace(
            "engine = \"ice\"",
            "tech_base = \"clan\"\nmixed_tech = true\nengine = \"ice\"",
        );
        assert_eq!(slots(&with_rear_text(&clan, case)), slots(&clan));
    }

    /// Artemis V is incompatible with Artemis IV, so a unit may not carry both (Tactical
    /// Operations: Advanced Units & Equipment p. 94).
    #[test]
    fn artemis_iv_and_v_are_not_combined() {
        let mixed = ZHUKOV
            .replace("engine = \"ice\"", "engine = \"ice\"\nmixed_tech = true")
            .replace(
                "{ at = 2, item = \"Ammo_IS.SRM-6\", rounds = 15 },",
                "{ at = 2, item = \"Ammo_IS.SRM-6\", rounds = 15 },\n    { at = 3, item = \"ArtemisIV\", link = 1 },",
            )
            .replace(
                "{ at = \"3-4\", item = \"Ammo_IS.AC/10\", rounds = 10 },",
                "{ at = \"3-4\", item = \"Ammo_IS.AC/10\", rounds = 10 },\n    { at = 5, item = \"IS.SRM-6\" },\n    { at = 6, item = \"ArtemisV\", link = 5 },",
            );
        let refused = rejection(&parse(&mixed));
        assert!(
            refused.contains("Artemis V is incompatible with Artemis IV"),
            "{refused}"
        );
    }

    /// A fire-control system serves only launchers in its own location: a turret launcher
    /// guided from the turret builds, while one guided from the rear is refused, for Artemis
    /// IV and V (TechManual p. 206) and the MRM "Apollo" system (Tactical Operations:
    /// Advanced Units & Equipment p. 142).
    #[test]
    fn fire_control_sits_in_its_launchers_location() {
        // The SRM-6 moves into the turret, whose mass rises half a ton; with a ton of fire
        // control, 24 points of front armor pay for both.
        let turret = ZHUKOV
            .replace("    { at = 1, item = \"IS.SRM-6\" },\n", "")
            .replace(
                "armor = 44\nslots = [\n    { at = 2",
                "armor = 20\nslots = [\n    { at = 2",
            )
            .replace(
                "{ at = \"3-4\", item = \"Ammo_IS.AC/10\", rounds = 10 },",
                "{ at = \"3-4\", item = \"Ammo_IS.AC/10\", rounds = 10 },\n    { at = 5, item = \"IS.SRM-6\" },",
            );
        let mrm = turret
            .replace("IS.SRM-6", "IS.MRM-10")
            .replace("rounds = 15", "rounds = 24");
        // A controller beside the turret launcher in slot 5, or alone in the rear.
        let local = |source: &str, controller: &str| {
            parse(&source.replace(
                "    { at = 5, item = ",
                &format!(
                    "    {{ at = 6, item = \"{controller}\", link = 5 }},\n    {{ at = 5, item = "
                ),
            ))
        };
        let rear = |source: &str, controller: &str| {
            parse(&source.replace(
                "[sections.aft_side]\narmor = 24\n",
                &format!(
                    "[sections.aft_side]\narmor = 24\nslots = [{{ at = 1, item = \"{controller}\", link = 5 }}]\n"
                ),
            ))
        };
        for (source, controller, system) in [
            (&turret, "ArtemisIV", System::ArtemisIv),
            (&mrm, "ApolloFCS", System::ApolloFcs),
        ] {
            let guided = local(source, controller);
            guided.construction_rules().unwrap();
            let loadout = VehicleLoadout::resolve(&guided).unwrap();
            assert_eq!(loadout.systems[0].system, system);
            assert_eq!(loadout.systems[0].location.section, VehicleSection::Turret);
        }
        let refused = rejection(&rear(&turret, "ArtemisIV"));
        assert!(
            refused.contains(
                "The Artemis IV in Aft_Side slot 1 links slot 5, which is not the first slot of an LRM, SRM or MML launcher in Aft_Side"
            ),
            "{refused}"
        );
        // Artemis V is Clan technology, so the Inner Sphere chassis declares mixed technology.
        let mixed = turret.replace("engine = \"ice\"", "engine = \"ice\"\nmixed_tech = true");
        let refused = rejection(&rear(&mixed, "ArtemisV"));
        assert!(
            refused.contains("The Artemis V in Aft_Side slot 1 links slot 5"),
            "{refused}"
        );
        let refused = rejection(&rear(&mrm, "ApolloFCS"));
        assert!(
            refused.contains(
                "The Apollo fire control system in Aft_Side slot 1 links slot 5, which is not the first slot of an MRM launcher in Aft_Side"
            ),
            "{refused}"
        );
    }

    /// Every fire-control entry names the first slot of the launcher it guides in its own
    /// location, and no two entries share a launcher (TechManual p. 206).
    #[test]
    fn fire_control_links_are_explicit_and_unique() {
        // A ton of front armor pays for each Artemis IV beside the front SRM-6.
        let guided = |armor: u16, controllers: &[&str]| {
            let entries: String = controllers
                .iter()
                .map(|entry| format!("\n    {entry},"))
                .collect();
            parse(
                &ZHUKOV
                    .replace(
                        "armor = 44\nslots = [\n    { at = 1",
                        &format!("armor = {armor}\nslots = [\n    {{ at = 1"),
                    )
                    .replace("rounds = 15 },", &format!("rounds = 15 }},{entries}")),
            )
        };
        let valid = guided(28, &[r#"{ at = 3, item = "ArtemisIV", link = 1 }"#]);
        valid.construction_rules().unwrap();
        let refused = rejection(&guided(28, &[r#"{ at = 3, item = "ArtemisIV" }"#]));
        assert!(
            refused.contains("The Artemis IV in Front_Side slot 3 has no launcher link"),
            "{refused}"
        );
        let refused = rejection(&guided(
            28,
            &[r#"{ at = 3, item = "ArtemisIV", link = 2 }"#],
        ));
        assert!(
            refused.contains(
                "The Artemis IV in Front_Side slot 3 links slot 2, which is not the first slot of an LRM, SRM or MML launcher in Front_Side"
            ),
            "{refused}"
        );
        // Turret slot 1 holds the autocannon; the SRM-6 in front slot 1 is in another section.
        let refused = rejection(&parse(&ZHUKOV.replace(
            "rounds = 10 },",
            "rounds = 10 },\n    { at = 5, item = \"ArtemisIV\", link = 1 },",
        )));
        assert!(
            refused.contains("The Artemis IV in Turret slot 5 links slot 1, which is not the first slot of an LRM, SRM or MML launcher in Turret"),
            "{refused}"
        );
        let refused = rejection(&guided(
            12,
            &[
                r#"{ at = 3, item = "ArtemisIV", link = 1 }"#,
                r#"{ at = 4, item = "ArtemisIV", link = 1 }"#,
            ],
        ));
        assert!(
            refused.contains("2 fire-control entries link the")
                && refused.contains("in Front_Side slot 1"),
            "{refused}"
        );
        // A second SRM-6 in the turret without its own controller breaks all-or-none.
        let refused = rejection(&parse(
            &ZHUKOV
                .replace(
                    "rounds = 15 },",
                    "rounds = 15 },\n    { at = 3, item = \"ArtemisIV\", link = 1 },",
                )
                .replace(
                    "rounds = 10 },",
                    "rounds = 10 },\n    { at = 5, item = \"IS.SRM-6\" },",
                ),
        ));
        assert!(
            refused.contains("no controller links the") && refused.contains("in Turret slot 5"),
            "{refused}"
        );
    }

    /// A machine gun array links two to four guns of one class beside it in its location and
    /// takes an item slot (TechManual p. 228).
    #[test]
    fn machine_gun_arrays_link_two_to_four_guns() {
        let guns = |count: u8| {
            parse(&YELLOW_JACKET.replace(
                "{ at = 1, item = \"IS.GaussRifle\" },\n    { at = \"2-3\", item = \"Ammo_IS.GaussRifle\", rounds = 8 },",
                &format!(
                    "{{ at = \"1-{count}\", item = \"IS.MachineGun\" }},\n    {{ at = {}, item = \"IS.MGA\", links = {:?} }},\n    {{ at = 6, item = \"Ammo_IS.MachineGun\", rounds = 200 }},",
                    count + 1,
                    (1..=count).collect::<Vec<_>>()
                ),
            ))
        };
        let three = guns(3);
        let report = three.construction_rules().unwrap();
        // Three guns, the array and one ammunition type.
        assert_eq!(report.item_slots, 5);
        let arrays = VehicleLoadout::resolve(&three)
            .unwrap()
            .machine_gun_arrays();
        assert_eq!(arrays.len(), 1);
        assert_eq!(arrays[0].weapon_indices.len(), 3);
        assert!(rejection(&guns(1)).contains("links 1 slots"));
    }
}
