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

/// Whether a resolved system claims an item slot. Heat sinks sit deep in the body and take no
/// space (TechManual p. 104); armor, structure and actuator placeholders are chassis facts
/// whose slot cost comes from the chassis choice itself (TechManual pp. 101, 105).
pub fn occupies_vehicle_slot(system: System) -> bool {
    !matches!(
        system,
        System::ShoulderOrHip
            | System::UpperActuator
            | System::LowerActuator
            | System::HandOrFootActuator
            | System::Engine
            | System::Gyro
            | System::Cockpit
            | System::LifeSupport
            | System::Sensors
            | System::HeatSink
            | System::FuelTank
            | System::FerroFibrous
            | System::EndoSteel
            | System::HeavyFerroFibrous
            | System::LightFerroFibrous
            | System::StealthArmor
            | System::LaserReflective
            | System::NullSignature
            | System::TripleStrengthMyomer
    )
}

impl VehicleTemplate {
    /// Item slots the chassis offers: 5 plus 1 per 5 full tons (TechManual p. 98), less the
    /// slots an XL or light engine claims and plus the one a compact engine frees (TechManual
    /// p. 101, Combat Vehicle Engine Space Table), less the slots ferro-fibrous armor claims
    /// (TechManual p. 105, Combat Vehicle Armor Table) and the slots the advanced armors claim
    /// in the CV column of the Advanced Armor Table: 1 for hardened and for Inner Sphere or Clan
    /// laser-reflective armor, 2 for vehicular stealth armor (Tactical Operations: Advanced
    /// Units & Equipment p. 92).
    pub fn item_slot_limit(&self) -> u16 {
        let base = 5 + i32::from(self.tons) / 5;
        let engine = match self.engine_family() {
            Some(Engine::Xl) if self.clan_engine() => -1,
            Some(Engine::Xl) => -2,
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
    /// `cargo_space` describes (TechManual pp. 98, 106), one per other type of transport bay and
    /// one per cargo container (TechManual pp. 107, 239). Heat sinks and power amplifiers take
    /// none (TechManual pp. 104, 106).
    pub fn item_slots(&self, loadout: &VehicleLoadout) -> u16 {
        let weapons = loadout.weapons.len();
        let systems = loadout
            .systems
            .iter()
            .filter(|part| occupies_vehicle_slot(part.system))
            .count();
        let mut ammunition: Vec<(Weapon, AmmunitionMode)> = Vec::new();
        for bin in &loadout.ammunition {
            if !ammunition.contains(&(bin.weapon, bin.mode)) {
                ammunition.push((bin.weapon, bin.mode));
            }
        }
        let cargo = self
            .attributes
            .get("cargo_space")
            .and_then(|space| space.parse::<u32>().ok())
            .is_some_and(|space| space > 0);
        let bays = self
            .transport_bays()
            .map_or(0, |bays| bays.item_slots() as usize);
        let total = weapons + systems + ammunition.len() + usize::from(cargo) + bays;
        u16::try_from(total).unwrap_or(u16::MAX)
    }

    /// Heat of every energy weapon firing at once: the fewest heat sinks the vehicle may carry
    /// (TechManual p. 104).
    pub fn energy_heat(loadout: &VehicleLoadout) -> u32 {
        loadout
            .weapons
            .iter()
            .filter(|mount| mount.weapon.is_energy())
            .map(|mount| u32::from(mount.weapon.profile().heat))
            .sum()
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
    /// base on a vehicle that does not declare mixed technology (TechManual p. 46). A VTOL's
    /// turret is a chin turret, which follows the standard turret rules (TacOps AUE p. 160).
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
            weapons: loadout.weapons.iter().map(|mount| mount.weapon),
            systems: loadout.systems.iter().map(|part| part.system),
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
        for part in &loadout.systems {
            match part.system {
                System::JumpJet => {
                    bail!("Combat vehicles cannot mount jump jets (TechManual p. 102)")
                }
                System::Masc => bail!("Combat vehicles cannot mount MASC (TechManual p. 103)"),
                _ => {}
            }
        }
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

    /// Ferro-fibrous armor and XL engines claim item slots; a compact engine frees one.
    #[test]
    fn item_slots_follow_engine_and_armor_choices() {
        let jacket = parse(YELLOW_JACKET);
        for (specials, limit) in [
            ("ICEEngine_Tech", 11),
            ("XLEngine_Tech", 9),
            ("Clan XLEngine_Tech", 10),
            ("LightEngine_Tech", 10),
            ("CompactEngine_Tech", 12),
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
    /// p. 239), alongside the standard cargo bay `cargo_space` describes, and survive a
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
                .replace(
                    "walk_mp = 3\n",
                    "walk_mp = 3\ncargo_space = 100\nspecials = [\"CargoTech\"]\n",
                ),
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

    /// ICE vehicles carry power amplifiers for energy weapons; fusion vehicles do not, and
    /// Clan CASE weighs nothing.
    #[test]
    fn amplifiers_and_case_follow_the_engine_and_technology_base() {
        let lasers = YELLOW_JACKET.replace(
            "{ at = 1, item = \"IS.GaussRifle\" },",
            "{ at = 1, item = \"IS.LargeLaser\" },\n    { at = 4, item = \"CASE\" },",
        );
        let ice = parse(&lasers);
        assert_eq!(ice.mass().unwrap().power_amplifiers, 512);
        assert_eq!(ice.system_mass(System::Case), 512);
        let fusion = parse(&lasers.replace("engine = \"ice\"", "tech_base = \"clan\""));
        assert_eq!(fusion.mass().unwrap().power_amplifiers, 0);
        assert_eq!(fusion.system_mass(System::Case), 0);
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
}
