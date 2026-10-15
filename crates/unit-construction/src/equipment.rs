//! Typed equipment facts used by template application and combat rules.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

mod catalogue;
mod families;
mod range;
mod system_values;
mod water;
pub use catalogue::Weapon;
pub use range::{RangeBracket, WeaponRange};
pub use system_values::{PhysicalBattleValue, SystemContext};
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

/// Hexes across one mapsheet: indirect artillery lists its reach in mapsheets
/// (Tactical Operations p. 149), and a standard sheet is seventeen hexes wide.
pub const ARTILLERY_MAPSHEET_HEXES: u16 = 17;

impl Weapon {
    /// Effective range in hexes, including artillery map-sheet units.
    pub fn effective_range(self) -> u16 {
        self.effective_range_for_ammunition(super::AmmunitionMode::Normal)
    }

    /// Maximum reach in hexes for the selected ammunition family. Indirect artillery stores
    /// mapsheets in its long range; artillery cannons and every other weapon store hexes.
    pub fn effective_range_for_ammunition(self, ammunition: super::AmmunitionMode) -> u16 {
        let profile = self.profile_for_ammunition(ammunition);
        let sheet = if self.is_indirect_artillery() {
            ARTILLERY_MAPSHEET_HEXES
        } else {
            1
        };
        u16::from(profile.long_range) * sheet
    }

    /// Weapons whose installations may span adjacent Mech sections: any ranged weapon of
    /// eight or more critical slots (TM p. 47).
    pub fn supports_split_mount(self) -> bool {
        self.profile().critical_slots >= 8
    }

    /// Whether the weapon is Clan technology, by its catalogue namespace.
    pub fn is_clan(self) -> bool {
        strip_name_prefix(self.name(), "CL.").is_some()
    }

    /// Direct-fire and pulse weapons whose tonnage sizes a targeting computer: every
    /// non-missile heavy weapon except machine guns, flamers, artillery, anti-missile
    /// systems and anti-personnel pods (TM p. 238).
    pub fn is_direct_fire(self) -> bool {
        self.profile().missiles == 0
            && !self.is_artillery()
            && !self.supports_gatling()
            && !self.is_flamer()
            && !matches!(
                self,
                Self::AntiMissileSystem
                    | Self::ClanAntiMissileSystem
                    | Self::LaserAms
                    | Self::ClanLaserAms
                    | Self::APod
                    | Self::ClanAPod
                    | Self::BPod
                    | Self::ClanBPod
                    | Self::NarcBeacon
                    | Self::ClanNarcBeacon
                    | Self::INarcBeacon
            )
    }

    /// Launchers an Artemis IV fire-control system applies to: standard LRM, SRM and MML
    /// launchers and their torpedo versions (TM p. 206), and enhanced LRMs (Tactical
    /// Operations: Advanced Units & Equipment).
    pub fn is_artemis_launcher(self) -> bool {
        self.is_mml()
            || matches!(
                self,
                Self::Nlrm5 | Self::Nlrm10 | Self::Nlrm15 | Self::Nlrm20
            )
            || matches!(
                self,
                Self::Lrm5
                    | Self::Lrm10
                    | Self::Lrm15
                    | Self::Lrm20
                    | Self::ClanLrm5
                    | Self::ClanLrm10
                    | Self::ClanLrm15
                    | Self::ClanLrm20
                    | Self::Srm2
                    | Self::Srm4
                    | Self::Srm6
                    | Self::ClanSrm2
                    | Self::ClanSrm4
                    | Self::ClanSrm6
                    | Self::Lrt5
                    | Self::Lrt10
                    | Self::Lrt15
                    | Self::Lrt20
                    | Self::Srt2
                    | Self::Srt4
                    | Self::Srt6
                    | Self::ClanLrt5
                    | Self::ClanLrt10
                    | Self::ClanLrt15
                    | Self::ClanLrt20
                    | Self::ClanSrt2
                    | Self::ClanSrt4
                    | Self::ClanSrt6
            )
    }

    /// Medium-range missile launchers, the launchers an MRM "Apollo" fire-control system serves
    /// (Tactical Operations: Advanced Units & Equipment p. 142).
    pub fn is_mrm(self) -> bool {
        matches!(self, Self::Mrm10 | Self::Mrm20 | Self::Mrm30 | Self::Mrm40)
    }

    /// Flamers can transfer their damage as heat, independently of their ammunition supply.
    pub fn is_flamer(self) -> bool {
        matches!(
            self,
            Self::ClanFlamer | Self::Flamer | Self::HeavyFlamer | Self::VehicleFlamer
        )
    }

    /// Heat-mode controls select flamer heat transfer.
    pub fn supports_heat_mode(self) -> bool {
        self.is_flamer()
    }

    /// Machine guns can trade three rounds per rolled damage point for gatling fire.
    pub fn supports_gatling(self) -> bool {
        matches!(
            self,
            Self::MachineGun
                | Self::LightMachineGun
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
            Self::RotaryAc2 | Self::RotaryAc5 | Self::ClanRotaryAc2 | Self::ClanRotaryAc5
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
    /// ATMs may not fire indirectly (Total Warfare p. 141), and Streak LRMs may not either.
    pub fn supports_indirect_fire(self) -> bool {
        self.is_rocket()
            || self.is_mml()
            || self.is_thunderbolt()
            || matches!(
                self,
                Self::ClanLrm5
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
            )
    }

    /// Weapons a targeting computer assists: the direct-fire and pulse weapons that size it
    /// (TM p. 238), so flamers, machine guns, missiles, artillery, anti-missile systems and
    /// A-Pods gain nothing.
    pub fn supports_targeting_computer(self) -> bool {
        self.is_direct_fire()
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
            Self::APod
            | Self::ClanAPod
            | Self::BPod
            | Self::ClanBPod
            | Self::LaserAms
            | Self::ClanLaserAms => false,
            Self::HeavyFlamer | Self::VehicleFlamer | Self::PlasmaRifle => true,
            _ => self.gunnery_skill(true) == "Gunnery-Laser",
        }
    }

    /// Internal damage released by the first critical hit on a functional Gauss weapon. A
    /// hyper-assault Gauss rifle explodes for 10, 15 or 20 points and an anti-personnel Gauss
    /// rifle for 3 (Total Warfare p. 135).
    pub fn weapon_explosion_damage(self) -> u8 {
        match self {
            Self::HeavyGaussRifle => 25,
            Self::GaussRifle | Self::ClanGaussRifle | Self::ClanHag40 => 20,
            Self::LightGaussRifle => 16,
            Self::ClanHag30 => 15,
            Self::ClanHag20 => 10,
            Self::MagshotGaussRifle | Self::ClanApGaussRifle => 3,
            _ => 0,
        }
    }

    /// Whether a BattleMech may mount this weapon. The Long Tom's Mech space is "NA"
    /// (Tactical Operations: Advanced Units & Equipment p. 217): only vehicles and larger
    /// units carry it.
    pub fn mech_mountable(self) -> bool {
        self != Self::LongTom
    }

    /// Whether this launcher has a one-shot (OS) version: the BV tables list a second, OS
    /// figure for exactly those launchers (TechManual p. 317; Tactical Operations: Advanced
    /// Units & Equipment pp. 194-197). Rocket launchers are built one-shot and have no
    /// separate OS version.
    pub fn one_shot_capable(self) -> bool {
        self.one_shot_battle_value().is_some()
    }

    /// Anti-battle armor pods (TechManual p. 204; Total Warfare p. 130).
    pub fn is_b_pod(self) -> bool {
        matches!(self, Self::BPod | Self::ClanBPod)
    }

    /// Damage an unexpended B-Pod releases when a critical hit destroys it: 2 points (Total
    /// Warfare p. 130). Zero for every other weapon; Gauss weapons explode by
    /// [`Weapon::weapon_explosion_damage`].
    pub fn pod_explosion_damage(self) -> u8 {
        if self.is_b_pod() { 2 } else { 0 }
    }

    /// Critical slots this weapon counts as a Gauss weapon when the Defensive Battle Rating
    /// subtracts one point per exposed Gauss slot (TechManual p. 302): every Gauss rifle,
    /// hyper-assault and anti-personnel Gauss rifle by its full slot count, and a
    /// hyper-velocity autocannon as one slot (Tactical Operations: Advanced Units & Equipment
    /// p. 195, note Q), and a B-Pod by its one slot (TechManual p. 317, note ‡). `None` for
    /// every other weapon.
    pub fn defensive_gauss_slots(self) -> Option<u8> {
        if matches!(self, Self::HyperAc2 | Self::HyperAc5 | Self::HyperAc10) || self.is_b_pod() {
            return Some(1);
        }
        (self.weapon_explosion_damage() > 0).then_some(self.profile().critical_slots)
    }

    /// Weapons whose BV counts toward the Defensive Battle Rating rather than the offensive
    /// rating (the † entries of TechManual pp. 317-318 and Tactical Operations: Advanced Units
    /// & Equipment p. 194): anti-missile systems, laser anti-missile systems, A-Pods and
    /// B-Pods.
    pub fn is_defensive_equipment(self) -> bool {
        self.is_ams() || matches!(self, Self::APod | Self::ClanAPod) || self.is_b_pod()
    }

    /// Whether this weapon's ammunition is explosive for the Defensive Battle Rating
    /// (TechManual pp. 317-318, § entries): Gauss and plasma ammunition is not, nor is the
    /// ammunition of a weapon that has none.
    pub fn ammunition_is_explosive(self) -> bool {
        self.profile().ammunition_per_ton > 0
            && self.weapon_explosion_damage() == 0
            && !self.is_plasma()
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
    /// Clan Artemis V fire control: one and a half tons in two slots per launcher (Tactical
    /// Operations: Advanced Units & Equipment p. 217).
    ArtemisV,
    /// Inner Sphere MRM "Apollo" fire control: a ton in one slot per MRM launcher, linked to
    /// its launcher like Artemis IV (Tactical Operations: Advanced Units & Equipment pp. 142,
    /// 219).
    ApolloFcs,
    Ecm,
    AngelEcm,
    StealthArmor,
    /// Filler slots claimed by laser-reflective armor; the armor itself is a chassis technology.
    LaserReflective,
    NullSignature,
    BeagleProbe,
    Tag,
    /// Clan light TAG: half a ton in one slot with a 3/6/9 range (TechManual p. 343).
    LightTag,
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
    /// Clan talons: two slots in every leg, a ton per fifteen tons of Mech in all, raising
    /// kick and death from above damage by half (Tactical Operations: Advanced Units &
    /// Equipment pp. 100, 103).
    Talons,
    /// A machine gun array: one slot linking two to four machine guns of one size class in its
    /// location, half a ton (Inner Sphere) or a quarter ton (Clan) (TechManual pp. 228,
    /// 341-343).
    MachineGunArray,
    /// An Inner Sphere spike cluster: half a ton in one slot, at most one per location,
    /// blunting physical attacks against its location (Tactical Operations: Advanced Units &
    /// Equipment pp. 100, 103).
    Spikes,
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

    /// Look up a system slot name, or `None` for weapons, ammunition and unknown parts. A
    /// system both technology bases build may carry an `IS.` or `CL.` prefix stating its base
    /// (see [`System::installation`]).
    ///
    /// Validation and critical-slot scans ask this of every slot on a unit, most of which
    /// are not systems, so the miss returns no error: building one per slot would format a
    /// message and, when backtraces are enabled, capture a stack trace each time.
    pub fn named(name: &str) -> Option<Self> {
        Self::installation(name).map(|(system, _)| system)
    }

    /// Look up a system slot name without a technology prefix.
    pub(crate) fn unprefixed(name: &str) -> Option<Self> {
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
            name if name.eq_ignore_ascii_case("ArtemisV") => Some(Self::ArtemisV),
            name if name.eq_ignore_ascii_case("ApolloFCS") => Some(Self::ApolloFcs),
            name if name.eq_ignore_ascii_case("Ecm") => Some(Self::Ecm),
            name if name.eq_ignore_ascii_case("AngelEcm") => Some(Self::AngelEcm),
            name if name.eq_ignore_ascii_case("StealthArmor") => Some(Self::StealthArmor),
            name if name.eq_ignore_ascii_case("LaserReflective") => Some(Self::LaserReflective),
            name if name.eq_ignore_ascii_case("NullSig_Device") => Some(Self::NullSignature),
            name if name.eq_ignore_ascii_case("TAG") => Some(Self::Tag),
            name if name.eq_ignore_ascii_case("LightTAG") => Some(Self::LightTag),
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
            name if name.eq_ignore_ascii_case("Talons") => Some(Self::Talons),
            name if name.eq_ignore_ascii_case("Spikes") => Some(Self::Spikes),
            name if name.eq_ignore_ascii_case("MGA") => Some(Self::MachineGunArray),
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

    /// ATMs and Streak LRMs fire directly only (Total Warfare p. 141; Tactical Operations:
    /// Advanced Units & Equipment p. 139), so neither can hotload; standard LRMs do both.
    #[test]
    fn atms_and_streak_lrms_fire_directly_only() {
        for weapon in [W::ClanAtm3, W::ClanAtm12, W::ClanStreakLrm5, W::StreakSrm2] {
            assert!(!weapon.supports_indirect_fire(), "{weapon:?}");
            assert!(!weapon.supports_hotload(), "{weapon:?}");
        }
        for weapon in [W::Lrm5, W::ClanLrm20, W::Nlrm10, W::Elrm5, W::Mml3] {
            assert!(weapon.supports_indirect_fire(), "{weapon:?}");
            assert!(weapon.supports_hotload(), "{weapon:?}");
        }
    }

    /// Indirect artillery reaches its Tactical Operations p. 149 mapsheet count at seventeen
    /// hexes per sheet, while artillery cannons keep their direct-fire hex ranges.
    #[test]
    fn artillery_reach_converts_mapsheets_to_hexes() {
        for (weapon, sheets) in [
            (W::ArrowIv, 8),
            (W::ClanArrowIv, 9),
            (W::Thumper, 21),
            (W::Sniper, 18),
            (W::LongTom, 30),
        ] {
            assert!(weapon.is_indirect_artillery());
            assert_eq!(
                weapon.effective_range(),
                sheets * super::ARTILLERY_MAPSHEET_HEXES,
                "{weapon:?}"
            );
        }
        for (weapon, hexes) in [
            (W::LongTomCannon, 20),
            (W::SniperCannon, 12),
            (W::ThumperCannon, 14),
        ] {
            assert!(weapon.is_artillery() && !weapon.is_indirect_artillery());
            assert_eq!(weapon.effective_range(), hexes, "{weapon:?}");
        }
        assert_eq!(W::Thumper.profile().heat, 6);
    }

    /// A BattleMech cannot mount a Long Tom (Tactical Operations: Advanced Units & Equipment
    /// p. 217), and only launchers with a one-shot version take the OneShot mode (TechManual
    /// p. 291 and the BV tables' one-shot figures).
    #[test]
    fn mech_mounting_follows_mech_space_and_one_shot_versions() {
        let jenner = |weapon: W, modes: &[&str]| {
            let mut template =
                crate::MechTemplate::parse("JR7-D", include_str!("../tests/fixtures/JR7-D.toml"))
                    .unwrap();
            let critical = template
                .sections
                .get_mut(&crate::MechSection::CenterTorso)
                .unwrap()
                .criticals
                .get_mut(&10)
                .unwrap();
            critical.equipment = weapon.name().into();
            critical.modes = modes.iter().map(|mode| (*mode).into()).collect();
            crate::MechLoadout::resolve(&template)
        };
        assert!(!W::LongTom.mech_mountable());
        let error = format!("{:#}", jenner(W::LongTom, &[]).unwrap_err());
        assert!(
            error.contains("cannot be mounted on a BattleMech"),
            "{error}"
        );
        assert!(jenner(W::Srm4, &["OneShot"]).is_ok());
        assert!(jenner(W::ClanAtm3, &["OneShot"]).is_err());
        assert!(jenner(W::Mml3, &["OneShot"]).is_err());
        for weapon in [W::Lrm20, W::ClanStreakSrm2, W::NarcBeacon, W::Thunderbolt5] {
            assert!(weapon.one_shot_capable(), "{weapon:?}");
        }
        for weapon in [W::ClanAtm12, W::Mml9, W::Rocket10, W::MediumLaser] {
            assert!(!weapon.one_shot_capable(), "{weapon:?}");
        }
    }

    /// Gauss-type weapons count their slots against the Defensive Battle Rating, the
    /// hyper-velocity autocannon as a single slot; their ammunition is inert.
    #[test]
    fn defensive_battle_rating_facts_follow_the_bv_notes() {
        assert_eq!(W::GaussRifle.defensive_gauss_slots(), Some(7));
        assert_eq!(W::ClanHag40.defensive_gauss_slots(), Some(10));
        assert_eq!(W::ClanApGaussRifle.defensive_gauss_slots(), Some(1));
        assert_eq!(W::HyperAc10.defensive_gauss_slots(), Some(1));
        assert_eq!(W::Ac20.defensive_gauss_slots(), None);
        assert!(W::Ac20.ammunition_is_explosive());
        assert!(!W::GaussRifle.ammunition_is_explosive());
        assert!(!W::ClanPlasmaCannon.ammunition_is_explosive());
        assert!(!W::MediumLaser.ammunition_is_explosive());
        assert!(W::AntiMissileSystem.is_defensive_equipment());
        assert!(W::ClanLaserAms.is_defensive_equipment());
        assert!(W::APod.is_defensive_equipment());
        assert!(W::ClanBPod.is_defensive_equipment());
        assert_eq!(W::BPod.defensive_gauss_slots(), Some(1));
        assert_eq!(W::BPod.pod_explosion_damage(), 2);
        assert_eq!(W::BPod.weapon_explosion_damage(), 0);
        assert!(!W::BPod.is_direct_fire() && !W::BPod.is_energy());
        assert_eq!((W::BPod.mass(), W::BPod.cost()), (1024, 2500));
        assert!(!W::Lrm5.is_defensive_equipment());
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
