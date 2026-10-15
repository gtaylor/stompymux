//! Weapon families and ammunition-dependent profiles: catalogue facts that template
//! validation, ammunition compatibility and the server's combat rules all consult.
use crate::{AmmunitionMode, Weapon, WeaponProfile};

impl Weapon {
    /// Long- and short-range torpedo launchers, which fire only from a submerged mount.
    pub fn is_torpedo(self) -> bool {
        matches!(
            self,
            Self::Lrt5
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

    /// Defensive-only anti-missile systems; ballistic AMS draws from a matching bin, laser AMS from heat.
    pub fn is_ams(self) -> bool {
        matches!(
            self,
            Self::AntiMissileSystem
                | Self::ClanAntiMissileSystem
                | Self::LaserAms
                | Self::ClanLaserAms
        )
    }

    /// Clan Advanced Tactical Missile launchers.
    pub fn is_atm(self) -> bool {
        matches!(
            self,
            Self::ClanAtm3 | Self::ClanAtm6 | Self::ClanAtm9 | Self::ClanAtm12
        )
    }

    /// Extended Range missiles trade damage for reach and High Explosive missiles trade reach
    /// for damage; standard ammunition keeps the catalogue profile.
    pub fn atm_profile(mut profile: WeaponProfile, ammunition: AmmunitionMode) -> WeaponProfile {
        let (damage, minimum, short, medium, long) = match ammunition {
            AmmunitionMode::ExtendedRange => (1, 4, 9, 18, 27),
            AmmunitionMode::HighExplosive => (3, 0, 3, 6, 9),
            _ => return profile,
        };
        profile.damage = damage;
        profile.minimum_range = minimum;
        profile.short_range = short;
        profile.medium_range = medium;
        profile.long_range = long;
        profile
    }

    /// Standard LRM launchers: Inner Sphere, Clan and Enhanced (NLRM) racks, which share every
    /// LRM munition (Tactical Operations: Advanced Units & Equipment p. 139). Extended and Streak
    /// racks, Thunderbolts, MRMs, rockets and ATMs are not standard LRM launchers.
    pub fn is_standard_lrm(self) -> bool {
        matches!(
            self,
            Self::Lrm5
                | Self::Lrm10
                | Self::Lrm15
                | Self::Lrm20
                | Self::Nlrm5
                | Self::Nlrm10
                | Self::Nlrm15
                | Self::Nlrm20
                | Self::ClanLrm5
                | Self::ClanLrm10
                | Self::ClanLrm15
                | Self::ClanLrm20
        )
    }

    /// Standard Inner Sphere and Clan SRM launchers, which take the SRM munitions.
    pub fn is_standard_srm(self) -> bool {
        matches!(
            self,
            Self::Srm2 | Self::Srm4 | Self::Srm6 | Self::ClanSrm2 | Self::ClanSrm4 | Self::ClanSrm6
        )
    }

    /// LRM launchers that accept the specialized Thunder rounds: Total Warfare p. 141 limits
    /// Thunder (FASCAM) rounds to standard LRM launchers.
    pub fn supports_thunder(self) -> bool {
        self.is_standard_lrm()
    }

    /// Launchers whose rounds may carry Artemis IV or V guidance: standard LRM and SRM racks, MMLs
    /// and torpedo launchers (Total Warfare p. 131, TechManual p. 206). Streak, MRM, rocket, ATM,
    /// Thunderbolt, Extended LRM and Narc launchers are excluded.
    pub fn accepts_artemis(self) -> bool {
        self.is_standard_lrm() || self.is_standard_srm() || self.is_mml() || self.is_torpedo()
    }

    /// Launchers that load Narc-capable missiles: standard LRM and SRM racks and MMLs
    /// (Total Warfare pp. 141-142).
    pub fn accepts_narc_rounds(self) -> bool {
        self.is_standard_lrm() || self.is_standard_srm() || self.is_mml()
    }

    /// Machine guns, including their light and heavy versions, are the only weapons that may
    /// carry ammunition in half-ton lots (TechManual p. 48).
    pub fn allows_half_ton_bins(self) -> bool {
        self.supports_gatling()
    }

    /// Arrow IV, Long Tom, Sniper and Thumper pieces, whose catalogue reach is in mapsheets.
    /// Artillery cannons are direct-fire weapons with ordinary hex ranges.
    pub fn is_indirect_artillery(self) -> bool {
        matches!(
            self,
            Self::ArrowIv | Self::ClanArrowIv | Self::LongTom | Self::Sniper | Self::Thumper
        )
    }

    /// Multi-missile launchers accept dedicated short- and long-range supplies.
    pub fn is_mml(self) -> bool {
        matches!(self, Self::Mml3 | Self::Mml5 | Self::Mml7 | Self::Mml9)
    }

    /// Resolve ballistic facts once for aim, damage, interception and ammunition hazards.
    /// Normal MML ammunition is SRM; MML_LRM bins carry the long-range family. ATM Extended
    /// Range and High Explosive missiles change damage and ranges.
    pub fn profile_for_ammunition(self, ammunition: AmmunitionMode) -> WeaponProfile {
        let mut profile = self.profile();
        if self.is_atm() {
            return Self::atm_profile(profile, ammunition);
        }
        if self.is_mml() && ammunition.is_mml_lrm() {
            profile.damage = 1;
            profile.minimum_range = 6;
            profile.short_range = 7;
            profile.medium_range = 14;
            profile.long_range = 21;
            profile.ammunition_per_ton = 120 / profile.missiles;
        }
        profile
    }

    /// Observer eligibility follows the loaded family, while equipment eligibility stays weapon-based.
    pub fn supports_indirect_ammunition(self, ammunition: AmmunitionMode) -> bool {
        self.supports_indirect_fire() && (!self.is_mml() || ammunition.is_mml_lrm())
    }

    /// Hotloaded MMLs draw their selected family; other launchers retain ordinary hotload supply.
    pub fn hotload_supply_mode(self, selected: AmmunitionMode) -> AmmunitionMode {
        if self.is_mml() {
            selected
        } else {
            AmmunitionMode::Normal
        }
    }

    /// Internal bin damage uses its contents, independent of the launcher's selected supply.
    pub fn ammunition_explosion_damage_for_mode(
        self,
        rounds: u16,
        ammunition: AmmunitionMode,
    ) -> u32 {
        if self.weapon_explosion_damage() > 0 || self.is_plasma() {
            return 0;
        }
        let profile = self.profile_for_ammunition(ammunition);
        u32::from(rounds) * u32::from(profile.damage) * u32::from(profile.missiles.max(1))
    }

    /// The Inner Sphere plasma rifle and the Clan plasma cannon. Their ammunition never explodes
    /// and they cannot fire underwater (Total Warfare p. 139).
    pub fn is_plasma(self) -> bool {
        matches!(self, Self::PlasmaRifle | Self::ClanPlasmaCannon)
    }

    /// Clan hyper-assault Gauss rifles, which roll on the Cluster Hits Table and deal their
    /// damage in five-point groups (Total Warfare p. 136).
    pub fn is_hag(self) -> bool {
        matches!(self, Self::ClanHag20 | Self::ClanHag30 | Self::ClanHag40)
    }

    /// Conventional IS and Clan Narc launchers, with normal beacons or explosive rounds.
    pub fn is_narc(self) -> bool {
        matches!(self, Self::NarcBeacon | Self::ClanNarcBeacon)
    }

    /// Standard LRM launchers take semi-guided and Swarm rounds (Total Warfare p. 142;
    /// Tactical Operations: Advanced Units & Equipment p. 228). MML long-range bins carry their own
    /// family variants of these rounds instead.
    pub fn supports_semiguided(self) -> bool {
        self.is_standard_lrm()
    }

    /// Weapons whose impacts use delayed artillery area effects rather than conventional salvos.
    pub fn is_artillery(self) -> bool {
        matches!(
            self,
            Self::ArrowIv
                | Self::ClanArrowIv
                | Self::LongTom
                | Self::Sniper
                | Self::Thumper
                | Self::LongTomCannon
                | Self::SniperCannon
                | Self::ThumperCannon
        )
    }

    /// Whether ordinary ammunition can start a woodland fire.
    pub fn can_ignite_terrain(self) -> bool {
        !matches!(
            self,
            Self::INarcBeacon
                | Self::NarcBeacon
                | Self::ClanNarcBeacon
                | Self::ClanSrm2
                | Self::ClanStreakSrm2
                | Self::ClanGaussRifle
                | Self::ClanApGaussRifle
                | Self::ClanHag20
                | Self::ClanHag30
                | Self::ClanHag40
                | Self::ClanMachineGun
                | Self::ClanLightMachineGun
                | Self::ClanHeavyMachineGun
                | Self::ClanErSmallLaser
                | Self::ClanHeavySmallLaser
                | Self::ClanSmallPulseLaser
                | Self::ClanErSmallPulseLaser
                | Self::MachineGun
                | Self::LightMachineGun
                | Self::HeavyMachineGun
                | Self::SmallLaser
                | Self::ErSmallLaser
                | Self::SmallPulseLaser
                | Self::XSmallPulseLaser
                | Self::Srm2
                | Self::StreakSrm2
                | Self::HeavyGaussRifle
                | Self::GaussRifle
                | Self::LightGaussRifle
                | Self::MagshotGaussRifle
        )
    }

    /// Whether a sufficiently damaging shot can reduce woodland density.
    pub fn can_clear_terrain(self) -> bool {
        !matches!(
            self,
            Self::ClanLbx2
                | Self::ClanLbx5
                | Self::ClanUltraAc2
                | Self::ClanUltraAc5
                | Self::ClanSrm2
                | Self::ClanStreakSrm2
                | Self::ClanApGaussRifle
                | Self::ClanMachineGun
                | Self::ClanLightMachineGun
                | Self::ClanHeavyMachineGun
                | Self::ClanErSmallLaser
                | Self::ClanHeavySmallLaser
                | Self::ClanSmallPulseLaser
                | Self::ClanErSmallPulseLaser
                | Self::HyperAc2
                | Self::HyperAc5
                | Self::MachineGun
                | Self::LightMachineGun
                | Self::HeavyMachineGun
                | Self::LightAc2
                | Self::LightAc5
                | Self::SmallLaser
                | Self::ErSmallLaser
                | Self::SmallPulseLaser
                | Self::XSmallPulseLaser
                | Self::Srm2
                | Self::StreakSrm2
                | Self::Lbx2
                | Self::Lbx5
                | Self::Ac2
                | Self::Ac5
                | Self::UltraAc2
                | Self::UltraAc5
                | Self::RotaryAc2
                | Self::RotaryAc5
                | Self::ClanRotaryAc2
                | Self::ClanRotaryAc5
        )
    }
}
