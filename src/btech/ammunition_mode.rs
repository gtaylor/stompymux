//! Typed ammunition selection, separate from live weapon firing modes and ammunition quantities.
use super::{BattleUnit, BattleWeapon};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
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
    pub(super) fn supports(self, weapon: BattleWeapon) -> bool {
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
    pub(super) fn from_flag(weapon: BattleWeapon, flag: &str) -> Option<Self> {
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
    pub(super) fn initial_selection(weapon: BattleWeapon, flags: &[String]) -> Self {
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
    pub(super) fn from_flags(weapon: BattleWeapon, flags: &[String]) -> Result<Self> {
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

    /// Resolve a selected artillery ammunition type into its delayed arrival effect.
    pub fn artillery_payload(self) -> Result<super::BattleArtilleryMode> {
        Ok(match self {
            Self::Normal => super::BattleArtilleryMode::Standard,
            Self::Cluster => super::BattleArtilleryMode::Cluster,
            Self::Smoke => super::BattleArtilleryMode::Smoke,
            Self::Mine => super::BattleArtilleryMode::Mine,
            _ => anyhow::bail!("Ammunition is not an artillery payload"),
        })
    }

    /// Artemis command feedback distinguishes compatible missiles from ordinary ammunition.
    pub(crate) fn artemis_message(self, index: usize) -> String {
        if self.munition() == Self::Artemis {
            return self.message(index);
        }
        format!("Weapon {index} has been set to fire normal missiles")
    }

    /// Shared mode-switch feedback for native and Lua callers.
    pub(crate) fn message(self, index: usize) -> String {
        if self.munition() == Self::Artemis {
            return format!("Weapon {index} has been set to fire Artemis IV compatible missiles.");
        }
        format!(
            "Weapon {index} has been set to {} fire mode",
            if self == Self::Cluster {
                "LBX"
            } else {
                "normal"
            }
        )
    }
}

impl BattleUnit {
    /// Current selected ammunition type; independent of bin inventory and recycle readiness.
    pub fn ammunition_mode(&self, index: usize) -> Result<BattleAmmunitionMode> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(self
            .ammunition_modes
            .get(&index)
            .copied()
            .unwrap_or_default())
    }
}

/// Toggle an intact, recycled LB-X autocannon between slug and cluster ammunition.
pub fn toggle_lbx(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let readiness = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(readiness.weapon.is_lbx(), "That weapon cannot be set LBX!");
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Cluster,
    ))
}

/// Toggle artillery cluster rounds, preserving an existing smoke or mine selection.
pub fn toggle_cluster(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let readiness = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    let current = if let Some(vehicle) = world.btech.vehicles().get(&id) {
        vehicle.ammunition_mode(index)?
    } else {
        world.btech.constructed_units()[&id].ammunition_mode(index)?
    };
    ensure!(readiness.weapon.is_artillery(), "Invalid weapon type!");
    ensure!(
        matches!(
            current,
            BattleAmmunitionMode::Normal | BattleAmmunitionMode::Cluster
        ),
        "That weapon has already been set to fire special rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Cluster,
    ))
}

impl BattleAmmunitionMode {
    /// Artillery cluster feedback retains its distinct normal-round wording.
    pub(crate) fn cluster_message(self, index: usize) -> String {
        if self == Self::Cluster {
            return format!("Weapon {index} has been set to fire cluster rounds.");
        }
        format!("Weapon {index} has been set to fire normal rounds")
    }
}

/// Apply ordered cockpit selections through the shared weapon-control parser.
pub(crate) fn cluster_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_cluster(world, id, pilot, index).map(|mode| mode.cluster_message(index))
    })
}

/// Use the same bounded, ordered cockpit selection parser as other weapon mode commands.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_lbx(world, id, pilot, index).map(|mode| mode.message(index))
    })
}

/// Toggle compatible missile ammunition; controller lookup is required even when disabling the mode.
pub fn toggle_artemis(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let readiness = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    let operational = if let Some(vehicle) = world.btech.vehicles().get(&id) {
        vehicle.artemis_operational(index)?
    } else {
        world.btech.constructed_units()[&id].artemis_operational(index)?
    };
    ensure!(
        operational,
        "You do not have an Artemis system for that weapon."
    );
    ensure!(
        super::weapon_controls::selectable_munition(
            world,
            id,
            index,
            BattleAmmunitionMode::Artemis
        ) && !readiness.weapon.is_rocket(),
        "That weapon cannot be set ARTEMIS!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Artemis,
    ))
}

/// Native cockpit selection uses the same ordering and partial-error behavior as other modes.
pub(crate) fn artemis_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_artemis(world, id, pilot, index).map(|mode| mode.artemis_message(index))
    })
}

impl BattleAmmunitionMode {
    /// These missile supplies do not trigger automatic defensive interception.
    pub(super) fn bypasses_ams(self) -> bool {
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
