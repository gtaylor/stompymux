//! Typed equipment facts used by template application and combat rules.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

mod catalogue;
mod water;
pub use catalogue::BattleWeapon;
pub use water::BattleWaterRanges;

/// Match ASCII equipment namespaces while retaining the original asset text for diagnostics.
pub(super) fn strip_name_prefix<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
    name.get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))?;
    name.get(prefix.len()..)
}

/// Intrinsic weapon facts; missile damage is per missile, ammunition is per salvo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WeaponProfile {
    pub heat: u8,
    pub damage: u8,
    pub missiles: u8,
    pub minimum_range: u8,
    pub short_range: u8,
    pub medium_range: u8,
    pub long_range: u8,
    pub critical_slots: u8,
    pub ammunition_per_ton: u8,
    pub recycle_seconds: u8,
}

impl BattleWeapon {
    /// Resolve an exact canonical or manufacturer-qualified operator name, ignoring ASCII case.
    /// A manufacturer selects the weapon identity; settings apply to all its installations.
    pub fn parse_operator_name(name: &str) -> Result<Self> {
        if let Ok(weapon) = Self::parse(name) {
            return Ok(weapon);
        }
        let Some((brand, canonical)) = name.split_once('.') else {
            bail!("Unsupported weapon {name}");
        };
        let weapon = Self::parse(canonical)?;
        if (1..=5).any(|quality| {
            super::equipment_display::weapon_brand(weapon, quality)
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(brand))
        }) {
            return Ok(weapon);
        }
        bail!("Unsupported manufacturer for weapon {name}")
    }

    /// Effective range in hexes, including artillery map-sheet units and optional extreme range.
    pub fn effective_range(self, extended: bool) -> u16 {
        self.effective_range_for_ammunition(extended, super::BattleAmmunitionMode::Normal)
    }

    /// Maximum reach follows the selected ammunition family before extreme-range expansion.
    pub fn effective_range_for_ammunition(
        self,
        extended: bool,
        ammunition: super::BattleAmmunitionMode,
    ) -> u16 {
        let profile = self.profile_for_ammunition(ammunition);
        let normal = u16::from(profile.long_range) * if self.is_artillery() { 20 } else { 1 };
        if extended {
            normal.max(u16::from(profile.medium_range) * 2)
        } else {
            normal
        }
    }

    /// Weapons whose installations may span adjacent Mech sections through explicit links.
    pub(super) fn supports_split_mount(self) -> bool {
        matches!(
            self,
            Self::Ac20
                | Self::HeavyGaussRifle
                | Self::Lbx20
                | Self::UltraAc20
                | Self::ClanUltraAc20
                | Self::ArrowIv
        )
    }

    /// Flamers can transfer their damage as heat, independently of their ammunition supply.
    pub fn is_flamer(self) -> bool {
        matches!(
            self,
            Self::ClanFlamer
                | Self::Flamer
                | Self::HeavyFlamer
                | Self::VehicleFlamer
                | Self::VehicleHeavyFlamer
        )
    }

    /// Heat-mode controls select flamer heat transfer or coolant self-application.
    pub fn supports_heat_mode(self) -> bool {
        self.is_flamer() || self == Self::CoolantGun
    }

    /// Machine guns can trade three rounds per rolled damage point for gatling fire.
    pub fn supports_gatling(self) -> bool {
        matches!(
            self,
            Self::MachineGun
                | Self::HeavyMachineGun
                | Self::ClanMachineGun
                | Self::ClanLightMachineGun
                | Self::ClanHeavyMachineGun
        )
    }

    /// Rotary autocannons use gunnery-based feed recovery and selectable burst lengths.
    pub fn is_rotary(self) -> bool {
        matches!(
            self,
            Self::RotaryAc2
                | Self::RotaryAc5
                | Self::ClanRotaryAc2
                | Self::ClanRotaryAc5
                | Self::ClanRotaryAc10
                | Self::ClanRotaryAc20
        )
    }

    /// Conventional and light IS autocannons permit rapid two-round firing.
    pub fn supports_rapid_fire(self) -> bool {
        matches!(
            self,
            Self::Ac2 | Self::Ac5 | Self::Ac10 | Self::Ac20 | Self::LightAc2 | Self::LightAc5
        )
    }

    /// Ultra autocannons support two-round firing cycles.
    pub fn is_ultra(self) -> bool {
        matches!(
            self,
            Self::UltraAc2
                | Self::UltraAc5
                | Self::UltraAc10
                | Self::UltraAc20
                | Self::ClanUltraAc2
                | Self::ClanUltraAc5
                | Self::ClanUltraAc10
                | Self::ClanUltraAc20
        )
    }

    /// Supported indirect-fire launchers with selectable hotloading; disposable rockets cannot switch.
    pub fn supports_hotload(self) -> bool {
        self.is_artillery() || (self.supports_indirect_fire() && !self.is_rocket())
    }

    /// Supported missile profiles carrying indirect-fire capability, including disposable rockets.
    pub fn supports_indirect_fire(self) -> bool {
        self.is_rocket()
            || self.is_mml()
            || self.is_thunderbolt()
            || matches!(
                self,
                Self::ClanAtm3
                    | Self::ClanAtm6
                    | Self::ClanAtm9
                    | Self::ClanAtm12
                    | Self::ClanLrm5
                    | Self::ClanLrm10
                    | Self::ClanLrm15
                    | Self::ClanLrm20
                    | Self::Lrm5
                    | Self::Lrm10
                    | Self::Lrm15
                    | Self::Lrm20
                    | Self::Elrm5
                    | Self::Elrm10
                    | Self::Elrm15
                    | Self::Elrm20
                    | Self::LrDfm5
                    | Self::LrDfm10
                    | Self::LrDfm15
                    | Self::LrDfm20
            )
    }

    /// Beam/ballistic computer eligibility excludes ordinary flamers, machine guns, missiles and artillery.
    pub fn supports_targeting_computer(self) -> bool {
        self.profile().missiles == 0
            && !self.is_artillery()
            && !self.supports_gatling()
            && !matches!(
                self,
                Self::Flamer
                    | Self::ClanFlamer
                    | Self::AntiMissileSystem
                    | Self::ClanAntiMissileSystem
            )
    }

    /// Thunderbolt launchers fire one large missile per salvo.
    pub fn is_thunderbolt(self) -> bool {
        matches!(
            self,
            Self::Thunderbolt5 | Self::Thunderbolt10 | Self::Thunderbolt15 | Self::Thunderbolt20
        )
    }

    /// Disposable rocket launchers use the template OneShot supply.
    pub fn is_rocket(self) -> bool {
        matches!(self, Self::Rocket10 | Self::Rocket15 | Self::Rocket20)
    }

    /// LB-X autocannons accept slug or cluster ammunition.
    pub fn is_lbx(self) -> bool {
        matches!(
            self,
            Self::Lbx2
                | Self::Lbx5
                | Self::Lbx10
                | Self::Lbx20
                | Self::ClanLbx2
                | Self::ClanLbx5
                | Self::ClanLbx10
                | Self::ClanLbx20
        )
    }

    /// Internal damage released by the first critical hit on a functional Gauss weapon.
    pub fn weapon_explosion_damage(self) -> u8 {
        match self {
            Self::HeavyGaussRifle => 25,
            Self::GaussRifle | Self::ClanGaussRifle => 20,
            Self::LightGaussRifle => 16,
            Self::MagshotGaussRifle => 3,
            _ => 0,
        }
    }

    /// Gauss and plasma ammunition are inert; ordinary rounds release their projectile damage.
    pub fn ammunition_explosion_damage(self, rounds: u16) -> u32 {
        self.ammunition_explosion_damage_for_mode(rounds, super::BattleAmmunitionMode::Normal)
    }

    /// Whether a launcher requires a successful Streak lock before spending heat and ammunition.
    pub fn is_streak(self) -> bool {
        matches!(
            self,
            Self::StreakSrm2
                | Self::StreakSrm4
                | Self::StreakSrm6
                | Self::ClanStreakSrm2
                | Self::ClanStreakSrm4
                | Self::ClanStreakSrm6
                | Self::ClanStreakLrm5
                | Self::ClanStreakLrm10
                | Self::ClanStreakLrm15
                | Self::ClanStreakLrm20
        )
    }

    /// Dead-fire launchers always use the lowest two of three attack dice.
    pub fn is_dead_fire(self) -> bool {
        matches!(
            self,
            Self::LrDfm5
                | Self::LrDfm10
                | Self::LrDfm15
                | Self::LrDfm20
                | Self::SrDfm2
                | Self::SrDfm4
                | Self::SrDfm6
        )
    }

    /// Dead-fire missiles and extended LRMs below minimum range use the lowest two of three dice.
    pub(super) fn attack_roll(self, distance: f64, dice: &mut super::BattleDice) -> u8 {
        if self.is_dead_fire()
            || (matches!(
                self,
                Self::Elrm5 | Self::Elrm10 | Self::Elrm15 | Self::Elrm20
            ) && distance < f64::from(self.profile().minimum_range))
        {
            let first = dice.d6();
            let second = dice.d6();
            let third = dice.d6();
            return first + second + third - first.max(second).max(third);
        }
        dice.generic_roll()
    }

    /// Intrinsic target-number adjustment, separate from range and damaged mounting systems.
    pub fn accuracy_modifier(self) -> i8 {
        match self {
            Self::ClanLargePulseLaser
            | Self::ClanMediumPulseLaser
            | Self::ClanSmallPulseLaser
            | Self::ClanMicroPulseLaser
            | Self::ClanErLargePulseLaser
            | Self::ClanErMediumPulseLaser
            | Self::ClanErSmallPulseLaser
            | Self::SmallPulseLaser
            | Self::MediumPulseLaser
            | Self::LargePulseLaser
            | Self::XSmallPulseLaser
            | Self::XMediumPulseLaser
            | Self::XLargePulseLaser => -2,
            Self::ClanHeavyLargeLaser
            | Self::ClanHeavyMediumLaser
            | Self::ClanHeavySmallLaser
            | Self::Mrm10
            | Self::Mrm20
            | Self::Mrm30
            | Self::Mrm40
            | Self::Rocket10
            | Self::Rocket15
            | Self::Rocket20 => 1,
            _ => 0,
        }
    }
}

/// Conventional biped systems represented by individual critical slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleSystem {
    ShoulderOrHip,
    UpperActuator,
    LowerActuator,
    HandOrFootActuator,
    Engine,
    Gyro,
    Cockpit,
    LifeSupport,
    Sensors,
    HeatSink,
    FuelTank,
    JumpJet,
    FerroFibrous,
    EndoSteel,
    TripleStrengthMyomer,
    Masc,
    Supercharger,
    C3Master,
    C3Slave,
    C3i,
    HeavyFerroFibrous,
    LightFerroFibrous,
    Case,
    /// CASE II vents a local explosion through armor after one point of internal damage.
    CaseIi,
    TargetingComputer,
    ArtemisIv,
    Ecm,
    AngelEcm,
    StealthArmor,
    NullSignature,
    BeagleProbe,
    Tag,
    LightProbe,
    BloodhoundProbe,
    Axe,
    Sword,
    Mace,
    DualSaw,
    Claw,
    RetractableBlade,
    Lance,
    Flail,
    WreckingBall,
    ChainWhip,
    SmallVibroblade,
    MediumVibroblade,
    LargeVibroblade,
}

impl BattleSystem {
    /// Passive equipment occupies slots but cannot receive random critical hits.
    pub(crate) fn is_noncritical(self) -> bool {
        matches!(
            self,
            Self::FerroFibrous
                | Self::StealthArmor
                | Self::EndoSteel
                | Self::TripleStrengthMyomer
                | Self::HeavyFerroFibrous
                | Self::LightFerroFibrous
                | Self::Case
                | Self::CaseIi
        )
    }

    /// Resolve only systems whose slot identity is represented by the Rust domain.
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            name if name.eq_ignore_ascii_case("ShoulderOrHip") => Ok(Self::ShoulderOrHip),
            name if name.eq_ignore_ascii_case("UpperActuator") => Ok(Self::UpperActuator),
            name if name.eq_ignore_ascii_case("LowerActuator") => Ok(Self::LowerActuator),
            name if name.eq_ignore_ascii_case("HandOrFootActuator") => Ok(Self::HandOrFootActuator),
            name if name.eq_ignore_ascii_case("Engine") => Ok(Self::Engine),
            name if name.eq_ignore_ascii_case("Gyro") => Ok(Self::Gyro),
            name if name.eq_ignore_ascii_case("Cockpit") => Ok(Self::Cockpit),
            name if name.eq_ignore_ascii_case("LifeSupport") => Ok(Self::LifeSupport),
            name if name.eq_ignore_ascii_case("Sensors") => Ok(Self::Sensors),
            name if name.eq_ignore_ascii_case("HeatSink") => Ok(Self::HeatSink),
            name if name.eq_ignore_ascii_case("Fuel_Tank") => Ok(Self::FuelTank),
            name if name.eq_ignore_ascii_case("JumpJet") => Ok(Self::JumpJet),
            name if name.eq_ignore_ascii_case("TargetingComputer") => Ok(Self::TargetingComputer),
            name if name.eq_ignore_ascii_case("ArtemisIV") => Ok(Self::ArtemisIv),
            name if name.eq_ignore_ascii_case("Ecm") => Ok(Self::Ecm),
            name if name.eq_ignore_ascii_case("AngelEcm") => Ok(Self::AngelEcm),
            name if name.eq_ignore_ascii_case("StealthArmor") => Ok(Self::StealthArmor),
            name if name.eq_ignore_ascii_case("NullSig_Device") => Ok(Self::NullSignature),
            name if name.eq_ignore_ascii_case("TAG") => Ok(Self::Tag),
            name if name.eq_ignore_ascii_case("BeagleProbe") => Ok(Self::BeagleProbe),
            name if name.eq_ignore_ascii_case("Light_BAP") => Ok(Self::LightProbe),
            name if name.eq_ignore_ascii_case("BloodhoundProbe") => Ok(Self::BloodhoundProbe),
            name if name.eq_ignore_ascii_case("Axe") => Ok(Self::Axe),
            name if name.eq_ignore_ascii_case("Sword") => Ok(Self::Sword),
            name if name.eq_ignore_ascii_case("Mace") => Ok(Self::Mace),
            name if name.eq_ignore_ascii_case("Dual_Saw") => Ok(Self::DualSaw),
            name if name.eq_ignore_ascii_case("Claw") => Ok(Self::Claw),
            name if name.eq_ignore_ascii_case("Retractable_Blade") => Ok(Self::RetractableBlade),
            name if name.eq_ignore_ascii_case("Lance") => Ok(Self::Lance),
            name if name.eq_ignore_ascii_case("Flail") => Ok(Self::Flail),
            name if name.eq_ignore_ascii_case("Wrecking_Ball") => Ok(Self::WreckingBall),
            name if name.eq_ignore_ascii_case("Chain_Whip") => Ok(Self::ChainWhip),
            name if name.eq_ignore_ascii_case("Small_Vibroblade") => Ok(Self::SmallVibroblade),
            name if name.eq_ignore_ascii_case("Medium_Vibroblade") => Ok(Self::MediumVibroblade),
            name if name.eq_ignore_ascii_case("Large_Vibroblade") => Ok(Self::LargeVibroblade),
            name if name.eq_ignore_ascii_case("FerroFibrous") => Ok(Self::FerroFibrous),
            name if name.eq_ignore_ascii_case("EndoSteel") => Ok(Self::EndoSteel),
            name if name.eq_ignore_ascii_case("TripleStrengthMyomer") => {
                Ok(Self::TripleStrengthMyomer)
            }
            name if name.eq_ignore_ascii_case("Masc") => Ok(Self::Masc),
            name if name.eq_ignore_ascii_case("SuperCharger") => Ok(Self::Supercharger),
            name if name.eq_ignore_ascii_case("C3Master") => Ok(Self::C3Master),
            name if name.eq_ignore_ascii_case("C3Slave") => Ok(Self::C3Slave),
            name if name.eq_ignore_ascii_case("C3i") => Ok(Self::C3i),
            name if name.eq_ignore_ascii_case("HvyFerroFibrous") => Ok(Self::HeavyFerroFibrous),
            name if name.eq_ignore_ascii_case("LtFerroFibrous") => Ok(Self::LightFerroFibrous),
            name if name.eq_ignore_ascii_case("CASE") => Ok(Self::Case),
            name if name.eq_ignore_ascii_case("CASE-II") => Ok(Self::CaseIi),
            _ => bail!("Unsupported equipment {name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BattleWeapon as W;
    use crate::BattleDice;

    /// Dead-fire attack dice never revert to ordinary 2d6 at longer ranges.
    #[test]
    fn dead_fire_attack_dice_at_all_ranges() {
        for weapon in [
            W::LrDfm5,
            W::LrDfm10,
            W::LrDfm15,
            W::LrDfm20,
            W::SrDfm2,
            W::SrDfm4,
            W::SrDfm6,
        ] {
            for seed in 0..=255 {
                for distance in [0.0, 4.0, 6.0, 12.0, 18.0, 24.001] {
                    let mut expected = BattleDice::seeded([seed; 32]);
                    let mut values = [expected.d6(), expected.d6(), expected.d6()];
                    values.sort_unstable();
                    let mut actual = BattleDice::seeded([seed; 32]);
                    assert_eq!(
                        weapon.attack_roll(distance, &mut actual),
                        values[0] + values[1]
                    );
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    /// Raw distance controls the extra attack die independently of aim-bracket rounding.
    #[test]
    fn elrm_minimum_range_attack_dice_and_replay() {
        for weapon in [W::Elrm5, W::Elrm10, W::Elrm15, W::Elrm20] {
            for seed in 0..=255 {
                for distance in [0.0, 9.999, 10.0, 10.001, 36.0] {
                    let mut expected = BattleDice::seeded([seed; 32]);
                    let count = if distance < 10.0 { 3 } else { 2 };
                    let mut values: Vec<_> = (0..count).map(|_| expected.d6()).collect();
                    values.sort_unstable();
                    let mut actual = BattleDice::seeded([seed; 32]);
                    assert_eq!(
                        weapon.attack_roll(distance, &mut actual),
                        values[0] + values[1]
                    );
                    assert_eq!(actual, expected);
                    let mut restored: BattleDice =
                        serde_json::from_value(serde_json::to_value(&actual).unwrap()).unwrap();
                    assert_eq!(restored.two_d6(), actual.two_d6());
                }
            }
        }
        let mut ordinary = BattleDice::seeded([1; 32]);
        let mut expected = ordinary.clone();
        assert_eq!(W::Lrm20.attack_roll(0.0, &mut ordinary), expected.two_d6());
        assert_eq!(ordinary, expected);
    }
}
