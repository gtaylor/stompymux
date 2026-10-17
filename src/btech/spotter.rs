//! Spotters for LRM indirect fire (Total Warfare p. 111): who may spot, which unit an
//! observer link aims at, whether a shot through it is indirect at all, and the spotter's
//! own contributions to the to-hit number. Mechs and vehicles share every rule here.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, bail, ensure};

/// One turn of the realtime game, in simulation seconds: the window within which a spotter
/// "also attacked this turn" or "charged or made a DFA this turn".
pub const TURN_SECONDS: i64 = 30;

/// A validated spotter and its currently acquired unit target; no dice or derived aim is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpotterTarget {
    pub spotter: ObjectId,
    pub target: ObjectId,
}

impl Mech {
    /// Self-selection declares this unit a spotter; another ID selects a forward observer.
    pub fn spotter(&self) -> Option<ObjectId> {
        self.spotter
    }

    /// A selected observer restricts fire to indirect-capable weapons; spotting itself does
    /// not stop this unit firing.
    pub(super) fn check_spotter_fire(&self, id: ObjectId, index: usize) -> Result<()> {
        check_fire(
            self.spotter,
            id,
            self.weapon_readiness(index)?.weapon,
            self.ammunition_mode(index)?,
        )
    }
}

impl Vehicle {
    /// Self-selection declares this unit a spotter; another ID selects its observer.
    pub fn spotter(&self) -> Option<ObjectId> {
        self.spotter
    }

    /// Vehicle weapons obey the same observer restriction as Mech weapons.
    pub(super) fn check_spotter_fire(&self, id: ObjectId, index: usize) -> Result<()> {
        check_fire(
            self.spotter,
            id,
            self.weapon_readiness(index)?.weapon,
            self.ammunition_mode(index)?,
        )
    }
}

/// With another unit selected as observer, only weapons that can fire through it may fire.
fn check_fire(
    spotter: Option<ObjectId>,
    id: ObjectId,
    weapon: Weapon,
    ammunition: AmmunitionMode,
) -> Result<()> {
    ensure!(
        spotter.is_none_or(|spotter| spotter == id)
            || weapon.supports_indirect_ammunition(ammunition)
            || weapon.is_artillery(),
        "Remove your spotter to fire non-IDF weapons"
    );
    Ok(())
}

/// Read the selected link from either owned construction store.
pub(super) fn selected(world: &World, id: ObjectId) -> Option<ObjectId> {
    world
        .btech
        .vehicles()
        .get(&id)
        .and_then(Vehicle::spotter)
        .or_else(|| {
            world
                .btech
                .constructed_units()
                .get(&id)
                .and_then(Mech::spotter)
        })
}

/// Whether `id` is declared a spotter and some other unit currently fires through it. Its
/// own weapon attacks then take +1 (TW p. 111).
pub(super) fn spotting_for_others(world: &World, id: ObjectId) -> bool {
    selected(world, id) == Some(id)
        && super::scanner::scanner_ids(world)
            .into_iter()
            .any(|other| other != id && selected(world, other) == Some(id))
}

/// Whether the unit launched a weapon within the last turn.
pub(super) fn attacked_this_turn(world: &World, id: ObjectId) -> bool {
    let now = world.btech.simulation_time();
    world
        .btech
        .unit(id)
        .and_then(|unit| unit.last_weapon_fire())
        .is_some_and(|time| now.saturating_sub(time) < TURN_SECONDS)
}

/// Record a charge, ram or death-from-above attack, which bars the unit from spotting for a
/// turn.
pub(super) fn record_physical_assault(world: &mut World, id: ObjectId) {
    let now = world.btech.simulation_time();
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        unit.physical_assault_time = Some(now);
    } else if let Some(vehicle) = world.btech.vehicles.get_mut(&id) {
        vehicle.physical_assault_time = Some(now);
    }
}

/// Why a unit may not spot right now (TW p. 111, p. 141), or `None` when it may: a Haywire
/// pod attached, or a charge, ram or death-from-above attack under way or made within the
/// turn (TW p. 112).
pub(super) fn spotting_prohibited(world: &World, id: ObjectId) -> Option<&'static str> {
    if super::narc::has_beacon(world, id, BeaconKind::Haywire) {
        return Some("cannot spot with an iNarc Haywire pod attached");
    }
    let now = world.btech.simulation_time();
    if let Some(vehicle) = world.btech.vehicles().get(&id) {
        let ramming = vehicle.charge.target.is_some()
            || vehicle
                .physical_assault_time()
                .is_some_and(|time| now.saturating_sub(time) < TURN_SECONDS);
        return ramming.then_some("cannot spot in the turn of a charge or death from above attack");
    }
    let unit = world.btech.constructed_units().get(&id)?;
    let assaulting = unit.charge().target.is_some()
        || unit
            .flight()
            .is_some_and(|flight| flight.dfa_target().is_some())
        || unit
            .physical_assault_time()
            .is_some_and(|time| now.saturating_sub(time) < TURN_SECONDS);
    assaulting.then_some("cannot spot in the turn of a charge or death from above attack")
}

/// Shared perception and movement contributions for both shooters and observer types: a
/// declared spotter's link, or failing that a friendly TAG designation or a Narc pod on the
/// locked target (see [`unspotted_indirect`]).
pub(super) fn indirect_aim(
    world: &World,
    source: super::fire_target::TargetSource,
    target: ObjectId,
    index: usize,
) -> Result<Option<IndirectAim>> {
    let Some(link) = indirect_target_for_source(world, source, index)? else {
        return unspotted_indirect(world, source, target, index);
    };
    ensure!(
        link.target == target,
        "Target differs from the spotter's selected target"
    );
    observer_aim(world, link.spotter).map(Some)
}

/// The spotter's contributions to an indirect shot: its movement, and whether it attacked
/// this turn. TAG guidance is decided by the shot's ammunition, not here.
pub(super) fn observer_aim(world: &World, spotter: ObjectId) -> Result<IndirectAim> {
    let movement = crate::btech::with_unit!(world.btech.unit(spotter).unwrap(), |unit| {
        unit.attacker_movement_modifier()
    });
    Ok(IndirectAim {
        spotter: Some(spotter),
        movement,
        spotter_attacked: attacked_this_turn(world, spotter),
        tag_guided: false,
    })
}

/// Indirect LRM fire at the locked target without a declared spotter, when the firer cannot
/// see it itself. A friendly unit whose TAG designates the target spots it (Total Warfare
/// p. 142): its movement counts, but TAG is used before the attack phase, so its own attacks
/// add nothing. Failing that, Narc-equipped missiles fire indirectly at a target carrying a
/// Narc pod, or an iNarc homing pod, that no enemy ECM covers, with no spotter at all
/// (p. 139). Hex targets and artillery never qualify.
pub(super) fn unspotted_indirect(
    world: &World,
    source: super::fire_target::TargetSource,
    target: ObjectId,
    index: usize,
) -> Result<Option<IndirectAim>> {
    let firer = source.unit;
    let (weapon, ammunition) = installation(world, firer, index)?;
    if !weapon.supports_indirect_ammunition(ammunition)
        || weapon.is_artillery()
        || target == firer
        || !matches!(
            source.selection(world),
            Some(TargetSelection::Unit(lock)) if lock.target == target
        )
        || attacker_sees(world, firer, target)?
    {
        return Ok(None);
    }
    if let Some(tagger) = super::tag::friendly_tagger(world, firer, target)
        && tagger != firer
        && spotting_prohibited(world, tagger).is_none()
    {
        return Ok(Some(IndirectAim {
            spotter_attacked: false,
            ..observer_aim(world, tagger)?
        }));
    }
    let homing = super::narc::has_beacon(world, target, BeaconKind::Narc)
        || super::narc::has_beacon(world, target, BeaconKind::Homing);
    if ammunition.munition() == AmmunitionMode::Narc
        && homing
        && !super::electronic_field(world, target)?.blocks_incoming_guidance()
    {
        return Ok(Some(IndirectAim {
            spotter: None,
            movement: 0,
            spotter_attacked: false,
            tag_guided: false,
        }));
    }
    Ok(None)
}

/// Whether a shot at `target` is indirect: through a declared spotter's link, or without one
/// (see [`unspotted_indirect`]).
pub(super) fn fires_indirectly(
    world: &World,
    source: super::fire_target::TargetSource,
    target: ObjectId,
    index: usize,
) -> Result<bool> {
    Ok(indirect_target_for_source(world, source, index)?.is_some()
        || unspotted_indirect(world, source, target, index)?.is_some())
}

/// Select a friendly observer, declare self spotting, or clear either role.
///
/// An observer must be a declared spotter that is friendly, running, conscious and on the
/// same battlefield; the firer needs no sight of it (TW p. 111 asks for nothing between
/// attacker and spotter). A spotter may go on firing its own weapons.
pub fn select_spotter(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    selected: Option<ObjectId>,
) -> Result<Vec<Notice>> {
    power::controlled(world, id, pilot)?;
    let unit = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    ensure!(unit.power == Power::Running, "Start the unit first");
    let text = match selected {
        None => {
            if self::selected(world, id) == Some(id) {
                "You spot no longer.".into()
            } else {
                "You no longer have a spotter.".into()
            }
        }
        Some(target) if target == id => {
            if let Some(reason) = spotting_prohibited(world, id) {
                bail!("You {reason}.");
            }
            "You are now set as a spotter.".into()
        }
        Some(target) => {
            let observer = super::scanner::scanner_unit(world, target)
                .context("That target does not exist!")?;
            ensure!(
                observer.signature.team == unit.signature.team,
                "That target does not exist!"
            );
            ensure!(
                self::selected(world, target) == Some(target),
                "That unit is not set up as spotter!"
            );
            ensure!(
                observer.power == Power::Running && !observer.destroyed,
                "That spotter is not running!"
            );
            ensure!(
                unit.position.is_some()
                    && unit.position.map(|p| p.map) == observer.position.map(|p| p.map),
                "That spotter is on another battlefield!"
            );
            ensure!(
                !unconscious_crew(world, target),
                "That spotter is unconscious!"
            );
            format!("#{0} set as spotter.", target.0)
        }
    };
    super::artillery_adjustment::spotter_change(world, id, selected);
    crate::btech::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        unit.spotter = selected;
    });
    Ok(vec![Notice { unit: id, text }])
}

/// Whether the unit's pilot is unconscious.
fn unconscious_crew(world: &World, id: ObjectId) -> bool {
    world
        .btech
        .unit(id)
        .and_then(|unit| unit.pilot())
        .is_some_and(|pilot| world.btech.unconscious(pilot))
}

/// Resolve a selected observer's unit target without requiring that the firer see the target.
pub fn spotter_target(world: &World, firer: ObjectId) -> Result<SpotterTarget> {
    let spotter = active_observer(world, firer)?;
    let target = match super::targeting::selection(world, spotter) {
        Some(TargetSelection::Unit(lock)) => lock.target,
        Some(TargetSelection::Hex(lock)) => super::hex_occupant(world, firer, lock.hex)?
            .context("Your spotter's hex is empty; no unit target is available")?,
        None => anyhow::bail!("Your spotter has no target set!"),
    };
    // The spotter must have line of sight to the target itself (Total Warfare p. 111): a
    // contact a teammate relays, or a sensor blip behind a hill, will not do.
    ensure!(
        attacker_sees(world, spotter, target)?,
        "Your spotter does not have a target in LOS!"
    );
    Ok(SpotterTarget { spotter, target })
}

/// Coordinate spotting earns no spotting awards; the shot still takes the observer's terrain
/// modifiers to the unit standing in that hex (Total Warfare p. 111).
pub(super) fn coordinate_target(world: &World, observer: ObjectId) -> bool {
    matches!(
        super::targeting::selection(world, observer),
        Some(TargetSelection::Hex(_))
    )
}

/// Revalidate an established observer link independently of whether its target is a unit or hex.
pub(super) fn active_observer(world: &World, firer: ObjectId) -> Result<ObjectId> {
    let unit =
        super::scanner::scanner_unit(world, firer).context("Firing unit is not constructed")?;
    let spotter = selected(world, firer)
        .filter(|spotter| *spotter != firer)
        .context("No spotter selected")?;
    for id in [firer, spotter] {
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Spotter link is unavailable"
        );
    }
    let observer =
        super::scanner::scanner_unit(world, spotter).context("Spotter is unavailable")?;
    ensure!(
        selected(world, spotter) == Some(spotter),
        "You do not have a spotter!"
    );
    ensure!(
        observer.power == Power::Running && !observer.destroyed,
        "Spotter is unavailable"
    );
    ensure!(
        unit.position.is_some() && unit.position.map(|p| p.map) == observer.position.map(|p| p.map),
        "Spotter is on another battlefield"
    );
    ensure!(
        observer.signature.team == unit.signature.team,
        "Spotter is not friendly"
    );
    ensure!(
        !unconscious_crew(world, spotter),
        "Your spotter is unconscious!"
    );
    if let Some(reason) = spotting_prohibited(world, spotter) {
        bail!("Your spotter {reason}.");
    }
    Ok(spotter)
}

/// The unit target a selected observer supplies, whether or not the shot ends up indirect.
pub(super) fn observed_target_for_source(
    world: &World,
    source: super::fire_target::TargetSource,
    index: usize,
) -> Result<Option<SpotterTarget>> {
    let firer = source.unit;
    ensure!(
        super::scanner::scanner_unit(world, firer).is_some(),
        "Firing unit is not constructed"
    );
    if !uses_observer(world, source, index)? {
        return Ok(None);
    }
    spotter_target(world, firer).map(Some)
}

/// The observer link of a shot that is indirect: the observer supplies the target and the
/// firer has no valid line of sight of its own to it. An attacker that can see its target
/// fires directly (TW p. 111).
pub(super) fn indirect_target_for_source(
    world: &World,
    source: super::fire_target::TargetSource,
    index: usize,
) -> Result<Option<SpotterTarget>> {
    let Some(link) = observed_target_for_source(world, source, index)? else {
        return Ok(None);
    };
    Ok((!attacker_sees(world, source.unit, link.target)?).then_some(link))
}

/// Whether the attacker itself perceives the target along a clear line of sight.
fn attacker_sees(world: &World, firer: ObjectId, target: ObjectId) -> Result<bool> {
    Ok(super::aim::perception_aim(world, firer, target, false)?
        .is_some_and(|perception| perception.channel.is_some() && perception.direct_fire))
}

/// A direct unit lock overrides a selected observer for conventional weapons.
fn uses_observer(
    world: &World,
    source: super::fire_target::TargetSource,
    index: usize,
) -> Result<bool> {
    let firer = source.unit;
    let (weapon, ammunition) = installation(world, firer, index)?;
    Ok(weapon.supports_indirect_ammunition(ammunition)
        && !weapon.is_artillery()
        && selected(world, firer).is_some_and(|spotter| spotter != firer)
        && !matches!(source.selection(world), Some(TargetSelection::Unit(_))))
}

/// The empty coordinate a selected observer supplies, whether or not the shot ends up indirect.
pub(super) fn observed_hex_for_source(
    world: &World,
    source: super::fire_target::TargetSource,
    index: usize,
) -> Result<Option<(ObjectId, HexCoordinate)>> {
    let firer = source.unit;
    if !uses_observer(world, source, index)? {
        return Ok(None);
    }
    let observer = active_observer(world, firer)?;
    let Some(TargetSelection::Hex(lock)) = super::targeting::selection(world, observer) else {
        return Ok(None);
    };
    if super::hex_occupant(world, firer, lock.hex)?.is_some() {
        return Ok(None);
    }
    ensure!(
        super::hex_visible(world, observer, lock.hex)?,
        "That target is not in your spotters line of sight!"
    );
    Ok(Some((observer, lock.hex)))
}

/// The observer link of an indirect shot at an empty coordinate; a firer that sees the hex
/// itself fires at it directly.
pub(super) fn indirect_hex_for_source(
    world: &World,
    source: super::fire_target::TargetSource,
    index: usize,
) -> Result<Option<(ObjectId, HexCoordinate)>> {
    let Some((observer, hex)) = observed_hex_for_source(world, source, index)? else {
        return Ok(None);
    };
    Ok((!super::hex_visible(world, source.unit, hex)?).then_some((observer, hex)))
}

/// Indirect fire cannot cross from a surface shooter into a target entirely under the water; a
/// 'Mech wading in Depth 1 water stands above the surface and can be hit (Total Warfare
/// p. 108).
pub(super) fn check_indirect_water(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
) -> Result<()> {
    let underwater = |id| super::los::unit_underwater(world, id);
    ensure!(
        !underwater(target)? || underwater(shooter)?,
        "You can't fire into water with that weapon from here."
    );
    Ok(())
}

/// Award one point to each eligible crew before indirect-shot damage changes the target.
/// Ordinary skill timing applies independently to the observer and firing unit.
pub(super) fn award_indirect_experience(
    world: &mut World,
    firer: ObjectId,
    link: SpotterTarget,
) -> Result<Vec<DiagnosticMessage>> {
    if coordinate_target(world, link.spotter) {
        return Ok(Vec::new());
    }
    let mut messages = Vec::new();
    for (unit, skill, channel, description) in [
        (
            link.spotter,
            "Gunnery-Spotting",
            TraceTopic::Experience,
            "spotting XP",
        ),
        (
            firer,
            "Gunnery-Artillery",
            TraceTopic::GunneryExperience,
            "1 artillery XP",
        ),
    ] {
        if unit == link.target
            || [unit, link.target].into_iter().any(|id| {
                world.objects.get(&id).is_none_or(|object| {
                    !object.flags.contains(Flag::InCharacter) || object.flags.contains(Flag::Going)
                })
            })
        {
            continue;
        }
        let attacker =
            super::scanner::scanner_unit(world, unit).context("Shooter is unavailable")?;
        let target =
            super::scanner::scanner_unit(world, link.target).context("Target is unavailable")?;
        if target.destroyed || attacker.signature.team == target.signature.team {
            continue;
        }
        let Some(pilot) = super::skills::active_pilot(world, unit)? else {
            continue;
        };
        if world.objects[&pilot].flags.contains(Flag::Going) {
            continue;
        }
        let award =
            award_skill_experience(world, pilot, skill, 1, crate::clock::wall_time(), false)?;
        if award.accepted {
            messages.push(DiagnosticMessage::new(
                channel,
                format!("{} gained {description}", world.objects[&pilot].name),
            ));
        }
    }
    Ok(messages)
}

/// Read the chosen installation and ammunition family across supported chassis.
pub(super) fn installation(
    world: &World,
    id: ObjectId,
    index: usize,
) -> Result<(Weapon, AmmunitionMode)> {
    let unit = world.btech.unit(id).context("Unit is not constructed")?;
    Ok((
        unit.weapon_readiness(index)?.weapon,
        unit.ammunition_mode(index)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rockets can use observer coordination without gaining switchable hotload or semi-guided modes.
    #[test]
    fn spotter_indirect_profiles_do_not_enable_rocket_ammunition_modes() {
        for weapon in [Weapon::Rocket10, Weapon::Rocket15, Weapon::Rocket20] {
            assert!(weapon.supports_indirect_fire());
            assert!(!weapon.supports_hotload());
            assert!(!weapon.supports_semiguided());
        }
        assert!(!Weapon::MediumLaser.supports_indirect_fire());
        assert!(Weapon::Lrm5.supports_indirect_fire());
        assert!(Weapon::Lrm5.supports_semiguided());
    }

    /// A declared spotter keeps every weapon; a selected observer restricts fire to weapons
    /// that can use it.
    #[test]
    fn observer_selection_restricts_fire_but_spotting_does_not() {
        let me = ObjectId(7);
        let other = ObjectId(8);
        for (weapon, ammunition) in [
            (Weapon::MediumLaser, AmmunitionMode::Normal),
            (Weapon::Lrm5, AmmunitionMode::Normal),
        ] {
            assert!(check_fire(None, me, weapon, ammunition).is_ok());
            assert!(check_fire(Some(me), me, weapon, ammunition).is_ok());
            assert_eq!(
                check_fire(Some(other), me, weapon, ammunition).is_ok(),
                weapon.supports_indirect_fire()
            );
        }
    }
}
