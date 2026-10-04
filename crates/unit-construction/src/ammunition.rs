//! Typed ammunition selection, separate from live weapon firing modes and ammunition quantities.
use super::BattleWeapon;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Ammunition types understood by the supported weapons; normal is implicit in live state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleAmmunitionMode {
    #[default]
    Normal,
    Cluster,
    Smoke,
    Mine,
    Artemis,
    Narc,
    SemiGuided,
    Swarm,
    Swarm1,
    /// Anti-air indirect missiles with additional reach.
    Stinger,
    /// Long-range ammunition dedicated to a multi-missile launcher.
    MmlLrm,
    /// MML long-range rounds compatible with Artemis IV guidance.
    MmlLrmArtemis,
    /// MML long-range rounds compatible with Narc beacons.
    MmlLrmNarc,
    /// MML long-range Swarm rounds.
    MmlLrmSwarm,
    /// MML long-range Swarm-I rounds that skip friendly units.
    MmlLrmSwarm1,
    /// MML long-range semi-guided rounds.
    MmlLrmSemiGuided,
    /// MML long-range anti-air Stinger rounds.
    MmlLrmStinger,
    ExtendedRange,
    HighExplosive,
    INarcExplosive,
    INarcHaywire,
    INarcEcm,
    INarcNemesis,
    Precision,
    Flechette,
    ArmorPiercing,
    Caseless,
    Incendiary,
    Inferno,
    /// Thunder rounds that seed a weaker minefield across the target and adjacent hexes.
    ThunderAugmented,
    /// Thunder rounds that lay a vibrabomb field keyed to the firing unit's weight.
    ThunderVibrabomb,
    /// Thunder rounds whose mines also catch hovering and low-flying units.
    ThunderActive,
}

impl BattleAmmunitionMode {
    /// Equipment compatibility, shared by templates, saved units, controls and damage grouping.
    /// Installed controllers and disposable-weapon restrictions belong to live controls.
    pub fn supports(self, weapon: BattleWeapon) -> bool {
        // Torpedo launchers carry ordinary or Artemis-guided torpedoes only.
        if weapon.is_torpedo() {
            return matches!(self, Self::Normal | Self::Artemis);
        }
        match self {
            Self::Normal => true,
            Self::MmlLrm
            | Self::MmlLrmArtemis
            | Self::MmlLrmNarc
            | Self::MmlLrmSwarm
            | Self::MmlLrmSwarm1
            | Self::MmlLrmSemiGuided
            | Self::MmlLrmStinger => weapon.is_mml(),
            Self::ExtendedRange | Self::HighExplosive => weapon.is_atm(),
            Self::Inferno => weapon.profile().missiles > 0,
            Self::SemiGuided | Self::Stinger => weapon.supports_semiguided(),
            Self::Swarm | Self::Swarm1 => weapon.supports_semiguided() && !weapon.is_dead_fire(),
            Self::INarcExplosive | Self::INarcHaywire | Self::INarcEcm | Self::INarcNemesis => {
                weapon == BattleWeapon::INarcBeacon
            }
            Self::Cluster => weapon.is_lbx() || weapon.is_artillery(),
            Self::Smoke | Self::Mine => weapon.is_artillery() || weapon.profile().missiles > 0,
            Self::ThunderAugmented | Self::ThunderVibrabomb | Self::ThunderActive => {
                weapon.supports_thunder()
            }
            Self::Artemis => weapon.profile().missiles > 0,
            Self::Narc => {
                weapon.profile().missiles > 0
                    || matches!(
                        weapon,
                        BattleWeapon::AntiMissileSystem | BattleWeapon::ClanAntiMissileSystem
                    )
            }
            Self::Precision
            | Self::Flechette
            | Self::ArmorPiercing
            | Self::Caseless
            | Self::Incendiary => weapon.supports_rapid_fire(),
        }
    }

    /// Decode a single ammunition flag without interpreting firing or mounting flags.
    pub fn from_flag(weapon: BattleWeapon, flag: &str) -> Option<Self> {
        match flag {
            "LBX/Cluster" if !weapon.is_artillery() => Some(Self::Cluster),
            "Cluster" if weapon.is_artillery() => Some(Self::Cluster),
            "Smoke" => Some(Self::Smoke),
            "Mine" => Some(Self::Mine),
            "Artemis/Mine" => Some(Self::Artemis),
            "Sguided" => Some(Self::SemiGuided),
            "Swarm" => Some(Self::Swarm),
            "Swarm1" => Some(Self::Swarm1),
            "Stinger" => Some(Self::Stinger),
            "MML_LRM" => Some(Self::MmlLrm),
            "ExtendedRange" => Some(Self::ExtendedRange),
            "HighExplosive" => Some(Self::HighExplosive),
            "Narc/Smoke" => Some(Self::Narc),
            "iNarc_Explosive" => Some(Self::INarcExplosive),
            "iNarc_Haywire" => Some(Self::INarcHaywire),
            "iNarc_ECM" => Some(Self::INarcEcm),
            "iNarc_Nemesis" => Some(Self::INarcNemesis),
            "Inferno" => Some(Self::Inferno),
            "Incendiary" => Some(Self::Incendiary),
            "Caseless" => Some(Self::Caseless),
            "AP" => Some(Self::ArmorPiercing),
            "Precision" => Some(Self::Precision),
            "Flechette" => Some(Self::Flechette),
            "ThunderAug" => Some(Self::ThunderAugmented),
            "ThunderVibra" => Some(Self::ThunderVibrabomb),
            "ThunderActive" => Some(Self::ThunderActive),
            _ => None,
        }
    }

    /// Mounted weapons may carry several mode bits; preserve their ammunition precedence.
    pub fn initial_selection(weapon: BattleWeapon, flags: &[String]) -> Self {
        let mode = [
            "AP",
            "Precision",
            "Flechette",
            "Caseless",
            "Incendiary",
            "Cluster",
            "Smoke",
            "Mine",
            "LBX/Cluster",
            "Artemis/Mine",
            "Narc/Smoke",
            "Inferno",
            "iNarc_Explosive",
            "iNarc_Haywire",
            "iNarc_ECM",
            "iNarc_Nemesis",
            "Swarm",
            "Swarm1",
            "Sguided",
            "Stinger",
            "MML_LRM",
            "ExtendedRange",
            "HighExplosive",
            "ThunderAug",
            "ThunderVibra",
            "ThunderActive",
        ]
        .into_iter()
        .filter(|flag| !weapon.is_mml() || *flag != "MML_LRM")
        .find(|flag| flags.iter().any(|value| value == flag))
        .and_then(|flag| Self::from_flag(weapon, flag))
        .unwrap_or_default();
        if !weapon.is_mml() {
            return mode;
        }
        // MMLs select the flagged family; a round that family cannot carry falls back to normal.
        let long_range = flags.iter().any(|flag| flag == "MML_LRM");
        mode.with_mml_family(long_range)
            .or_else(|| Self::Normal.with_mml_family(long_range))
            .unwrap_or_default()
    }

    /// Read a bin's single ammunition type; conflicting or unrelated flags are invalid.
    pub fn from_flags(weapon: BattleWeapon, flags: &[String]) -> Result<Self> {
        if flags.is_empty() {
            return Ok(Self::Normal);
        }
        if let [flag] = flags
            && let Some(mode) = Self::from_flag(weapon, flag)
            && mode.supports(weapon)
        {
            return Ok(mode);
        }
        // MML long-range bins may pair the family flag with one compatible special round.
        if weapon.is_mml()
            && let [first, second] = flags
            && let Some(special) = [first, second].into_iter().find(|flag| *flag != "MML_LRM")
            && [first, second].into_iter().any(|flag| flag == "MML_LRM")
            && let Some(mode) = Self::from_flag(weapon, special)
            && let Some(mode) = mode.with_mml_family(true)
        {
            return Ok(mode);
        }
        anyhow::bail!(
            "Unsupported ammunition mode {:?} for {}",
            flags,
            weapon.name()
        )
    }
}

impl BattleAmmunitionMode {
    /// These missile supplies do not trigger automatic defensive interception.
    pub fn bypasses_ams(self) -> bool {
        matches!(
            self.munition(),
            Self::Swarm
                | Self::Swarm1
                | Self::Mine
                | Self::ThunderAugmented
                | Self::ThunderVibrabomb
                | Self::ThunderActive
        )
    }
}

impl BattleAmmunitionMode {
    /// Whether this supply belongs to the MML long-range family, with or without a special round.
    pub fn is_mml_lrm(self) -> bool {
        matches!(
            self,
            Self::MmlLrm
                | Self::MmlLrmArtemis
                | Self::MmlLrmNarc
                | Self::MmlLrmSwarm
                | Self::MmlLrmSwarm1
                | Self::MmlLrmSemiGuided
                | Self::MmlLrmStinger
        )
    }

    /// The special round independent of MML family; ordinary long-range MML rounds are normal.
    /// Combat rules keyed on a special round consult this instead of the stored supply.
    pub fn munition(self) -> Self {
        match self {
            Self::MmlLrm => Self::Normal,
            Self::MmlLrmArtemis => Self::Artemis,
            Self::MmlLrmNarc => Self::Narc,
            Self::MmlLrmSwarm => Self::Swarm,
            Self::MmlLrmSwarm1 => Self::Swarm1,
            Self::MmlLrmSemiGuided => Self::SemiGuided,
            Self::MmlLrmStinger => Self::Stinger,
            mode => mode,
        }
    }

    /// Combine this round's munition with an MML family. Returns `None` when the family
    /// cannot carry the round: long-range supplies exclude SRM-only rounds such as Inferno,
    /// and short-range supplies exclude LRM-only guidance and Swarm rounds.
    pub fn with_mml_family(self, long_range: bool) -> Option<Self> {
        let munition = self.munition();
        if !long_range {
            return (!matches!(
                munition,
                Self::Swarm | Self::Swarm1 | Self::SemiGuided | Self::Stinger
            ) && munition.supports(BattleWeapon::Mml3))
            .then_some(munition);
        }
        Some(match munition {
            Self::Normal => Self::MmlLrm,
            Self::Artemis => Self::MmlLrmArtemis,
            Self::Narc => Self::MmlLrmNarc,
            Self::Swarm => Self::MmlLrmSwarm,
            Self::Swarm1 => Self::MmlLrmSwarm1,
            Self::SemiGuided => Self::MmlLrmSemiGuided,
            Self::Stinger => Self::MmlLrmStinger,
            _ => return None,
        })
    }

    /// Replace the special round while keeping this supply's MML family, so guidance fallbacks
    /// such as blocked Artemis retain long-range damage and grouping.
    pub fn with_munition(self, munition: Self) -> Self {
        if self.is_mml_lrm() {
            munition.with_mml_family(true).unwrap_or(Self::MmlLrm)
        } else {
            munition
        }
    }

    /// Missile rounds that lay a minefield when fired at a hex. Plain mine rounds are the
    /// original Thunder munition.
    pub fn is_thunder(self) -> bool {
        matches!(
            self.munition(),
            Self::Mine | Self::ThunderAugmented | Self::ThunderVibrabomb | Self::ThunderActive
        )
    }

    /// Swarm rounds bypass interception and retain unused missiles between targets.
    pub fn is_swarm(self) -> bool {
        matches!(self.munition(), Self::Swarm | Self::Swarm1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bins_reject_conflicting_types_while_mounts_retain_selection_precedence() {
        let flags = vec!["Flechette".into(), "Precision".into()];
        assert!(BattleAmmunitionMode::from_flags(BattleWeapon::Ac5, &flags).is_err());
        assert_eq!(
            BattleAmmunitionMode::initial_selection(BattleWeapon::Ac5, &flags),
            BattleAmmunitionMode::Precision
        );
        let mounted = vec!["RapidFire".into(), "Flechette".into(), "RearMount".into()];
        assert_eq!(
            BattleAmmunitionMode::initial_selection(BattleWeapon::Ac5, &mounted),
            BattleAmmunitionMode::Flechette
        );
        assert!(BattleAmmunitionMode::from_flags(BattleWeapon::Ac5, &mounted).is_err());
        assert_eq!(
            BattleAmmunitionMode::initial_selection(BattleWeapon::Ac5, &["RearMount".into()]),
            BattleAmmunitionMode::Normal
        );
    }
}
