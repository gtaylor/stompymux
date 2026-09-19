//! Current Mech and vehicle Battle Value from installed equipment and live defensive facts.
use super::{
    BattleEngine, BattleGyro, BattleSection, BattleSystem, BattleUnit, BattleWeapon,
    BattleWeaponSettings, StoredBattleMap,
};
use anyhow::Result;
use serde::Serialize;

/// Offensive and defensive scores, including the defensive movement multiplier.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BattleValue {
    pub offensive: f64,
    pub defensive: f64,
    pub total: f64,
}

/// Offensive weapons consume heat capacity in descending value order, retaining integer half-values.
fn offensive_value(
    settings: &BattleWeaponSettings,
    mut weapons: Vec<BattleWeapon>,
    heat_efficiency: i32,
    tons: u16,
) -> f32 {
    weapons.sort_by_key(|weapon| settings.battle_value(*weapon));
    let mut heat = 0;
    let mut value = f32::from(tons);
    for weapon in weapons.into_iter().rev().filter(|weapon| !weapon.is_ams()) {
        heat += i32::from(weapon.profile().heat);
        let bv = settings.battle_value(weapon);
        value += (if heat > heat_efficiency { bv / 2 } else { bv }) as f32;
    }
    value
}

/// Defensive movement bands use truncated whole running MP.
fn movement_modifier(running: i32, jumping: i32) -> i32 {
    let ground = match running {
        ..=2 => 0,
        3..=4 => 1,
        5..=6 => 2,
        7..=9 => 3,
        10..=17 => 4,
        18..=24 => 5,
        _ => 6,
    };
    ground + i32::from(jumping > running)
}

/// Shared defensive equipment contributions retain installed weapons and ammunition bins.
fn defensive_equipment(
    settings: &BattleWeaponSettings,
    weapons: impl Iterator<Item = BattleWeapon>,
    ammunition: impl Iterator<Item = BattleWeapon>,
    ecm: bool,
    probe: bool,
) -> f32 {
    let mut value = if ecm { 61.0 } else { 0.0 } + if probe { 10.0 } else { 0.0 };
    for weapon in weapons {
        if weapon.is_ams() || matches!(weapon, BattleWeapon::APod | BattleWeapon::ClanAPod) {
            value += settings.battle_value(weapon) as f32;
        }
    }
    for weapon in ammunition {
        value += match weapon {
            BattleWeapon::AntiMissileSystem => 11.0,
            BattleWeapon::ClanAntiMissileSystem => 21.0,
            _ => 0.0,
        };
    }
    value
}

/// Apply movement and shared decimal rounding after chassis-specific defensive adjustments.
fn finish_value(offense: f32, mut defense: f32, movement: i32) -> BattleValue {
    defense += defense * (movement as f32 * 0.1);
    defense = ((defense * 100.0).round() / 100.0).max(0.0);
    BattleValue {
        offensive: f64::from(offense),
        defensive: f64::from(defense),
        total: f64::from(offense) + f64::from(defense),
    }
}

impl BattleUnit {
    /// Compute the supported biped score with catalogue weapon values, without changing state.
    /// Installed weapons and bins retain their BV contribution after damage or expenditure.
    /// Armor, structure, heat sinks, jump jets and effective running speed use current state.
    pub fn battle_value(&self, map: Option<&StoredBattleMap>) -> Result<BattleValue> {
        self.battle_value_at_speed(
            self.effective_maximum_speed(map)?,
            &BattleWeaponSettings::default(),
        )
    }

    /// Reuse valuation with the caller's world-aware effective maximum.
    fn battle_value_at_speed(
        &self,
        maximum: f64,
        settings: &BattleWeaponSettings,
    ) -> Result<BattleValue> {
        let definition = self.definition();
        let loadout = self.loadout()?;
        let clan = definition.has_special("Clan");
        // C derives the engine mass family from technology flags alone
        // (battle_value.c); reference builds without engine criticals still
        // produce a value, so fall back to the flag spelling there.
        let engine = self.engine().unwrap_or_else(|_| {
            BattleEngine::display_from_flags(
                definition.has_special("LightEngine_Tech"),
                definition.has_special("CompactEngine_Tech"),
                definition.has_special("XXL_Tech"),
                definition.has_special("XLEngine_Tech"),
            )
        });
        let engine_factor = match engine {
            BattleEngine::Light => 0.75,
            BattleEngine::Xl if clan => 0.75,
            BattleEngine::Xl | BattleEngine::Xxl => 0.5,
            _ => 1.0,
        };
        let armor: u32 = self
            .sections()
            .values()
            .map(|section| u32::from(section.armor) + u32::from(section.rear))
            .sum();
        let structure: u32 = self
            .sections()
            .values()
            .map(|section| u32::from(section.internal))
            .sum();
        let mut defense = armor as f32 * 2.5 + structure as f32 * 1.5 * engine_factor;
        defense += f32::from(definition.tons)
            * if self.gyro() == BattleGyro::Hardened {
                1.0
            } else {
                0.5
            };
        let ecm_slots = loadout
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::Ecm)
            .count();
        let probe_slots = loadout
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::BeagleProbe)
            .count();
        defense += defensive_equipment(
            settings,
            loadout.weapons.iter().map(|mount| mount.weapon),
            loadout.ammunition.iter().map(|bin| bin.weapon),
            ecm_slots >= if clan { 1 } else { 2 },
            probe_slots >= if clan { 1 } else { 2 },
        );
        let vulnerable_core = |section| {
            matches!(
                section,
                BattleSection::CenterTorso
                    | BattleSection::Head
                    | BattleSection::LeftLeg
                    | BattleSection::RightLeg
            )
        };
        let xl = matches!(engine, BattleEngine::Xl | BattleEngine::Xxl);
        for bin in &loadout.ammunition {
            let section = bin.location.section;
            if vulnerable_core(section) || xl || !self.has_case(section) {
                defense -= 15.0;
            }
        }
        // Gauss exposure is assessed per installed weapon slot, including destroyed slots.
        for (&section, layout) in &definition.sections {
            let exposed = if (clan && vulnerable_core(section)) || xl {
                true
            } else if vulnerable_core(section) {
                !self.has_case(section)
            } else {
                match section {
                    BattleSection::LeftArm => !self.has_case(BattleSection::LeftTorso),
                    BattleSection::RightArm => !self.has_case(BattleSection::RightTorso),
                    _ => false,
                }
            };
            if !exposed {
                continue;
            }
            defense -= layout
                .criticals
                .values()
                .filter(|part| {
                    BattleWeapon::parse(&part.equipment)
                        .is_ok_and(|weapon| weapon.weapon_explosion_damage() > 0)
                })
                .count() as f32;
        }
        // BV jump MP is independent of map gravity and does not use flight admission rules.
        let jump = ((definition.jump_speed as f32
            - f32::from(self.system_hits(BattleSystem::JumpJet)) * 10.75)
            .max(0.0)
            / 10.75) as i32;
        let maximum = maximum as f32;
        let running = if definition.has_triple_myomer() {
            (((maximum / 1.5 / 10.75).round_ties_even() + 1.0) * 1.5) as i32
        } else {
            (maximum / 10.75) as i32
        };
        let sinks = i32::from(self.cooling_capacity());
        let offense = offensive_value(
            settings,
            loadout.weapons.iter().map(|mount| mount.weapon).collect(),
            6 + sinks - jump.max(2),
            definition.tons,
        );
        Ok(finish_value(
            offense,
            defense,
            movement_modifier(running, jump),
        ))
    }
}

impl super::BattleVehicle {
    /// Ground-vehicle and VTOL BV with catalogue values; installed equipment remains counted after damage.
    /// Hull protection and available motive speed use current state. Vehicles have no gyro bonus
    /// or ammunition-bin vulnerability penalty in this valuation.
    pub fn battle_value(&self) -> Result<BattleValue> {
        self.battle_value_at_speed(self.maximum_speed(), &BattleWeaponSettings::default())
    }

    /// Vehicle movement bands consume the same loaded ceiling as world movement.
    fn battle_value_at_speed(
        &self,
        maximum: f64,
        settings: &BattleWeaponSettings,
    ) -> Result<BattleValue> {
        use super::{BattleVehicleMovement as Movement, BattleVehicleSection as Section};
        let definition = self.definition();
        let loadout = super::BattleVehicleLoadout::resolve(definition)?;
        let armor: u32 = self
            .sections()
            .values()
            .map(|section| u32::from(section.armor) + u32::from(section.rear))
            .sum();
        let structure: u32 = self
            .sections()
            .values()
            .map(|section| u32::from(section.internal))
            .sum();
        let mut defense = armor as f32 * 2.5 + structure as f32 * 1.5;
        defense += defensive_equipment(
            settings,
            loadout.weapons.iter().map(|mount| mount.weapon),
            loadout.ammunition.iter().map(|bin| bin.weapon),
            loadout
                .systems
                .iter()
                .any(|part| part.system == BattleSystem::Ecm),
            loadout
                .systems
                .iter()
                .any(|part| part.system == BattleSystem::BeagleProbe),
        );
        let xl = definition.has_special("XLEngine_Tech") || definition.has_special("XXL_Tech");
        for mount in &loadout.weapons {
            if mount.weapon.weapon_explosion_damage() == 0 {
                continue;
            }
            let exposed = match mount.criticals[0].section {
                Section::Turret | Section::Left => true,
                Section::Right => xl || !self.has_powerplant_containment(),
                _ => xl,
            };
            defense -= f32::from(exposed);
        }
        let discount = match definition.movement {
            Movement::Tracked => 0.1,
            Movement::Wheeled => 0.2,
            Movement::Hover | Movement::Vtol => 0.3,
            Movement::Stationary => 0.0,
        };
        defense -= defense * discount;
        let mut running = (maximum as f32 / 10.75) as i32;
        let masc = loadout
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::Masc)
            .count()
            >= usize::from(
                (definition.tons
                    / if definition.has_special("Clan") {
                        25
                    } else {
                        20
                    })
                .max(1),
            );
        let supercharger = definition.has_special("SuperCharger_Tech");
        if masc && supercharger {
            running = ((running * 2 / 3) as f32 * 2.5) as i32;
        }
        if masc || supercharger {
            running = (running * 2 / 3) * 2;
        }
        // Rotorcraft receive their class bonus even when motive or rotor damage stops them.
        let movement = movement_modifier(running, 0)
            + i32::from(definition.movement == Movement::Vtol)
            + if definition.has_special("StealthArmor_Tech") {
                2
            } else {
                0
            };
        let sinks = definition.heat_sink_capacity();
        let offense = offensive_value(
            settings,
            loadout.weapons.iter().map(|mount| mount.weapon).collect(),
            4 + i32::from(sinks),
            definition.tons,
        );
        Ok(finish_value(offense, defense, movement))
    }
}

/// Derive battle value from current world load without changing cached or persisted state.
pub fn unit_battle_value(
    world: &crate::World,
    id: crate::ObjectId,
    tsm_tow_bonus: bool,
) -> Result<BattleValue> {
    configured(
        world,
        id,
        super::SpeedPolicy {
            tsm_tow_bonus,
            ..super::SpeedPolicy::STANDARD
        },
    )
}

/// Compute live battle value with the host's towing and sprint policies.
pub(crate) fn configured(
    world: &crate::World,
    id: crate::ObjectId,
    policy: super::SpeedPolicy,
) -> Result<BattleValue> {
    let maximum = super::effective_speed::configured(world, id, policy)?;
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return unit.battle_value_at_speed(maximum, world.btech.weapon_settings());
    }
    world
        .btech
        .constructed_units()
        .get(&id)
        .ok_or_else(|| anyhow::anyhow!("Unit construction is unavailable"))?
        .battle_value_at_speed(maximum, world.btech.weapon_settings())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact heat capacity receives full credit; additional weapons retain integer half-values.
    #[test]
    fn weapon_heat_order_and_truncation() {
        use BattleWeapon::{MediumLaser, Srm4};
        assert_eq!(
            offensive_value(
                &BattleWeaponSettings::default(),
                vec![Srm4, MediumLaser],
                3,
                35
            ),
            35.0 + 46.0 + 19.0
        );
        assert_eq!(
            offensive_value(
                &BattleWeaponSettings::default(),
                vec![MediumLaser, Srm4],
                6,
                35
            ),
            35.0 + 46.0 + 39.0
        );
        assert_eq!(
            offensive_value(&BattleWeaponSettings::default(), vec![Srm4], -1, 35),
            54.0
        );
    }

    /// Every band transition and the independent jump bonus use strict whole-MP comparisons.
    #[test]
    fn defensive_movement_boundaries() {
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
            assert_eq!(movement_modifier(mp, mp), modifier);
            assert_eq!(movement_modifier(mp, mp + 1), modifier + 1);
        }
    }
}
