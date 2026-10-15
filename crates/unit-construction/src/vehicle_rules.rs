//! Combat vehicle and VTOL construction rules from TechManual pp. 95-107: motive-type tonnage
//! limits, structure and engine choices, heat sinks, the Maximum Armor Factor, item slots and
//! the tonnage budget. Stationary installations are not combat vehicles and only pass anatomy
//! and mass arithmetic.
use super::{
    AmmunitionMode, Engine, System, Technology, VehicleLoadout, VehicleMass, VehicleMovement,
    VehiclePowerplant, VehicleSection, VehicleTemplate, Weapon,
    vehicle_engine::VEHICLE_MAXIMUM_ENGINE_RATING,
};
use anyhow::{Result, bail, ensure};
use serde::Serialize;

/// Most armor points a VTOL rotor may carry (TechManual p. 105).
pub const ROTOR_MAXIMUM_ARMOR: u16 = 2;

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
    /// (TechManual p. 105, Combat Vehicle Armor Table).
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
        } else if self.has_special("LtFerroFibrous_Tech") {
            -1
        } else {
            0
        };
        u16::try_from((base + engine + armor).max(0)).unwrap_or(0)
    }

    /// Item slots the loadout occupies: one per weapon and per equipment item, one per
    /// ammunition type however many tons it holds, and one for a cargo bay (TechManual
    /// pp. 98, 106). Heat sinks and power amplifiers take none (TechManual pp. 104, 106).
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
        let total = weapons + systems + ammunition.len() + usize::from(cargo);
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
            warnings: Vec::new(),
        };
        if self.movement == VehicleMovement::Stationary {
            return Ok(report);
        }
        self.check_chassis()?;
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

    /// Equipment the motive type or location forbids. A VTOL's turret is a chin turret, which
    /// follows the standard turret rules (TacOps AUE p. 160).
    fn check_equipment(&self, loadout: &VehicleLoadout) -> Result<()> {
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
walk_mp = 9

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
walk_mp = 5

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
        fast.max_speed = 9.0 * 10.75;
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
        ] {
            let mut template = jacket.clone();
            template
                .attributes
                .insert("specials".into(), specials.into());
            assert_eq!(template.item_slot_limit(), limit, "{specials}");
        }
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
        template.max_speed = 14.0 * 10.75;
        assert_eq!(template.engine().unwrap().weight_rating, 50);
        // A 65-ton tank at 2 cruising MP has a 130 rating: the XL engine lists 2.5 tons and
        // weighs 4 with shielding, where shielding the standard weight first would give 3.5.
        let mut tank = parse(ZHUKOV);
        tank.attributes
            .insert("specials".into(), "XLEngine_Tech".into());
        tank.tons = 65;
        tank.max_speed = 3.0 * 10.75;
        let engine = tank.engine().unwrap();
        assert_eq!(engine.weight_rating, 130);
        assert_eq!(engine.engine_mass, 4 * 1024);
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
