//! Narc beacon attachment, surviving-section transfer and compatible missile controls.
use super::{AmmunitionMode, HitArc, HitRules, HitTable, Mech, MechSection, Notice, Weapon};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Persistent pod effects; several kinds can coexist on one surviving section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeaconKind {
    Narc,
    Homing,
    Haywire,
    Ecm,
}

/// Launch facts shared by all non-explosive pod variants.
pub(super) struct PodShot {
    pub kind: BeaconKind,
    pub hit: bool,
    pub intercepted: bool,
}

/// A launched beacon's outcome; attachment does not apply weapon damage or damage XP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NarcReport<S = MechSection> {
    pub kind: BeaconKind,
    pub hit: bool,
    pub intercepted: bool,
    pub section: Option<S>,
    pub rear: bool,
    /// Cockpit effects caused by the location roll, without armor damage.
    pub notices: Vec<Notice>,
    /// Raw location-event broadcasts, resolved against the pre-shot audience by the host.
    pub broadcasts: Vec<Notice>,
}

impl Mech {
    /// Attached pod kinds grouped by surviving section, with no team or expiry ownership.
    pub fn beacons(&self) -> &BTreeMap<MechSection, BTreeSet<BeaconKind>> {
        &self.beacons
    }

    /// Sections carrying conventional Narc beacons.
    pub fn narc_sections(&self) -> BTreeSet<MechSection> {
        self.beacons
            .iter()
            .filter(|(_, kinds)| kinds.contains(&BeaconKind::Narc))
            .map(|(&section, _)| section)
            .collect()
    }

    /// Any surviving section carrying this pod effect.
    pub fn has_beacon(&self, kind: BeaconKind) -> bool {
        self.beacons.values().any(|kinds| kinds.contains(&kind))
    }
}

/// Beacon effects that Narc and iNarc launchers attach on a hit.
pub(crate) trait BeaconLaunch {
    /// Select the attached effect; explosive ammunition uses ordinary salvo damage instead.
    fn beacon_kind(self, mode: AmmunitionMode) -> Option<BeaconKind>;
}

impl BeaconLaunch for Weapon {
    fn beacon_kind(self, mode: AmmunitionMode) -> Option<BeaconKind> {
        if self.is_narc() {
            return (mode.munition() != AmmunitionMode::Narc).then_some(BeaconKind::Narc);
        }
        if self != Self::INarcBeacon {
            return None;
        }
        match mode {
            AmmunitionMode::INarcExplosive => None,
            AmmunitionMode::INarcHaywire => Some(BeaconKind::Haywire),
            AmmunitionMode::INarcEcm => Some(BeaconKind::Ecm),
            _ => Some(BeaconKind::Homing),
        }
    }
}

/// Resolve a beacon location without damage, transferring off already destroyed sections.
pub(super) fn attach(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    shot: PodShot,
    rules: HitRules,
    hit_arc_mode: i64,
) -> Result<NarcReport> {
    let PodShot {
        kind,
        hit,
        intercepted,
    } = shot;
    let mut report = NarcReport {
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
    let arc = HitArc::from_bearing(
        range.bearing.unwrap_or(180.0),
        unit.motion().context("Target is not placed")?.heading,
        hit_arc_mode,
    )?;
    let partial_cover = super::unit_terrain_los(world, shooter, target)?.partial_cover;
    let mut dice = unit.dice.clone();
    let (location, crew_stun) = if partial_cover {
        (
            HitTable::Punch.location(unit.chassis(), arc, dice.d6())?,
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
    if section.is_some() && kind == BeaconKind::Haywire {
        report.notices.push(Notice {
            unit: target,
            text: "Your targetting system goes a bit haywire!".into(),
        });
    }
    if section.is_some() && kind == BeaconKind::Ecm {
        report
            .notices
            .extend(super::electronics::refresh_receiver(world, target)?);
    }
    report.section = section;
    report.rear = arc == HitArc::Rear;
    Ok(report)
}

impl<S> NarcReport<S> {
    /// Pod-specific cockpit feedback, including successful interception and absent attachment locations.
    fn messages(&self, shooter: ObjectId, target: ObjectId, section: Option<&str>) -> Vec<Notice> {
        if !self.hit {
            return vec![Notice {
                unit: shooter,
                text: "Your NARC Beacon flies off into the distance.".into(),
            }];
        }
        if self.intercepted {
            return vec![
                Notice {
                    unit: shooter,
                    text: "The pod is shot down by the target!".into(),
                },
                Notice {
                    unit: target,
                    text: "Your Anti-Missile System activates and shoots down the incoming pod!"
                        .into(),
                },
            ];
        }
        let Some(section) = section else {
            return vec![Notice {
                unit: shooter,
                text: "Your NARC Beacon attaches to the target!".into(),
            }];
        };
        let rear = if self.rear { " (Rear)" } else { "" };
        vec![
            Notice {
                unit: target,
                text: format!(
                    "A NARC Beacon has been attached to your {}{rear}!",
                    section.replace('_', " ")
                ),
            },
            Notice {
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
) -> Result<AmmunitionMode> {
    super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        super::weapon_controls::selectable_munition(world, id, index, AmmunitionMode::Narc),
        "That weapon cannot be set NARC!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        AmmunitionMode::Narc,
    ))
}

/// The explosive control is restricted to Narc launchers and uses their compatible-ammunition bit.
pub fn toggle_explosive(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<AmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        ready.weapon.is_narc(),
        "That weapon cannot be set to fire explosive rounds!"
    );
    toggle_narc(world, id, pilot, index)
}

/// Shared mode descriptions for native and Lua controls.
pub(crate) fn message(mode: AmmunitionMode, index: usize, explosive: bool) -> String {
    let text = match (explosive, mode.munition() == AmmunitionMode::Narc) {
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
pub enum UnitSection {
    Mech(MechSection),
    Vehicle(super::VehicleSection),
}

impl UnitSection {
    /// Human-readable section heading supplied by its own construction type.
    pub fn name(self) -> &'static str {
        match self {
            Self::Mech(section) => section.name(),
            Self::Vehicle(section) => section.name(),
        }
    }
}

impl NarcReport<UnitSection> {
    /// Mixed target reports use the same wording and their own section names.
    pub(super) fn notices(&self, shooter: ObjectId, target: ObjectId) -> Vec<Notice> {
        self.messages(shooter, target, self.section.map(UnitSection::name))
    }
}

impl<S> NarcReport<S> {
    /// Adapt only section identity while preserving every resolved consequence.
    pub(super) fn map_section<T>(self, map: impl FnOnce(S) -> T) -> NarcReport<T> {
        NarcReport {
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
pub(super) fn has_beacon(world: &World, id: ObjectId, kind: BeaconKind) -> bool {
    world
        .btech
        .unit(id)
        .is_some_and(|unit| unit.has_beacon(kind))
}
