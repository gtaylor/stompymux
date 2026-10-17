//! Narc beacon attachment, surviving-section transfer and compatible missile controls.
use super::{AmmunitionMode, HitArc, Mech, MechSection, Notice, Weapon};
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
    /// An iNarc Nemesis pod: friendly guided missiles divert to the unit carrying it (Total
    /// Warfare p. 142).
    Nemesis,
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
            AmmunitionMode::INarcNemesis => Some(BeaconKind::Nemesis),
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
    )?;
    let partial_cover = super::unit_terrain_los(world, shooter, target)?.partial_cover;
    let mut dice = unit.dice.clone();
    let roll = dice.generic_roll();
    let location = super::resolve_hit(unit, arc, roll, &mut dice)?.section;
    // A pod aimed at legs behind partial cover sticks to the cover instead.
    let mut section = (!(partial_cover && unit.chassis().is_leg(location))).then_some(location);
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
                    text: "Your Anti-Missile System activates and destroys the incoming pod!"
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

/// Whether a Nemesis pod draws these missiles: Artemis-guided, semi-guided and Narc-equipped
/// rounds (Total Warfare p. 142).
fn drawn_by_nemesis(ammunition: AmmunitionMode) -> bool {
    matches!(
        ammunition.munition(),
        AmmunitionMode::Artemis | AmmunitionMode::SemiGuided | AmmunitionMode::Narc
    )
}

/// The unit a Nemesis pod diverts a direct attack onto, if any (Total Warfare p. 142). When a
/// unit fires Artemis-guided, semi-guided or Narc-equipped missiles, a unit on its own side
/// carrying a Nemesis pod draws the attack if it stands in a hex the line from the attacker to
/// the target crosses, the attacker has a clear line of sight to it, and it is not under the
/// water; the closest such unit draws it first. The attack then strikes that unit instead, at
/// +1. A battlefield that forbids friendly fire diverts nothing.
pub(super) fn nemesis_decoy(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    ammunition: AmmunitionMode,
) -> Result<Option<ObjectId>> {
    if !drawn_by_nemesis(ammunition) {
        return Ok(None);
    }
    let source = super::scanner::scanner_unit(world, shooter).context("Shooter is unavailable")?;
    let (Some(from), Some(to)) = (
        source.position,
        super::scanner::scanner_unit(world, target).and_then(|unit| unit.position),
    ) else {
        return Ok(None);
    };
    if from.map != to.map || world.btech.maps()[&from.map].blocks_friendly_fire() {
        return Ok(None);
    }
    let hex = |position: super::Position| super::HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    };
    let cells = super::los_trace::center_trace(hex(from), hex(to))?;
    let crossed = cells.get(1..cells.len().saturating_sub(1)).unwrap_or(&[]);
    let mut decoy: Option<(u64, ObjectId)> = None;
    for id in super::map_slots::all_unit_order(world, from.map)? {
        if id == shooter || id == target || !has_beacon(world, id, BeaconKind::Nemesis) {
            continue;
        }
        let Some(unit) = super::scanner::scanner_unit(world, id) else {
            continue;
        };
        if unit.destroyed
            || unit.signature.team != source.signature.team
            || !unit
                .position
                .is_some_and(|position| crossed.contains(&hex(position)))
            || super::los::unit_underwater(world, id)?
            || !super::unit_terrain_los(world, shooter, id)?.clear_line()
        {
            continue;
        }
        let distance = super::unit_range(world, shooter, id)?.hex_distance;
        if decoy.is_none_or(|(closest, _)| distance < closest) {
            decoy = Some((distance, id));
        }
    }
    Ok(decoy.map(|(_, id)| id))
}

/// The to-hit modifier for an attack a Nemesis pod diverted (Total Warfare p. 142).
pub(super) const NEMESIS_DIVERSION: i8 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    /// Each iNarc round attaches its own pod, Nemesis included; explosive rounds attach none,
    /// and a Narc launcher's beacon is a standard Narc pod.
    #[test]
    fn inarc_rounds_attach_their_own_pods() {
        for (mode, kind) in [
            (AmmunitionMode::Normal, Some(BeaconKind::Homing)),
            (AmmunitionMode::INarcHaywire, Some(BeaconKind::Haywire)),
            (AmmunitionMode::INarcEcm, Some(BeaconKind::Ecm)),
            (AmmunitionMode::INarcNemesis, Some(BeaconKind::Nemesis)),
            (AmmunitionMode::INarcExplosive, None),
        ] {
            assert_eq!(Weapon::INarcBeacon.beacon_kind(mode), kind, "{mode:?}");
        }
        assert_eq!(
            Weapon::NarcBeacon.beacon_kind(AmmunitionMode::Normal),
            Some(BeaconKind::Narc)
        );
        assert_eq!(
            Weapon::MediumLaser.beacon_kind(AmmunitionMode::Normal),
            None
        );
    }

    /// Only Artemis-guided, semi-guided and Narc-equipped missiles answer a Nemesis pod.
    #[test]
    fn nemesis_pods_draw_only_guided_missiles() {
        for (mode, drawn) in [
            (AmmunitionMode::Artemis, true),
            (AmmunitionMode::SemiGuided, true),
            (AmmunitionMode::Narc, true),
            (AmmunitionMode::MmlLrmArtemis, true),
            (AmmunitionMode::Normal, false),
            (AmmunitionMode::Swarm, false),
        ] {
            assert_eq!(drawn_by_nemesis(mode), drawn, "{mode:?}");
        }
    }
}
