//! Clubs (Total Warfare p. 146): trees uprooted from woods, 'Mech arms and legs that an
//! attack blew off, and girders searched out of a rubbled building; their pickup, release and
//! the hexes they lie in, and the arm holding one, shared by the physical commands.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// What a 'Mech swings as a club (Total Warfare p. 146).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClubKind {
    /// A tree uprooted from a wooded hex, good for one successful club attack.
    Tree,
    /// A 'Mech arm or leg blown off by an attack.
    Limb,
    /// A girder from the rubble of a medium, heavy or hardened building, which may be reused
    /// again and again.
    Girder,
}

impl ClubKind {
    /// Whether the club breaks on its first successful attack: an uprooted tree does, while a
    /// girder may be reused again and again and the book sets no limit on a limb (Total
    /// Warfare p. 146).
    pub fn breaks_on_hit(self) -> bool {
        self == Self::Tree
    }

    /// Lowercase name for cockpit and observer messages.
    pub fn name(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Limb => "severed limb",
            Self::Girder => "girder",
        }
    }
}

/// A club and the arm that holds it; swinging it needs both hands (Total Warfare p. 145).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarriedClub {
    pub arm: Arm,
    pub kind: ClubKind,
}

/// What lies in one hex for a 'Mech to take up as a club (Total Warfare p. 146): limbs blown
/// off 'Mechs there, limbs and girders a 'Mech let go of there, and the ruin of a building that
/// collapsed there, whose rubble may be searched for a girder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HexDebris {
    #[serde(default)]
    pub limbs: u16,
    #[serde(default)]
    pub girders: u16,
    /// The class of the building that collapsed into this hex's rubble.
    #[serde(default)]
    pub ruin: Option<ConstructionClass>,
}

impl HexDebris {
    /// Whether nothing lies here.
    pub fn is_empty(self) -> bool {
        self.limbs == 0 && self.girders == 0 && self.ruin.is_none()
    }

    /// The loose club a 'Mech picks up here: a limb first, then a girder.
    fn first(self) -> Option<ClubKind> {
        if self.limbs > 0 {
            Some(ClubKind::Limb)
        } else if self.girders > 0 {
            Some(ClubKind::Girder)
        } else {
            None
        }
    }

    /// This pile with one more `kind` in it; trees never lie loose.
    fn with(mut self, kind: ClubKind) -> Self {
        match kind {
            ClubKind::Limb => self.limbs = self.limbs.saturating_add(1),
            ClubKind::Girder => self.girders = self.girders.saturating_add(1),
            ClubKind::Tree => {}
        }
        self
    }

    /// This pile with one `kind` taken away.
    fn without(mut self, kind: ClubKind) -> Self {
        match kind {
            ClubKind::Limb => self.limbs = self.limbs.saturating_sub(1),
            ClubKind::Girder => self.girders = self.girders.saturating_sub(1),
            ClubKind::Tree => {}
        }
        self
    }
}

/// The 2D6 roll a search of the rubble of a building of `class` needs to find a girder: 7
/// for a medium building, 6 for a heavy one and 5 for a hardened one; a light building leaves
/// none (Total Warfare p. 146).
pub fn girder_target(class: ConstructionClass) -> Option<u8> {
    match class {
        ConstructionClass::Light => None,
        ConstructionClass::Medium => Some(7),
        ConstructionClass::Heavy => Some(6),
        ConstructionClass::Hardened => Some(5),
    }
}

impl Mech {
    /// The arm holding a club, independent of physical recovery.
    pub fn carried_club(&self) -> Option<Arm> {
        self.carried_club.map(|club| club.arm)
    }

    /// The club this 'Mech carries and the arm holding it.
    pub fn club(&self) -> Option<CarriedClub> {
        self.carried_club
    }

    /// Simulation time this 'Mech last searched rubble for a girder; it searches once a turn.
    pub fn girder_search(&self) -> Option<i64> {
        self.girder_search
    }
}

impl StoredMap {
    /// What lies in the hex at `coordinate` for a 'Mech to take up as a club.
    pub fn debris_at(&self, coordinate: HexCoordinate) -> Result<HexDebris> {
        let index = self.tile_index(coordinate)?;
        Ok(self.debris.get(&index).copied().unwrap_or_default())
    }

    /// Replace what lies in the hex at row-major `index`, forgetting it when nothing is left.
    fn set_debris(&mut self, index: u32, debris: HexDebris) {
        let all = std::sync::Arc::make_mut(&mut self.debris);
        if debris.is_empty() {
            all.remove(&index);
        } else {
            all.insert(index, debris);
        }
    }

    /// Forget the ruin under the hex at row-major `index` once the hex is no longer rubble.
    pub(super) fn clear_ruin(&mut self, index: u32) {
        let Some(&debris) = self.debris.get(&index) else {
            return;
        };
        if debris.ruin.is_some() {
            self.set_debris(
                index,
                HexDebris {
                    ruin: None,
                    ..debris
                },
            );
        }
    }
}

/// Note that the building hex at `coordinate` on `map` collapsed into rubble, so 'Mechs may
/// search it for girders (Total Warfare p. 146).
pub(super) fn record_ruin(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    class: ConstructionClass,
) -> Result<()> {
    let record = world.btech.maps.get_mut(&map).context("Map not found")?;
    let index = record.tile_index(coordinate)?;
    let debris = record.debris.get(&index).copied().unwrap_or_default();
    record.set_debris(
        index,
        HexDebris {
            ruin: Some(class),
            ..debris
        },
    );
    Ok(())
}

/// The hex a placed 'Mech stands in, as a map and coordinate.
fn unit_hex(world: &World, id: ObjectId) -> Option<(ObjectId, HexCoordinate)> {
    let position = world.btech.constructed_units().get(&id)?.position()?;
    Some((
        position.map,
        HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        },
    ))
}

/// Lay a limb or girder down in the hex the 'Mech `id` stands in; a tree is gone.
fn lay_down(world: &mut World, id: ObjectId, kind: ClubKind) -> Result<()> {
    if kind == ClubKind::Tree {
        return Ok(());
    }
    let Some((map, coordinate)) = unit_hex(world, id) else {
        return Ok(());
    };
    let record = world.btech.maps.get_mut(&map).context("Map not found")?;
    let index = record.tile_index(coordinate)?;
    let debris = record.debris.get(&index).copied().unwrap_or_default();
    record.set_debris(index, debris.with(kind));
    Ok(())
}

/// An arm or leg an attack blew off lies in the hex where the 'Mech took the damage, for any
/// 'Mech there to pick up as a club (Total Warfare p. 146).
pub(super) fn sever_limb(world: &mut World, id: ObjectId, section: MechSection) -> Result<()> {
    if !matches!(
        section,
        MechSection::LeftArm | MechSection::RightArm | MechSection::LeftLeg | MechSection::RightLeg
    ) {
        return Ok(());
    }
    lay_down(world, id, ClubKind::Limb)
}

/// Check the arm, shoulder and hand used when grabbing a club.
fn usable(unit: &Mech, arm: Arm) -> Result<bool> {
    let section = arm.section();
    if unit.sections()[&section].internal == 0 {
        return Ok(false);
    }
    let loadout = unit.loadout()?;
    for (slot, system) in [(0, System::ShoulderOrHip), (3, System::HandOrFootActuator)] {
        let location = CriticalLocation { section, slot };
        if unit.critical_unavailable(location)
            || !loadout
                .systems
                .iter()
                .any(|part| part.location == location && part.system == system)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Let go of `club`, released by the pilot or by a shutdown or damage: a tree falls and
/// shatters, while a limb or girder lies in the hex for any 'Mech to pick up again.
pub(super) fn dropped(world: &mut World, id: ObjectId, club: CarriedClub) -> Result<Vec<Notice>> {
    lay_down(world, id, club.kind)?;
    Ok(drop_notices(world, id, club.kind))
}

/// The messages for a club of `kind` that the 'Mech `id` let go of, wherever it fell.
pub(super) fn drop_notices(world: &World, id: ObjectId, kind: ClubKind) -> Vec<Notice> {
    let (own, seen) = if kind == ClubKind::Tree {
        (
            "Your club falls to the ground and shatters.",
            "'s club falls to the ground and shatters.",
        )
    } else {
        (
            "Your club falls to the ground.",
            "'s club falls to the ground.",
        )
    };
    let mut notices = vec![Notice {
        unit: id,
        text: own.into(),
    }];
    notices.extend(super::broadcast::observer_notices(world, id, seen));
    notices
}

/// What a 'Mech may take up in its hex, in the order it reaches for them: a limb or girder
/// lying there, a tree in woods, or a girder searched out of a rubbled building's ruins.
enum Source {
    Loose(ClubKind),
    Tree,
    Rubble(u8),
}

/// Take up a club in a selected arm (left first by default), or let it go with `-`. A 'Mech in
/// its hex may pick up a limb or girder lying there, uproot a tree in woods, or search the
/// rubble of a medium, heavy or hardened building for a girder, rolling 2D6 for 7, 6 or 5 or
/// more; it searches once a turn, and a failed search leaves it free to act (Total Warfare
/// p. 146). A 'Mech may not fire weapons or make physical attacks in the turn it picks up a
/// club (ibid.): a pickup or search is refused within thirty seconds of either, and a pickup
/// ties up every section for the next thirty.
pub fn grab_club(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    selection: Option<&str>,
) -> Result<Vec<Notice>> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(
        unit.power() == Power::Running && !unit.is_destroyed(),
        "Start the unit first"
    );
    if selection == Some("-") {
        let club = unit
            .carried_club
            .context("You aren't currently carrying a club.")?;
        return world.attempt(|world| {
            world.btech.constructed.get_mut(&id).unwrap().carried_club = None;
            dropped(world, id, club)
        });
    }
    ensure!(
        unit.chassis() != MechChassis::Quad,
        "Quads can't carry a club."
    );
    ensure!(
        unit.posture() != Posture::Prone,
        "You can't grab a club while lying flat on your face."
    );
    ensure!(
        !unit.airborne() && unit.free_fall().is_none(),
        "You can't grab a club while in the air!"
    );
    ensure!(
        unit.unjam().is_none(),
        "You are too busy unjamming a weapon!"
    );
    let now = world.btech.simulation_time();
    ensure!(
        !unit.fired_this_turn(now),
        "You fired weapons this turn and cannot also pick up a club."
    );
    ensure!(
        unit.limb_recycle().is_empty(),
        "Your limbs are still recovering from another attack"
    );
    for kind in ArmAttack::HAND_WEAPONS
        .into_iter()
        .filter(|kind| kind.needs_hand())
    {
        for arm in [Arm::Left, Arm::Right] {
            ensure!(
                !kind.available(unit, arm.section())?,
                "You cannot grab a club while carrying a {}",
                kind.name()
            );
        }
    }
    let arm = match selection.map(str::to_ascii_lowercase).as_deref() {
        None if usable(unit, Arm::Left)? => Arm::Left,
        None => Arm::Right,
        Some("l" | "left") => Arm::Left,
        Some("r" | "right") => Arm::Right,
        _ => anyhow::bail!("Choose left, right, or - to drop the club"),
    };
    ensure!(
        usable(unit, arm)?,
        "You don't have a free arm with a working hand and shoulder!"
    );
    ensure!(
        unit.carried_club.is_none(),
        "You're already carrying a club."
    );
    let (map, coordinate) = unit_hex(world, id).context("Unit is not on a battlefield")?;
    let record = &world.btech.maps()[&map];
    let tile = record.hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let debris = record.debris_at(coordinate)?;
    let rubble = debris
        .ruin
        .filter(|_| matches!(tile.ground(), Ground::Rubble | Ground::UltraRubble))
        .and_then(girder_target);
    let source = if let Some(kind) = debris.first() {
        Source::Loose(kind)
    } else if tile.is_woods() {
        Source::Tree
    } else if let Some(target) = rubble {
        Source::Rubble(target)
    } else {
        anyhow::bail!(
            "There don't appear to be any trees, limbs or girders within grabbing distance."
        );
    };
    if let Source::Rubble(_) = source {
        ensure!(
            unit.girder_search
                .is_none_or(|time| now.saturating_sub(time) >= super::spotter::TURN_SECONDS),
            "You already searched this rubble this turn."
        );
    }
    let arm_name = arm.section().name().replace('_', " ");
    world.attempt(|world| {
        let (kind, notices) = match source {
            Source::Loose(kind) => {
                let record = world.btech.maps.get_mut(&map).unwrap();
                let index = record.tile_index(coordinate)?;
                record.set_debris(index, debris.without(kind));
                let mut notices = super::broadcast::observer_notices(
                    world,
                    id,
                    &format!("picks up a {} to use as a club!", kind.name()),
                );
                notices.push(Notice {
                    unit: id,
                    text: format!(
                        "You pick up a {} with your {arm_name} to use as a club.",
                        kind.name()
                    ),
                });
                (kind, notices)
            }
            Source::Tree => {
                let mut notices = super::broadcast::observer_notices(
                    world,
                    id,
                    "reaches down and yanks a tree out of the ground!",
                );
                notices.push(Notice {
                    unit: id,
                    text: format!(
                        "You reach down and yank a tree out of the ground with your {arm_name}."
                    ),
                });
                (ClubKind::Tree, notices)
            }
            Source::Rubble(target) => {
                let unit = world.btech.constructed.get_mut(&id).unwrap();
                unit.girder_search = Some(now);
                let roll = unit.dice.two_d6();
                let mut notices = super::broadcast::observer_notices(
                    world,
                    id,
                    "searches the rubble for a girder.",
                );
                let text =
                    format!("You search the rubble for a girder. BTH: {target}, Roll: {roll}");
                if roll < target {
                    notices.push(Notice {
                        unit: id,
                        text: format!("{text}\nYou find nothing you can swing."),
                    });
                    world.btech.validate_action(world)?;
                    return Ok(notices);
                }
                notices.push(Notice {
                    unit: id,
                    text: format!(
                        "{text}\nYou drag a girder out of the rubble with your {arm_name}."
                    ),
                });
                (ClubKind::Girder, notices)
            }
        };
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.carried_club = Some(CarriedClub { arm, kind });
        unit.tie_up_every_section();
        world.btech.validate_action(world)?;
        Ok(notices)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A medium building's rubble yields a girder on 7 or more, a heavy one's on 6 and a
    /// hardened one's on 5; a light building leaves none (Total Warfare p. 146).
    #[test]
    fn girder_searches_follow_the_building_class() {
        assert_eq!(girder_target(ConstructionClass::Light), None);
        assert_eq!(girder_target(ConstructionClass::Medium), Some(7));
        assert_eq!(girder_target(ConstructionClass::Heavy), Some(6));
        assert_eq!(girder_target(ConstructionClass::Hardened), Some(5));
    }

    /// Only an uprooted tree breaks on its first successful blow (Total Warfare p. 146).
    #[test]
    fn only_trees_break() {
        assert!(ClubKind::Tree.breaks_on_hit());
        assert!(!ClubKind::Limb.breaks_on_hit());
        assert!(!ClubKind::Girder.breaks_on_hit());
    }

    /// A pile hands out its limbs before its girders.
    #[test]
    fn piles_give_up_limbs_first() {
        let pile = HexDebris::default()
            .with(ClubKind::Girder)
            .with(ClubKind::Limb)
            .with(ClubKind::Tree);
        assert_eq!(
            pile,
            HexDebris {
                limbs: 1,
                girders: 1,
                ruin: None
            }
        );
        assert_eq!(pile.first(), Some(ClubKind::Limb));
        let pile = pile.without(ClubKind::Limb);
        assert_eq!(pile.first(), Some(ClubKind::Girder));
        assert!(pile.without(ClubKind::Girder).is_empty());
    }
}
