//! Narc beacon attachment, surviving-section transfer and compatible missile controls.
use super::{
    BattleAmmunitionMode, BattleHitArc, BattleHitRules, BattleHitTable, BattleNotice,
    BattleSection, BattleUnit, BattleWeapon,
};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Persistent pod effects; several kinds can coexist on one surviving section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleBeaconKind {
    Narc,
    Homing,
    Haywire,
    Ecm,
}

/// Launch facts shared by all non-explosive pod variants.
pub(super) struct PodShot {
    pub kind: BattleBeaconKind,
    pub hit: bool,
    pub intercepted: bool,
}

/// A launched beacon's outcome; attachment does not apply weapon damage or damage XP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleNarcReport<S = BattleSection> {
    pub kind: BattleBeaconKind,
    pub hit: bool,
    pub intercepted: bool,
    pub section: Option<S>,
    pub rear: bool,
    /// Cockpit effects caused by the location roll, without armor damage.
    pub notices: Vec<BattleNotice>,
    /// Raw location-event broadcasts, resolved against the pre-shot audience by the host.
    pub broadcasts: Vec<BattleNotice>,
}

impl BattleWeapon {
    /// Conventional IS and Clan Narc launchers, with normal beacons or explosive rounds.
    pub fn is_narc(self) -> bool {
        matches!(self, Self::NarcBeacon | Self::ClanNarcBeacon)
    }
}

impl BattleUnit {
    /// Attached pod kinds grouped by surviving section, with no team or expiry ownership.
    pub fn beacons(&self) -> &BTreeMap<BattleSection, BTreeSet<BattleBeaconKind>> {
        &self.beacons
    }

    /// Sections carrying conventional Narc beacons.
    pub fn narc_sections(&self) -> BTreeSet<BattleSection> {
        self.beacons
            .iter()
            .filter(|(_, kinds)| kinds.contains(&BattleBeaconKind::Narc))
            .map(|(&section, _)| section)
            .collect()
    }

    /// Any surviving section carrying this pod effect.
    pub fn has_beacon(&self, kind: BattleBeaconKind) -> bool {
        self.beacons.values().any(|kinds| kinds.contains(&kind))
    }
}

impl BattleWeapon {
    /// Select the attached effect; explosive ammunition uses ordinary salvo damage instead.
    pub(super) fn beacon_kind(self, mode: BattleAmmunitionMode) -> Option<BattleBeaconKind> {
        if self.is_narc() {
            return (mode != BattleAmmunitionMode::Narc).then_some(BattleBeaconKind::Narc);
        }
        if self != Self::INarcBeacon {
            return None;
        }
        match mode {
            BattleAmmunitionMode::INarcExplosive => None,
            BattleAmmunitionMode::INarcHaywire => Some(BattleBeaconKind::Haywire),
            BattleAmmunitionMode::INarcEcm => Some(BattleBeaconKind::Ecm),
            _ => Some(BattleBeaconKind::Homing),
        }
    }
}

/// Resolve a beacon location without damage, transferring off already destroyed sections.
pub(super) fn attach(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    shot: PodShot,
    rules: BattleHitRules,
    hit_arc_mode: i64,
) -> Result<BattleNarcReport> {
    let PodShot {
        kind,
        hit,
        intercepted,
    } = shot;
    let mut report = BattleNarcReport {
        kind,
        hit,
        intercepted,
        section: None,
        rear: false,
        notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    if !hit || intercepted {
        return Ok(report);
    }
    let range = super::unit_range(world, target, shooter)?;
    let unit = &world.btech.constructed_units()[&target];
    let arc = BattleHitArc::from_bearing(
        range.bearing.unwrap_or(180.0),
        unit.motion().context("Target is not placed")?.heading,
        hit_arc_mode,
    )?;
    let partial_cover = super::unit_terrain_los(world, shooter, target)?.partial_cover;
    let mut dice = unit.dice.clone();
    let (location, crew_stun) = if partial_cover {
        (
            BattleHitTable::Punch.location(unit.chassis(), arc, dice.d6())?,
            false,
        )
    } else {
        let roll = dice.generic_roll();
        let hit = rules.resolve(unit, arc, roll, &mut dice)?;
        (hit.section, hit.crew_stun)
    };
    let mut section = Some(location);
    while let Some(current) = section {
        if unit.sections()[&current].internal > 0 {
            break;
        }
        section = current.damage_transfer();
    }
    let unit = world.btech.constructed.get_mut(&target).unwrap();
    unit.dice = dice;
    if let Some(section) = section {
        unit.beacons.entry(section).or_default().insert(kind);
    }
    if crew_stun {
        report.notices.push(super::stun_unit(world, target)?);
    }
    if section.is_some() && kind == BattleBeaconKind::Haywire {
        report.notices.push(BattleNotice {
            unit: target,
            text: "Your targetting system goes a bit haywire!".into(),
        });
    }
    if section.is_some() && kind == BattleBeaconKind::Ecm {
        report
            .notices
            .extend(super::electronics::refresh_receiver(world, target)?);
    }
    report.section = section;
    report.rear = arc == BattleHitArc::Rear;
    Ok(report)
}

impl<S> BattleNarcReport<S> {
    /// Pod-specific cockpit feedback, including successful interception and absent attachment locations.
    fn messages(
        &self,
        shooter: ObjectId,
        target: ObjectId,
        section: Option<&str>,
    ) -> Vec<BattleNotice> {
        if !self.hit {
            return vec![BattleNotice {
                unit: shooter,
                text: "Your NARC Beacon flies off into the distance.".into(),
            }];
        }
        if self.intercepted {
            return vec![
                BattleNotice {
                    unit: shooter,
                    text: "The pod is shot down by the target!".into(),
                },
                BattleNotice {
                    unit: target,
                    text: "Your Anti-Missile System activates and shoots down the incoming pod!"
                        .into(),
                },
            ];
        }
        let Some(section) = section else {
            return vec![BattleNotice {
                unit: shooter,
                text: "Your NARC Beacon attaches to the target!".into(),
            }];
        };
        let rear = if self.rear { " (Rear)" } else { "" };
        vec![
            BattleNotice {
                unit: target,
                text: format!(
                    "A NARC Beacon has been attached to your {}{rear}!",
                    section.replace('_', " ")
                ),
            },
            BattleNotice {
                unit: shooter,
                text: format!(
                    "Your NARC Beacon attaches to the target's {}{rear}!",
                    section.replace('_', " ")
                ),
            },
        ]
    }
}

/// Toggle compatible missiles, or explosive rounds on a Narc launcher.
pub fn toggle_narc(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        BattleAmmunitionMode::Narc.supports(ready.weapon),
        "That weapon cannot be set NARC!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Narc,
    ))
}

/// The explosive control is restricted to Narc launchers and uses their compatible-ammunition bit.
pub fn toggle_explosive(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        ready.weapon.is_narc(),
        "That weapon cannot be set to fire explosive rounds!"
    );
    toggle_narc(world, id, pilot, index)
}

/// Shared mode descriptions for native and Lua controls.
pub(crate) fn message(mode: BattleAmmunitionMode, index: usize, explosive: bool) -> String {
    let text = match (explosive, mode == BattleAmmunitionMode::Narc) {
        (true, true) => "explosive rounds",
        (true, false) => "NARC beacons",
        (false, true) => "Narc Beacon compatible missiles.",
        (false, false) => "normal missiles",
    };
    format!("Weapon {index} has been set to fire {text}")
}

/// Bounded cockpit selections share ordinary weapon-mode authorization and publication.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let explosive = input.name == "explosive";
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        let mode = if explosive {
            toggle_explosive(world, id, pilot, index)?
        } else {
            toggle_narc(world, id, pilot, index)?
        };
        Ok(message(mode, index, explosive))
    })
}

/// A typed section from either construction class, retaining native serialization for Lua reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum BattleUnitSection {
    Mech(BattleSection),
    Vehicle(super::BattleVehicleSection),
}

impl BattleUnitSection {
    /// Human-readable section heading supplied by its own construction type.
    pub fn name(self) -> &'static str {
        match self {
            Self::Mech(section) => section.name(),
            Self::Vehicle(section) => section.name(),
        }
    }
}

impl BattleNarcReport<BattleUnitSection> {
    /// Mixed target reports use the same wording and their own section names.
    pub(super) fn notices(&self, shooter: ObjectId, target: ObjectId) -> Vec<BattleNotice> {
        self.messages(shooter, target, self.section.map(BattleUnitSection::name))
    }
}

impl<S> BattleNarcReport<S> {
    /// Adapt only section identity while preserving every resolved consequence.
    pub(super) fn map_section<T>(self, map: impl FnOnce(S) -> T) -> BattleNarcReport<T> {
        BattleNarcReport {
            kind: self.kind,
            hit: self.hit,
            intercepted: self.intercepted,
            section: self.section.map(map),
            rear: self.rear,
            notices: self.notices,
            broadcasts: self.broadcasts,
        }
    }
}

/// Query attached effects through the same construction boundary used by targeting.
pub(super) fn has_beacon(world: &World, id: ObjectId, kind: BattleBeaconKind) -> bool {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return unit.has_beacon(kind);
    }
    world
        .btech
        .constructed_units()
        .get(&id)
        .is_some_and(|unit| unit.has_beacon(kind))
}
