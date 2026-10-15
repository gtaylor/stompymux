//! Battle Value 2.0 (TechManual pp. 302-318; Tactical Operations: Advanced Units & Equipment
//! pp. 193-197) for 'Mechs and combat vehicles.
//!
//! One rule core, [`Valuation::evaluate`], computes the Defensive and Offensive Battle Ratings
//! from chassis-neutral facts. [`Mech`] and [`super::Vehicle`] gather those facts from their
//! design: the installed equipment, construction technologies and maximum legal movement.
//! Live units pass their current armor and internal structure, so damage lowers the defensive
//! rating, while installed weapons, ammunition and equipment keep their value after damage or
//! expenditure. Templates are valued by constructing the pristine unit they describe.
use super::{
    ArmAttack, CriticalLocation, Engine, Gyro, Mech, MechChassis, MechSection, PhysicalBattleValue,
    ResolvedLoadout, SpeedEnvelope, System, SystemContext, Technology, VehicleCriticalLocation,
    VehicleMovement, VehicleSection, Weapon, WeaponSettings,
};
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeMap;

/// A unit's Battle Value for the standard Gunnery 4 / Piloting 5 crew.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BattleValue {
    /// Offensive Battle Rating: the Weapon Battle Rating times the Speed Factor, with fractions.
    pub offensive: f64,
    /// Defensive Battle Rating: the defensive total times the Defensive Factor, with fractions.
    pub defensive: f64,
    /// Final Battle Value: both ratings summed, times any cockpit modifier, rounded to the
    /// nearest whole number with .5 rounding up (TechManual p. 303).
    pub total: u32,
}

impl BattleValue {
    /// The Battle Value of this unit crewed at other skill ratings: the final value times the
    /// BV Skill Multiplier Table entry, rounded to the nearest whole number (TechManual p. 314).
    pub fn with_skill(self, gunnery: i16, piloting: i16) -> u32 {
        round_half_up(f64::from(self.total) * skill_multiplier(gunnery, piloting))
    }
}

/// BV Skill Multiplier Table (TechManual p. 314), rows by Gunnery 0-8 and columns by
/// Piloting 0-8. Ratings outside the table use its nearest edge.
const SKILL_MULTIPLIERS: [[f64; 9]; 9] = [
    [2.42, 2.31, 2.21, 2.10, 1.93, 1.75, 1.68, 1.59, 1.50],
    [2.21, 2.11, 2.02, 1.92, 1.76, 1.60, 1.54, 1.46, 1.38],
    [1.93, 1.85, 1.76, 1.68, 1.54, 1.40, 1.35, 1.28, 1.21],
    [1.66, 1.58, 1.51, 1.44, 1.32, 1.20, 1.16, 1.10, 1.04],
    [1.38, 1.32, 1.26, 1.20, 1.10, 1.00, 0.95, 0.90, 0.85],
    [1.31, 1.19, 1.13, 1.08, 0.99, 0.90, 0.86, 0.81, 0.77],
    [1.24, 1.12, 1.07, 1.02, 0.94, 0.85, 0.81, 0.77, 0.72],
    [1.17, 1.06, 1.01, 0.96, 0.88, 0.80, 0.76, 0.72, 0.68],
    [1.10, 0.99, 0.95, 0.90, 0.83, 0.75, 0.71, 0.68, 0.64],
];

/// The BV Skill Multiplier for a crew's Gunnery and Piloting ratings (TechManual p. 314).
/// Gunnery 4 / Piloting 5 is 1.0. Ratings below 0 or above 8 use the table's edge.
pub fn skill_multiplier(gunnery: i16, piloting: i16) -> f64 {
    let index = |rating: i16| rating.clamp(0, 8) as usize;
    SKILL_MULTIPLIERS[index(gunnery)][index(piloting)]
}

/// Round a non-negative rating to the nearest whole number, .5 up (TechManual p. 303).
fn round_half_up(value: f64) -> u32 {
    (value.max(0.0) + 0.5).floor() as u32
}

/// Target movement modifier for movement points spent in a turn (Total Warfare p. 115), the
/// basis of the Defensive Factor (TechManual p. 315).
fn target_movement_modifier(mp: u32) -> u32 {
    match mp {
        0..=2 => 0,
        3..=4 => 1,
        5..=6 => 2,
        7..=9 => 3,
        10..=17 => 4,
        18..=24 => 5,
        _ => 6,
    }
}

/// Speed Factor for a movement total (TechManual p. 315): (1 + (MP - 5) / 10) ^ 1.2, rounded to
/// two decimal places. The formula reproduces every row of the printed table.
pub fn speed_factor(mp: u32) -> f64 {
    let raw = (1.0 + (f64::from(mp) - 5.0) / 10.0).powf(1.2);
    (raw * 100.0).round() / 100.0
}

/// Whole movement points in a speed, tolerating floating-point error in stored speeds.
fn whole_mp(kph: f64) -> u32 {
    (kph / super::MP_KPH + 1e-6).floor().max(0.0) as u32
}

/// One offensive weapon after its BV modifiers, ready for the heat-efficiency ordering.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ValuedWeapon {
    /// Modified BV: arc halving, targeting computer and Artemis applied (TechManual p. 302).
    value: f64,
    /// Heat counted toward Total Weapon Heat, with the Ultra, Rotary, Streak and one-shot
    /// adjustments applied (TechManual p. 302).
    heat: f64,
}

/// How a unit's chassis enters the rules that differ between 'Mechs and vehicles.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Chassis {
    /// 'Mech: gyro value in the defensive total and heat-efficiency weapon halving.
    Mech { gyro: f64, heat_efficiency: f64 },
    /// Vehicle: the Unit Type Modifier (TechManual p. 316) and no heat halving.
    Vehicle { unit_type: f64 },
}

/// Chassis-neutral facts the Battle Value rules read (TechManual pp. 302-306).
#[derive(Debug, Clone, PartialEq)]
struct Valuation {
    chassis: Chassis,
    /// Armor points times the armor type modifier.
    armor: f64,
    /// Internal structure points times the structure and engine type modifiers.
    structure: f64,
    /// Defensive equipment BV, including AMS ammunition up to the AMS value.
    defensive_equipment: f64,
    /// Explosive ammunition and Gauss weapon penalties (15 per ammunition slot, 1 per Gauss slot).
    explosive_penalty: f64,
    /// Highest potential target modifier, including jump, VTOL and stealth bonuses.
    target_modifier: u32,
    /// Offensive weapons with their modified BV and heat.
    weapons: Vec<ValuedWeapon>,
    /// Ammunition BV after the excessive-ammunition cap.
    ammunition: f64,
    /// Offensive equipment and physical weapons, which generate no heat.
    equipment: f64,
    /// The tonnage term of the Weapon Battle Rating: tons (x1.5 with TSM) for 'Mechs, tons / 2
    /// for vehicles.
    tonnage: f64,
    /// Movement total for the Speed Factor: running MP plus half jumping MP rounded up for
    /// 'Mechs, flanking MP for vehicles.
    speed_mp: u32,
    /// Final multiplier: 0.95 for a small cockpit (TechManual p. 303).
    final_multiplier: f64,
}

impl Valuation {
    /// Defensive Battle Rating (TechManual p. 302; vehicles p. 306).
    fn defensive(&self) -> f64 {
        let mut total = self.armor * 2.5 + self.structure * 1.5 + self.defensive_equipment;
        if let Chassis::Mech { gyro, .. } = self.chassis {
            total += gyro;
        }
        // The subtractions cannot drop the running total below 1 (nor raise a smaller one).
        total = (total - self.explosive_penalty).max(total.min(1.0));
        if let Chassis::Vehicle { unit_type } = self.chassis {
            total *= unit_type;
        }
        total * (1.0 + f64::from(self.target_modifier) / 10.0)
    }

    /// Weapon Battle Rating (TechManual p. 303 steps 1-8; vehicles p. 306).
    fn weapon_rating(&self) -> f64 {
        let mut total = self.ammunition + self.equipment + self.tonnage;
        let Chassis::Mech {
            heat_efficiency, ..
        } = self.chassis
        else {
            return total + self.weapons.iter().map(|weapon| weapon.value).sum::<f64>();
        };
        // Step 1: weapons that generate no heat count in full.
        total += self
            .weapons
            .iter()
            .filter(|weapon| weapon.heat <= 0.0)
            .map(|weapon| weapon.value)
            .sum::<f64>();
        let mut heated: Vec<_> = self
            .weapons
            .iter()
            .filter(|weapon| weapon.heat > 0.0)
            .copied()
            .collect();
        // Step 3: within the heat efficiency, every weapon counts in full.
        if heated.iter().map(|weapon| weapon.heat).sum::<f64>() <= heat_efficiency {
            return total + heated.iter().map(|weapon| weapon.value).sum::<f64>();
        }
        // Steps 4-7: highest modified BV first, ties by least heat. The weapon that reaches or
        // exceeds the efficiency keeps its full value; every later weapon counts half.
        heated.sort_by(|left, right| {
            right
                .value
                .total_cmp(&left.value)
                .then(left.heat.total_cmp(&right.heat))
        });
        let mut heat = 0.0;
        for weapon in heated {
            if heat < heat_efficiency {
                total += weapon.value;
                heat += weapon.heat;
            } else {
                total += weapon.value / 2.0;
            }
        }
        total
    }

    /// Offensive Battle Rating: the Weapon Battle Rating times the Speed Factor.
    fn offensive(&self) -> f64 {
        self.weapon_rating() * speed_factor(self.speed_mp)
    }

    /// Combine both ratings into the final Battle Value (TechManual p. 303).
    fn evaluate(&self) -> BattleValue {
        let defensive = self.defensive();
        let offensive = self.offensive();
        BattleValue {
            offensive,
            defensive,
            total: round_half_up((defensive + offensive) * self.final_multiplier),
        }
    }
}

/// Firing arc for the rear-firing weapon rule (TechManual pp. 302 and 306).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arc {
    /// Arm, side and turret mounts, which the arc rule never halves.
    Unaffected,
    /// Forward-firing torso, leg and head mounts, or a vehicle's front.
    Forward,
    /// Rear-firing torso, leg and head mounts, or a vehicle's rear.
    Rear,
}

/// The facts the weapon BV modifiers read from one installed weapon.
#[derive(Debug, Clone, Copy)]
struct MountFacts {
    weapon: Weapon,
    arc: Arc,
    one_shot: bool,
    targeting_computer: bool,
    /// 1.2 with Artemis IV, 1.3 with Artemis V (TO:AUE p. 196), otherwise 1.0.
    artemis: f64,
}

/// Unmodified BV of one mount: the runtime weapon value, or for a one-shot launcher the
/// published one-shot figure (TechManual p. 317). Rocket launchers are one-shot by design and
/// carry their own value.
fn unmodified_value(settings: &WeaponSettings, weapon: Weapon, one_shot: bool) -> f64 {
    if one_shot && let Some(value) = weapon.one_shot_battle_value() {
        return f64::from(value);
    }
    f64::from(settings.battle_value(weapon))
}

/// Heat a weapon adds to Total Weapon Heat: Ultra x2, Rotary x6, Streak x1/2 and one-shot
/// x1/4 (TechManual p. 302).
fn weapon_heat(weapon: Weapon, one_shot: bool) -> f64 {
    let mut heat = f64::from(weapon.profile().heat);
    if weapon.is_ultra() {
        heat *= 2.0;
    } else if weapon.is_rotary() {
        heat *= 6.0;
    } else if weapon.is_streak() {
        heat /= 2.0;
    }
    if one_shot || weapon.is_rocket() {
        heat /= 4.0;
    }
    heat
}

/// Offensive weapon values and the unmodified BV of each weapon model, which caps that
/// model's ammunition (TechManual p. 302).
struct WeaponValues {
    weapons: Vec<ValuedWeapon>,
    models: BTreeMap<Weapon, f64>,
}

/// Apply the weapon modifiers of TechManual p. 302: rear-firing halving (or forward halving
/// when rear-firing weapons are worth more), targeting computer x1.25 and Artemis.
fn value_weapons(settings: &WeaponSettings, mounts: &[MountFacts]) -> WeaponValues {
    let arc_total = |arc| {
        mounts
            .iter()
            .filter(|mount| mount.arc == arc)
            .map(|mount| unmodified_value(settings, mount.weapon, mount.one_shot))
            .sum::<f64>()
    };
    let halved = if arc_total(Arc::Forward) < arc_total(Arc::Rear) {
        Arc::Forward
    } else {
        Arc::Rear
    };
    let mut values = WeaponValues {
        weapons: Vec::new(),
        models: BTreeMap::new(),
    };
    for mount in mounts {
        let unmodified = unmodified_value(settings, mount.weapon, mount.one_shot);
        *values.models.entry(mount.weapon).or_default() += unmodified;
        let mut value = unmodified * mount.artemis;
        if mount.targeting_computer {
            value *= 1.25;
        }
        if mount.arc == halved {
            value /= 2.0;
        }
        values.weapons.push(ValuedWeapon {
            value,
            heat: weapon_heat(mount.weapon, mount.one_shot),
        });
    }
    values
}

/// Offensive ammunition BV: full-ton value per bin (half for half-ton bins), with each weapon
/// model's ammunition capped at that model's unmodified weapon BV (TechManual p. 302).
fn ammunition_value(
    bins: impl Iterator<Item = (Weapon, bool)>,
    models: &BTreeMap<Weapon, f64>,
) -> f64 {
    let mut by_model: BTreeMap<Weapon, f64> = BTreeMap::new();
    for (weapon, half_ton) in bins {
        let per_ton = f64::from(weapon.ammunition_battle_value());
        *by_model.entry(weapon).or_default() += if half_ton { per_ton / 2.0 } else { per_ton };
    }
    by_model
        .into_iter()
        .map(|(weapon, value)| value.min(models.get(&weapon).copied().unwrap_or(0.0)))
        .sum()
}

/// Defensive weapon BV plus anti-missile ammunition up to the value of the matching
/// anti-missile systems (TechManual p. 302).
fn defensive_weapon_value(
    settings: &WeaponSettings,
    weapons: impl Iterator<Item = Weapon>,
    bins: impl Iterator<Item = (Weapon, bool)>,
) -> f64 {
    let mut systems: BTreeMap<Weapon, f64> = BTreeMap::new();
    for weapon in weapons.filter(|weapon| weapon.is_defensive_equipment()) {
        *systems.entry(weapon).or_default() += f64::from(settings.battle_value(weapon));
    }
    let ammunition = ammunition_value(bins.filter(|(weapon, _)| weapon.is_ams()), &systems);
    systems.values().sum::<f64>() + ammunition
}

/// Armor type modifier (TechManual p. 315; TO:AUE p. 193): hardened 2.0, laser-reflective 1.5,
/// every other supported armor 1.0.
fn armor_type_modifier(has: impl Fn(Technology) -> bool) -> f64 {
    if has(Technology::HardenedArmor) {
        2.0
    } else if has(Technology::LaserReflectiveArmor) {
        1.5
    } else {
        1.0
    }
}

/// BV of a physical weapon from its catalogue entry (TechManual p. 317; TO:AUE pp. 195-197).
/// Damage-based values use the weapon's damage after triple-strength myomer doubling.
fn physical_weapon_value(attack: ArmAttack, tons: u16, triple_myomer: bool) -> f64 {
    let Some(value) = attack.system().and_then(System::physical_battle_value) else {
        return 0.0;
    };
    match value {
        PhysicalBattleValue::Fixed(value) => f64::from(value),
        PhysicalBattleValue::DamageTimes(factor) => {
            let damage = f64::from(attack.damage(tons));
            let doubled = triple_myomer && attack.myomer_doubles();
            factor * if doubled { damage * 2.0 } else { damage }
        }
    }
}

/// Defensive equipment BV of the installed systems: each complete installation's catalogue
/// value (TechManual pp. 317-318; TO:AUE pp. 195-197). `slots` gives the slots one
/// installation fills.
fn defensive_system_value<L>(
    loadout: &ResolvedLoadout<L>,
    context: SystemContext,
    slots: impl Fn(System, SystemContext) -> u16,
) -> f64 {
    // Each installation is valued and sized by its own technology base.
    let mut counts: BTreeMap<(System, Option<super::TechBase>), usize> = BTreeMap::new();
    for part in &loadout.systems {
        let context = context.for_installation(part.tech_base);
        if part.system.defensive_battle_value(context).is_some() {
            *counts.entry((part.system, part.tech_base)).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|((system, base), count)| {
            let context = context.for_installation(base);
            let value = system.defensive_battle_value(context).unwrap_or(0);
            let size = usize::from(slots(system, context).max(1));
            (count / size) as f64 * f64::from(value)
        })
        .sum()
}

/// Launcher multipliers by weapon index: each Artemis controller's catalogue multiplier for
/// the launchers it is linked to (TechManual p. 302; TO:AUE p. 197).
fn artemis_multipliers<L: Copy + PartialEq>(
    loadout: &ResolvedLoadout<L>,
    controllers: &[super::ArtemisController<L>],
) -> BTreeMap<usize, f64> {
    let mut multipliers = BTreeMap::new();
    for controller in controllers {
        let Some(multiplier) = loadout
            .systems
            .iter()
            .find(|part| part.location == controller.location)
            .and_then(|part| part.system.launcher_battle_value_multiplier())
        else {
            continue;
        };
        for &index in &controller.weapon_indices {
            multipliers.insert(index, multiplier);
        }
    }
    multipliers
}

impl Mech {
    /// Battle Value with catalogue weapon values. Armor and internal structure are current;
    /// installed weapons, ammunition, equipment, heat sinks and movement keep their design value
    /// after damage or expenditure.
    pub fn battle_value(&self) -> Result<BattleValue> {
        self.battle_value_with(&WeaponSettings::default())
    }

    /// Battle Value with the runtime weapon values.
    pub(crate) fn battle_value_with(&self, settings: &WeaponSettings) -> Result<BattleValue> {
        Ok(self.valuation(settings)?.evaluate())
    }

    /// Gather the 'Mech facts of TechManual pp. 302-303.
    fn valuation(&self, settings: &WeaponSettings) -> Result<Valuation> {
        let definition = self.definition();
        let loadout = self.loadout()?;
        let clan = definition.has_special("Clan");
        let quad = self.chassis() == MechChassis::Quad;
        let tons = definition.tons;
        let engine = self.engine().unwrap_or_else(|_| {
            Engine::display_from_flags(
                definition.has_special("LightEngine_Tech"),
                definition.has_special("CompactEngine_Tech"),
                definition.has_special("XXL_Tech"),
                definition.has_special("XLEngine_Tech"),
            )
        });
        let clan_engine = definition.clan_engine();
        // 'Mech Engine Type Modifier (TechManual p. 315; TO:AUE p. 193).
        let engine_modifier = match engine {
            Engine::Standard | Engine::Compact => 1.0,
            Engine::Light => 0.75,
            Engine::Xl if clan_engine => 0.75,
            Engine::Xl => 0.5,
            Engine::Xxl if clan_engine => 0.5,
            Engine::Xxl => 0.25,
        };
        // 'Mech Internal Structure Type Modifier (TechManual p. 315; TO:AUE p. 194).
        let structure_modifier = if definition.has_technology(Technology::ReinforcedStructure) {
            2.0
        } else if definition.has_technology(Technology::CompositeStructure) {
            0.5
        } else {
            1.0
        };
        let armor_modifier =
            armor_type_modifier(|technology| definition.has_technology(technology));
        // A torso-mounted cockpit counts the center torso's armor, front and rear, twice
        // (Tactical Operations: Advanced Units & Equipment p. 193).
        let torso_cockpit = self.torso_mounted_cockpit();
        let armor: u32 = self
            .sections()
            .iter()
            .map(|(&section, state)| {
                let points = u32::from(state.armor) + u32::from(state.rear);
                if torso_cockpit && section == MechSection::CenterTorso {
                    points * 2
                } else {
                    points
                }
            })
            .sum();
        let structure: u32 = self
            .sections()
            .values()
            .map(|section| u32::from(section.internal))
            .sum();
        // 'Mech Gyro Modifier (TechManual p. 315): heavy-duty 1.0, every other gyro 0.5.
        let gyro = f64::from(tons)
            * if self.gyro() == Gyro::Hardened {
                1.0
            } else {
                0.5
            };

        // Defensive equipment (TechManual pp. 317-318; TO:AUE pp. 195-197), counted by
        // complete installations.
        let context = SystemContext {
            clan,
            watchdog: definition.has_technology(Technology::Watchdog),
            tons,
            engine_rating: 0,
            legs: if quad { 4 } else { 2 },
        };
        let defensive_equipment = defensive_system_value(&loadout, context, |system, context| {
            system.installation_slots(context).unwrap_or(1)
        }) + defensive_weapon_value(
            settings,
            loadout.weapons.iter().map(|mount| mount.weapon),
            loadout
                .ammunition
                .iter()
                .map(|bin| (bin.weapon, bin.half_ton)),
        );

        // Explosive components (TechManual p. 302; TO:AUE p. 193 for CASE II and Clan XXL).
        let has = |system, section| {
            loadout
                .systems
                .iter()
                .any(|part| part.system == system && part.location.section == section)
        };
        let inward = |section| match section {
            MechSection::LeftArm if !quad => Some(MechSection::LeftTorso),
            MechSection::RightArm if !quad => Some(MechSection::RightTorso),
            _ => None,
        };
        let core = |section| match section {
            MechSection::CenterTorso
            | MechSection::Head
            | MechSection::LeftLeg
            | MechSection::RightLeg => true,
            MechSection::LeftArm | MechSection::RightArm => quad,
            MechSection::LeftTorso | MechSection::RightTorso => false,
        };
        let protected_by = |system, section| {
            has(system, section) || inward(section).is_some_and(|torso| has(system, torso))
        };
        // Inner Sphere XL engines and every XXL engine are lost with a side torso.
        let fragile_engine = (engine == Engine::Xl && !clan_engine) || engine == Engine::Xxl;
        let exposed = |section| {
            if protected_by(System::CaseIi, section) {
                return false;
            }
            if fragile_engine || core(section) {
                return true;
            }
            // Clan 'Mechs are assumed to carry CASE in every location.
            !clan && !protected_by(System::Case, section)
        };
        let ammunition_slots = loadout
            .ammunition
            .iter()
            .filter(|bin| bin.weapon.ammunition_is_explosive() && exposed(bin.location.section))
            .count();
        // Gauss weapons count every slot; hyper-velocity autocannons count one (TO:AUE p. 196).
        let gauss_slots = loadout
            .weapons
            .iter()
            .filter_map(|mount| {
                let slots = mount.weapon.defensive_gauss_slots()?;
                Some(mount.criticals.iter().take(usize::from(slots)))
            })
            .flatten()
            .filter(|location| exposed(location.section))
            .count();
        let explosive_penalty = ammunition_slots as f64 * 15.0 + gauss_slots as f64;

        // Maximum legal movement (TechManual p. 303). Hot triple-strength myomer adds two
        // walking MP while the nine points of heat that activate it cost one (Total Warfare
        // pp. 143 and 159); MASC and superchargers run at twice the walking MP, both together
        // at two and a half times.
        let mut envelope = SpeedEnvelope::from_walking_mp(f64::from(definition.walk_mp));
        let triple_myomer = definition.has_triple_myomer();
        if triple_myomer {
            envelope = envelope.with_walking_mp_delta(1.0);
        }
        let boosters = u8::from(self.masc_installed()?) + u8::from(self.supercharger_installed());
        let running = whole_mp(envelope.with_boosters(boosters).running);
        let jumping = whole_mp(definition.jump_speed);
        let stealth = self.has_stealth_armor()?;
        let ground = target_movement_modifier(running);
        let jump = if jumping > 0 {
            target_movement_modifier(jumping) + 1
        } else {
            0
        };
        // Stealth armor and a null signature system add their long-range modifier.
        let concealment = if stealth || self.has_null_signature()? {
            2
        } else {
            0
        };
        let target_modifier = ground.max(jump) + concealment;

        // Heat efficiency: 6 + heat dissipation - movement heat. Running makes 2 heat and
        // jumping 1 per MP with a minimum of 3 (Total Warfare p. 159); stealth armor adds 10.
        let jump_heat = if jumping > 0 { jumping.max(3) } else { 0 };
        let movement_heat = f64::from(jump_heat.max(2)) + if stealth { 10.0 } else { 0.0 };
        let heat_efficiency = 6.0 + f64::from(definition.heat_dissipation()) - movement_heat;

        let offense = Offense::gather(
            settings,
            &loadout,
            artemis_multipliers(
                &loadout,
                &[self.artemis_controllers()?, self.apollo_controllers()?].concat(),
            ),
            |location: CriticalLocation, rear_mount| match location.section {
                MechSection::LeftArm | MechSection::RightArm if !quad => Arc::Unaffected,
                _ if rear_mount => Arc::Rear,
                _ => Arc::Forward,
            },
        );
        let mut physical = 0.0;
        if !quad {
            for arm in [MechSection::LeftArm, MechSection::RightArm] {
                for attack in ArmAttack::HAND_WEAPONS {
                    if attack.system().is_some_and(|system| has(system, arm)) {
                        physical += physical_weapon_value(attack, tons, triple_myomer);
                    }
                }
            }
        }
        // Talons are worth their kick's final damage, after triple strength myomer and the
        // talons' own half again (Tactical Operations: Advanced Units & Equipment p. 196).
        if loadout
            .systems
            .iter()
            .any(|part| part.system == System::Talons)
        {
            let kick = tons / 5 * if triple_myomer { 2 } else { 1 };
            physical += f64::from(super::talons::raise(kick));
        }
        // A machine gun array is worth the BV of the guns it links times 0.67 (TechManual
        // p. 317, note F).
        for array in loadout.machine_gun_arrays() {
            let guns: u32 = array
                .weapon_indices
                .iter()
                .map(|&index| u32::from(loadout.weapons[index].weapon.battle_value()))
                .sum();
            physical += 0.67 * f64::from(guns);
        }
        Ok(Valuation {
            chassis: Chassis::Mech {
                gyro,
                heat_efficiency,
            },
            armor: f64::from(armor) * armor_modifier,
            structure: f64::from(structure) * structure_modifier * engine_modifier,
            defensive_equipment,
            explosive_penalty,
            target_modifier,
            weapons: offense.weapons,
            ammunition: offense.ammunition,
            equipment: physical,
            tonnage: f64::from(tons) * if triple_myomer { 1.5 } else { 1.0 },
            speed_mp: running + jumping.div_ceil(2),
            // Small and torso-mounted cockpits multiply the final value by 0.95 (TechManual
            // p. 303; Tactical Operations: Advanced Units & Equipment p. 193).
            final_multiplier: if definition.has_technology(Technology::SmallCockpit)
                || torso_cockpit
            {
                0.95
            } else {
                1.0
            },
        })
    }
}

/// Offensive weapons and ammunition of either chassis, after the weapon modifiers of
/// TechManual p. 302 and the excessive-ammunition cap.
struct Offense {
    weapons: Vec<ValuedWeapon>,
    ammunition: f64,
}

impl Offense {
    /// Value every offensive weapon and its ammunition. `artemis` maps weapon indices to their
    /// Artemis multiplier; `arc` places a weapon from its primary
    /// critical and rear-mount flag. A targeting computer links every direct-fire weapon it
    /// supports and any weapon the template links to it explicitly.
    fn gather<L: Copy>(
        settings: &WeaponSettings,
        loadout: &ResolvedLoadout<L>,
        artemis: BTreeMap<usize, f64>,
        arc: impl Fn(L, bool) -> Arc,
    ) -> Self {
        let targeting_computer = loadout
            .systems
            .iter()
            .any(|part| part.system == System::TargetingComputer);
        let mounts: Vec<_> = loadout
            .weapons
            .iter()
            .enumerate()
            .filter(|(_, mount)| !mount.weapon.is_defensive_equipment())
            .map(|(index, mount)| MountFacts {
                weapon: mount.weapon,
                arc: arc(mount.criticals[0], mount.rear_mount),
                one_shot: mount.one_shot,
                targeting_computer: targeting_computer
                    && (mount.on_targeting_computer || mount.weapon.supports_targeting_computer()),
                artemis: artemis.get(&index).copied().unwrap_or(1.0),
            })
            .collect();
        let values = value_weapons(settings, &mounts);
        let ammunition = ammunition_value(
            loadout
                .ammunition
                .iter()
                .filter(|bin| !bin.weapon.is_ams())
                .map(|bin| (bin.weapon, bin.half_ton)),
            &values.models,
        );
        Self {
            weapons: values.weapons,
            ammunition,
        }
    }
}

impl super::Vehicle {
    /// Battle Value with catalogue weapon values. Armor and internal structure are current;
    /// installed weapons, ammunition, equipment and movement keep their design value after
    /// damage or expenditure.
    pub fn battle_value(&self) -> Result<BattleValue> {
        self.battle_value_with(&WeaponSettings::default())
    }

    /// Battle Value with the runtime weapon values.
    pub(crate) fn battle_value_with(&self, settings: &WeaponSettings) -> Result<BattleValue> {
        Ok(self.valuation(settings)?.evaluate())
    }

    /// Gather the vehicle facts of TechManual p. 306.
    fn valuation(&self, settings: &WeaponSettings) -> Result<Valuation> {
        let definition = self.definition();
        let loadout = self.loadout()?;
        let clan = definition.has_special("Clan");
        let vtol = definition.is_vtol();
        let armor: u32 = self
            .sections()
            .values()
            .map(|section| u32::from(section.armor) + u32::from(section.rear))
            .sum();
        // Rotor and turret internal structure count with the hull's.
        let structure: u32 = self
            .sections()
            .values()
            .map(|section| u32::from(section.internal))
            .sum();
        let armor_modifier =
            armor_type_modifier(|technology| definition.has_technology(technology));
        // Vehicles install each electronic item in a single slot.
        let context = SystemContext {
            clan,
            watchdog: definition.has_technology(Technology::Watchdog),
            tons: definition.tons,
            engine_rating: 0,
            legs: 0,
        };
        let defensive_equipment = defensive_system_value(&loadout, context, |_, _| 1)
            + defensive_weapon_value(
                settings,
                loadout.weapons.iter().map(|mount| mount.weapon),
                loadout
                    .ammunition
                    .iter()
                    .map(|bin| (bin.weapon, bin.half_ton)),
            );
        // Unit Type Modifiers Table (TechManual p. 316).
        let unit_type = match definition.movement {
            VehicleMovement::Tracked => 0.9,
            VehicleMovement::Wheeled => 0.8,
            VehicleMovement::Hover | VehicleMovement::Vtol => 0.7,
            VehicleMovement::Stationary if vtol => 0.7,
            VehicleMovement::Stationary => 1.0,
        };
        // Flanking MP, at its maximum with MASC or a supercharger (TechManual p. 316). A
        // stationary installation has none.
        let boosters = u8::from(
            loadout
                .systems
                .iter()
                .any(|part| part.system == System::Masc),
        ) + u8::from(definition.has_special("SuperCharger_Tech"));
        let flanking = if definition.movement == VehicleMovement::Stationary {
            0
        } else {
            whole_mp(
                SpeedEnvelope::from_walking_mp(f64::from(definition.walk_mp))
                    .with_boosters(boosters)
                    .running,
            )
        };
        // VTOLs add one to their target modifier; vehicular stealth armor adds its long-range
        // modifier as a 'Mech's does (TO:AUE p. 193).
        let target_modifier = target_movement_modifier(flanking)
            + u32::from(vtol)
            + if definition.has_special("StealthArmor_Tech") {
                2
            } else {
                0
            };
        // Turret and side weapons are in no arc and keep their full value (TechManual p. 306).
        let offense = Offense::gather(
            settings,
            &loadout,
            artemis_multipliers(&loadout, &self.artemis_controllers()?),
            |location: VehicleCriticalLocation, _| match location.section {
                VehicleSection::Front => Arc::Forward,
                VehicleSection::Rear => Arc::Rear,
                _ => Arc::Unaffected,
            },
        );
        Ok(Valuation {
            chassis: Chassis::Vehicle { unit_type },
            armor: f64::from(armor) * armor_modifier,
            structure: f64::from(structure),
            defensive_equipment,
            explosive_penalty: 0.0,
            target_modifier,
            weapons: offense.weapons,
            ammunition: offense.ammunition,
            equipment: 0.0,
            tonnage: f64::from(definition.tons) / 2.0,
            speed_mp: flanking,
            final_multiplier: 1.0,
        })
    }
}

/// Battle Value of a live unit with the world's runtime weapon values. Towing, carried load,
/// heat and map conditions do not change it.
pub fn unit_battle_value(world: &crate::World, id: crate::ObjectId) -> Result<BattleValue> {
    let unit = world
        .btech
        .unit(id)
        .ok_or_else(|| anyhow::anyhow!("Unit construction is unavailable"))?;
    super::with_unit!(unit, |unit| unit
        .battle_value_with(world.btech.weapon_settings()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 'Mech valuation with no equipment, for exercising the rule core.
    fn mech(heat_efficiency: f64) -> Valuation {
        Valuation {
            chassis: Chassis::Mech {
                gyro: 0.0,
                heat_efficiency,
            },
            armor: 0.0,
            structure: 0.0,
            defensive_equipment: 0.0,
            explosive_penalty: 0.0,
            target_modifier: 0,
            weapons: Vec::new(),
            ammunition: 0.0,
            equipment: 0.0,
            tonnage: 0.0,
            speed_mp: 5,
            final_multiplier: 1.0,
        }
    }

    fn weapon(value: f64, heat: f64) -> ValuedWeapon {
        ValuedWeapon { value, heat }
    }

    /// Every row of the TechManual p. 315 Speed Factor Table.
    #[test]
    fn speed_factor_matches_the_table() {
        let table = [
            0.44, 0.54, 0.65, 0.77, 0.88, 1.00, 1.12, 1.24, 1.37, 1.50, 1.63, 1.76, 1.89, 2.02,
            2.16, 2.30, 2.44, 2.58, 2.72, 2.86, 3.00, 3.15, 3.29, 3.44, 3.59, 3.74,
        ];
        for (mp, factor) in table.into_iter().enumerate() {
            assert_eq!(speed_factor(mp as u32), factor, "{mp} MP");
        }
    }

    /// Target movement bands of Total Warfare p. 115.
    #[test]
    fn target_movement_bands() {
        for (mp, modifier) in [
            (0, 0),
            (2, 0),
            (3, 1),
            (4, 1),
            (5, 2),
            (6, 2),
            (7, 3),
            (9, 3),
            (10, 4),
            (17, 4),
            (18, 5),
            (24, 5),
            (25, 6),
        ] {
            assert_eq!(target_movement_modifier(mp), modifier);
        }
    }

    /// The skill table's corners, standard crew and clamped edges (TechManual p. 314).
    #[test]
    fn skill_multiplier_table() {
        assert_eq!(skill_multiplier(4, 5), 1.0);
        assert_eq!(skill_multiplier(3, 4), 1.32);
        assert_eq!(skill_multiplier(0, 0), 2.42);
        assert_eq!(skill_multiplier(8, 8), 0.64);
        assert_eq!(skill_multiplier(-3, 12), 1.5);
        let value = BattleValue {
            offensive: 0.0,
            defensive: 0.0,
            total: 1000,
        };
        assert_eq!(value.with_skill(4, 5), 1000);
        assert_eq!(value.with_skill(3, 4), 1320);
    }

    /// The weapon that reaches the heat efficiency keeps full value, later ones count half,
    /// fractions are kept and zero-heat weapons never halve (TechManual p. 303).
    #[test]
    fn heat_efficiency_ordering() {
        let mut valuation = mech(14.0);
        valuation.weapons = vec![
            weapon(46.0, 3.0),
            weapon(46.0, 3.0),
            weapon(46.0, 3.0),
            weapon(46.0, 3.0),
            weapon(39.0, 3.0),
            weapon(5.0, 0.0),
        ];
        // 3 + 3 + 3 + 3 = 12 < 14, the SRM-4 reaches 15 and keeps its value: nothing halves.
        assert_eq!(valuation.weapon_rating(), 46.0 * 4.0 + 39.0 + 5.0);
        valuation.chassis = Chassis::Mech {
            gyro: 0.0,
            heat_efficiency: 11.0,
        };
        // The fourth laser reaches 12 >= 11 and keeps full value; the SRM-4 counts half.
        assert_eq!(valuation.weapon_rating(), 46.0 * 4.0 + 19.5 + 5.0);
        valuation.chassis = Chassis::Vehicle { unit_type: 1.0 };
        assert_eq!(valuation.weapon_rating(), 46.0 * 4.0 + 39.0 + 5.0);
    }

    /// Equal modified BV resolves by the least heat first.
    #[test]
    fn heat_ties_prefer_least_heat() {
        let mut valuation = mech(4.0);
        valuation.weapons = vec![weapon(10.0, 5.0), weapon(10.0, 4.0)];
        assert_eq!(valuation.weapon_rating(), 10.0 + 5.0);
        valuation.weapons = vec![weapon(10.0, 4.0), weapon(10.0, 5.0)];
        assert_eq!(valuation.weapon_rating(), 10.0 + 5.0);
        valuation.chassis = Chassis::Mech {
            gyro: 0.0,
            heat_efficiency: 3.0,
        };
        // The 4-heat weapon is chosen first and crosses 3 on its own.
        assert_eq!(valuation.weapon_rating(), 10.0 + 5.0);
    }

    /// TechManual p. 304: the MON-66 Mongoose is worth 758.
    #[test]
    fn mongoose_worked_example() {
        let valuation = Valuation {
            chassis: Chassis::Mech {
                gyro: 12.5,
                heat_efficiency: 14.0,
            },
            armor: 89.0,
            structure: 43.0,
            defensive_equipment: 10.0,
            target_modifier: 4,
            weapons: vec![
                weapon(46.0, 3.0),
                weapon(46.0, 3.0),
                weapon(46.0, 3.0),
                weapon(9.0, 1.0),
            ],
            tonnage: 25.0,
            speed_mp: 12,
            ..mech(0.0)
        };
        let value = valuation.evaluate();
        assert!((value.defensive - 433.3).abs() < 1e-9);
        assert!((value.offensive - 325.08).abs() < 1e-9);
        assert_eq!(value.total, 758);
    }

    /// TechManual p. 304: the TDR-10SE Thunderbolt is worth 2,008, with a targeting computer
    /// and the second ER medium laser crossing its 26-point heat efficiency.
    #[test]
    fn thunderbolt_worked_example() {
        let settings = WeaponSettings::default();
        let tc = |weapon| MountFacts {
            weapon,
            arc: Arc::Unaffected,
            one_shot: false,
            targeting_computer: weapon.supports_targeting_computer(),
            artemis: 1.0,
        };
        let values = value_weapons(
            &settings,
            &[
                tc(Weapon::Lrm10),
                tc(Weapon::ErPpc),
                tc(Weapon::ErMediumLaser),
                tc(Weapon::ErMediumLaser),
                tc(Weapon::ErMediumLaser),
            ],
        );
        let valuation = Valuation {
            chassis: Chassis::Mech {
                gyro: 32.5,
                heat_efficiency: 26.0,
            },
            armor: 200.0,
            structure: 104.0,
            defensive_equipment: 61.0,
            explosive_penalty: 15.0,
            target_modifier: 3,
            ammunition: ammunition_value([(Weapon::Lrm10, false)].into_iter(), &values.models),
            weapons: values.weapons,
            tonnage: 65.0,
            speed_mp: 8 + 2,
            ..mech(0.0)
        };
        let value = valuation.evaluate();
        assert!((value.defensive - 954.85).abs() < 1e-9);
        assert!((value.offensive - 1052.98).abs() < 1e-9);
        assert_eq!(value.total, 2008);
    }

    /// TechManual p. 308: the Hephaestus OmniVehicle (Prime) is worth 777.
    #[test]
    fn hephaestus_worked_example() {
        let valuation = Valuation {
            chassis: Chassis::Vehicle { unit_type: 0.7 },
            armor: 96.0,
            structure: 15.0,
            defensive_equipment: 73.0,
            target_modifier: 4,
            weapons: vec![weapon(111.0, 4.0), weapon(111.0, 4.0)],
            tonnage: 15.0,
            speed_mp: 12,
            ..mech(0.0)
        };
        let value = valuation.evaluate();
        assert!((value.defensive - 328.79).abs() < 1e-9);
        assert!((value.offensive - 447.93).abs() < 1e-9);
        assert_eq!(value.total, 777);
    }

    /// Rear-firing weapons count half, unless they outweigh the forward-firing ones, when the
    /// forward-firing weapons halve instead; arm weapons are never halved.
    #[test]
    fn rear_arc_halving() {
        let settings = WeaponSettings::default();
        let mount = |weapon, arc| MountFacts {
            weapon,
            arc,
            one_shot: false,
            targeting_computer: false,
            artemis: 1.0,
        };
        let values = value_weapons(
            &settings,
            &[
                mount(Weapon::LargeLaser, Arc::Forward),
                mount(Weapon::MediumLaser, Arc::Rear),
                mount(Weapon::MediumLaser, Arc::Unaffected),
            ],
        );
        let large = f64::from(settings.battle_value(Weapon::LargeLaser));
        assert_eq!(values.weapons[0].value, large);
        assert_eq!(values.weapons[1].value, 23.0);
        assert_eq!(values.weapons[2].value, 46.0);
        let values = value_weapons(
            &settings,
            &[
                mount(Weapon::SmallLaser, Arc::Forward),
                mount(Weapon::MediumLaser, Arc::Rear),
            ],
        );
        assert_eq!(values.weapons[0].value, 4.5);
        assert_eq!(values.weapons[1].value, 46.0);
    }

    /// One-shot launchers carry a fifth of the launcher value and a quarter of its heat;
    /// Ultra, Rotary and Streak heat use their Total Weapon Heat multipliers.
    #[test]
    fn one_shot_and_heat_adjustments() {
        let settings = WeaponSettings::default();
        assert_eq!(unmodified_value(&settings, Weapon::Srm6, true), 12.0);
        assert_eq!(unmodified_value(&settings, Weapon::Lrm15, true), 27.0);
        assert_eq!(weapon_heat(Weapon::Srm6, true), 1.0);
        assert_eq!(weapon_heat(Weapon::UltraAc5, false), 2.0);
        assert_eq!(weapon_heat(Weapon::RotaryAc5, false), 6.0);
        assert_eq!(weapon_heat(Weapon::StreakSrm6, false), 2.0);
    }

    /// Ammunition for each weapon model is capped at that model's unmodified weapon BV, and
    /// ammunition without a weapon is worth nothing.
    #[test]
    fn excessive_ammunition_cap() {
        let models = BTreeMap::from([(Weapon::Ac20, 178.0), (Weapon::MachineGun, 5.0)]);
        let bins = [
            (Weapon::Ac20, false),
            (Weapon::Ac20, false),
            (Weapon::MachineGun, true),
            (Weapon::Lrm20, false),
        ];
        assert_eq!(ammunition_value(bins.into_iter(), &models), 44.0 + 0.5);
        let bins = std::iter::repeat_n((Weapon::MachineGun, false), 8);
        assert_eq!(ammunition_value(bins, &models), 5.0);
    }

    /// AMS ammunition counts toward defense only up to the AMS value.
    #[test]
    fn anti_missile_ammunition_cap() {
        let settings = WeaponSettings::default();
        let ams = f64::from(settings.battle_value(Weapon::AntiMissileSystem));
        let value = defensive_weapon_value(
            &settings,
            [Weapon::AntiMissileSystem, Weapon::MediumLaser].into_iter(),
            std::iter::repeat_n((Weapon::AntiMissileSystem, false), 5),
        );
        assert_eq!(value, ams + ams.min(55.0));
    }
}
