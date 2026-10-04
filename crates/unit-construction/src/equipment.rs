//! Typed equipment facts used by template application and combat rules.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

mod catalogue;
mod families;
mod range;
mod water;
pub use catalogue::Weapon;
pub use range::{RangeBracket, WeaponRange};
pub use water::WaterRanges;

/// Match ASCII equipment namespaces while retaining the original asset text for diagnostics.
pub fn strip_name_prefix<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
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

impl Weapon {
    /// Effective range in hexes, including artillery map-sheet units and optional extreme range.
    pub fn effective_range(self, extended: bool) -> u16 {
        self.effective_range_for_ammunition(extended, super::AmmunitionMode::Normal)
    }

    /// Maximum reach follows the selected ammunition family before extreme-range expansion.
    pub fn effective_range_for_ammunition(
        self,
        extended: bool,
        ammunition: super::AmmunitionMode,
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
    pub fn supports_split_mount(self) -> bool {
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
                    | Self::Nlrm5
                    | Self::Nlrm10
                    | Self::Nlrm15
                    | Self::Nlrm20
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

    /// Energy weapons for armor that reacts to the attack type: lasers, PPCs, flamers and
    /// plasma weapons. Anti-personnel pods and laser anti-missile systems never strike armor.
    pub fn is_energy(self) -> bool {
        match self {
            Self::APod | Self::ClanAPod | Self::LaserAms | Self::ClanLaserAms => false,
            Self::HeavyFlamer
            | Self::VehicleFlamer
            | Self::VehicleHeavyFlamer
            | Self::PlasmaRifle => true,
            _ => self.gunnery_skill(true) == "Gunnery-Laser",
        }
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

    /// Damage a critical hit releases from this weapon while its feed is jammed: a jammed
    /// rotary autocannon explodes for one shot's damage, as in MegaMek.
    pub fn jammed_explosion_damage(self) -> u8 {
        if self.is_rotary() {
            self.profile().damage
        } else {
            0
        }
    }

    /// Gauss and plasma ammunition are inert; ordinary rounds release their projectile damage.
    pub fn ammunition_explosion_damage(self, rounds: u16) -> u32 {
        self.ammunition_explosion_damage_for_mode(rounds, super::AmmunitionMode::Normal)
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

    /// Intrinsic target-number adjustment, separate from range and damaged mounting systems.
    pub fn accuracy_modifier(self) -> i8 {
        match self {
            Self::ClanLargePulseLaser
            | Self::ClanMediumPulseLaser
            | Self::ClanSmallPulseLaser
            | Self::ClanMicroPulseLaser
            | Self::SmallPulseLaser
            | Self::MediumPulseLaser
            | Self::LargePulseLaser
            | Self::XSmallPulseLaser
            | Self::XMediumPulseLaser
            | Self::XLargePulseLaser => -2,
            Self::ClanErLargePulseLaser
            | Self::ClanErMediumPulseLaser
            | Self::ClanErSmallPulseLaser => -1,
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
pub enum System {
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
    /// Filler slots claimed by laser-reflective armor; the armor itself is a chassis technology.
    LaserReflective,
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

impl System {
    /// Passive equipment occupies slots but cannot receive random critical hits.
    pub fn is_noncritical(self) -> bool {
        matches!(
            self,
            Self::FerroFibrous
                | Self::StealthArmor
                | Self::LaserReflective
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
        let Some(system) = Self::named(name) else {
            bail!("Unsupported equipment {name}");
        };
        Ok(system)
    }

    /// Look up a system slot name, or `None` for weapons, ammunition and unknown parts.
    ///
    /// Validation and critical-slot scans ask this of every slot on a unit, most of which
    /// are not systems, so the miss returns no error: building one per slot would format a
    /// message and, when backtraces are enabled, capture a stack trace each time.
    pub fn named(name: &str) -> Option<Self> {
        match name {
            name if name.eq_ignore_ascii_case("ShoulderOrHip") => Some(Self::ShoulderOrHip),
            name if name.eq_ignore_ascii_case("UpperActuator") => Some(Self::UpperActuator),
            name if name.eq_ignore_ascii_case("LowerActuator") => Some(Self::LowerActuator),
            name if name.eq_ignore_ascii_case("HandOrFootActuator") => {
                Some(Self::HandOrFootActuator)
            }
            name if name.eq_ignore_ascii_case("Engine") => Some(Self::Engine),
            name if name.eq_ignore_ascii_case("Gyro") => Some(Self::Gyro),
            name if name.eq_ignore_ascii_case("Cockpit") => Some(Self::Cockpit),
            name if name.eq_ignore_ascii_case("LifeSupport") => Some(Self::LifeSupport),
            name if name.eq_ignore_ascii_case("Sensors") => Some(Self::Sensors),
            name if name.eq_ignore_ascii_case("HeatSink") => Some(Self::HeatSink),
            name if name.eq_ignore_ascii_case("Fuel_Tank") => Some(Self::FuelTank),
            name if name.eq_ignore_ascii_case("JumpJet") => Some(Self::JumpJet),
            name if name.eq_ignore_ascii_case("TargetingComputer") => Some(Self::TargetingComputer),
            name if name.eq_ignore_ascii_case("ArtemisIV") => Some(Self::ArtemisIv),
            name if name.eq_ignore_ascii_case("Ecm") => Some(Self::Ecm),
            name if name.eq_ignore_ascii_case("AngelEcm") => Some(Self::AngelEcm),
            name if name.eq_ignore_ascii_case("StealthArmor") => Some(Self::StealthArmor),
            name if name.eq_ignore_ascii_case("LaserReflective") => Some(Self::LaserReflective),
            name if name.eq_ignore_ascii_case("NullSig_Device") => Some(Self::NullSignature),
            name if name.eq_ignore_ascii_case("TAG") => Some(Self::Tag),
            name if name.eq_ignore_ascii_case("BeagleProbe") => Some(Self::BeagleProbe),
            name if name.eq_ignore_ascii_case("Light_BAP") => Some(Self::LightProbe),
            name if name.eq_ignore_ascii_case("BloodhoundProbe") => Some(Self::BloodhoundProbe),
            name if name.eq_ignore_ascii_case("Axe") => Some(Self::Axe),
            name if name.eq_ignore_ascii_case("Sword") => Some(Self::Sword),
            name if name.eq_ignore_ascii_case("Mace") => Some(Self::Mace),
            name if name.eq_ignore_ascii_case("Dual_Saw") => Some(Self::DualSaw),
            name if name.eq_ignore_ascii_case("Claw") => Some(Self::Claw),
            name if name.eq_ignore_ascii_case("Retractable_Blade") => Some(Self::RetractableBlade),
            name if name.eq_ignore_ascii_case("Lance") => Some(Self::Lance),
            name if name.eq_ignore_ascii_case("Flail") => Some(Self::Flail),
            name if name.eq_ignore_ascii_case("Wrecking_Ball") => Some(Self::WreckingBall),
            name if name.eq_ignore_ascii_case("Chain_Whip") => Some(Self::ChainWhip),
            name if name.eq_ignore_ascii_case("Small_Vibroblade") => Some(Self::SmallVibroblade),
            name if name.eq_ignore_ascii_case("Medium_Vibroblade") => Some(Self::MediumVibroblade),
            name if name.eq_ignore_ascii_case("Large_Vibroblade") => Some(Self::LargeVibroblade),
            name if name.eq_ignore_ascii_case("FerroFibrous") => Some(Self::FerroFibrous),
            name if name.eq_ignore_ascii_case("EndoSteel") => Some(Self::EndoSteel),
            name if name.eq_ignore_ascii_case("TripleStrengthMyomer") => {
                Some(Self::TripleStrengthMyomer)
            }
            name if name.eq_ignore_ascii_case("Masc") => Some(Self::Masc),
            name if name.eq_ignore_ascii_case("SuperCharger") => Some(Self::Supercharger),
            name if name.eq_ignore_ascii_case("C3Master") => Some(Self::C3Master),
            name if name.eq_ignore_ascii_case("C3Slave") => Some(Self::C3Slave),
            name if name.eq_ignore_ascii_case("C3i") => Some(Self::C3i),
            name if name.eq_ignore_ascii_case("HvyFerroFibrous") => Some(Self::HeavyFerroFibrous),
            name if name.eq_ignore_ascii_case("LtFerroFibrous") => Some(Self::LightFerroFibrous),
            name if name.eq_ignore_ascii_case("CASE") => Some(Self::Case),
            name if name.eq_ignore_ascii_case("CASE-II") => Some(Self::CaseIi),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Weapon as W;

    /// Enhanced LRMs are heavier LRMs with a three-hex minimum range and the same salvos.
    #[test]
    fn enhanced_lrms_match_lrm_salvos_with_shorter_minimum_range() {
        for (enhanced, standard, tons) in [
            (W::Nlrm5, W::Lrm5, 3),
            (W::Nlrm10, W::Lrm10, 6),
            (W::Nlrm15, W::Lrm15, 9),
            (W::Nlrm20, W::Lrm20, 12),
        ] {
            assert_eq!(W::parse(enhanced.name()).unwrap(), enhanced);
            assert_eq!(W::from_part_id(enhanced.part_id()), Some(enhanced));
            assert_eq!(enhanced.mass(), tons * 1024);
            let (profile, lrm) = (enhanced.profile(), standard.profile());
            assert_eq!(profile.minimum_range, 3);
            assert_eq!(
                (profile.heat, profile.missiles, profile.long_range),
                (lrm.heat, lrm.missiles, lrm.long_range)
            );
            assert!(enhanced.supports_indirect_fire() && enhanced.supports_semiguided());
        }
        assert!(W::parse("IS.NLRM-15").is_ok());
    }

    /// Energy weapons are the ones reflective armor deflects.
    #[test]
    fn energy_weapons_include_flamers_and_plasma_but_not_defenses() {
        for weapon in [
            W::MediumLaser,
            W::ErPpc,
            W::HeavyFlamer,
            W::PlasmaRifle,
            W::ClanFlamer,
        ] {
            assert!(weapon.is_energy(), "{weapon:?}");
        }
        for weapon in [W::Ac20, W::Lrm20, W::LaserAms, W::APod, W::GaussRifle] {
            assert!(!weapon.is_energy(), "{weapon:?}");
        }
    }
}
