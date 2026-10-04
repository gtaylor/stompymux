//! Cockpit sighting shares target selection and aim, committing only its preparation dice and notices.
use super::*;
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Sighting retains the ordinary aim breakdown for its actual target kind.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum BattleSightAim {
    Unit(BattleAimModifiers),
    Hex(BattleHexAimModifiers),
    Artillery(BattleArtilleryAim),
}

/// A completed sighting consumes dice but never launches a weapon or changes its readiness.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleSightReport {
    pub shooter: ObjectId,
    pub weapon_index: usize,
    pub weapon: BattleWeapon,
    pub target: Option<ObjectId>,
    pub coordinate: Option<HexCoordinate>,
    pub aim: BattleSightAim,
    pub target_number: Option<i32>,
    pub roll: u8,
    pub gatling_roll: Option<u8>,
    pub partial_cover: bool,
}

/// Resolve sighting in the caller's host transaction, including notice publication and rollback.
pub(super) fn action(
    scripts: &Scripts,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    request: super::fire_target::FireTargetRequest<'_>,
) -> Result<BattleSightReport> {
    scripts.atomic(|before| {
        let operator = super::combat_operator::admit_running(before, shooter, pilot)?;
        let shooter = operator.source.unit;
        let mut candidate = before.clone();
        let report = resolve(&mut candidate, config, shooter, pilot, index, request)?;
        candidate.validate_action(config)?;
        *scripts.world.borrow_mut() = candidate;
        let network = match &report.aim {
            BattleSightAim::Unit(aim) => aim.network_range,
            BattleSightAim::Hex(aim) => aim.modifiers.network_range,
            BattleSightAim::Artillery(_) => None,
        };
        if report.target_number.is_some()
            && let Some(source) = network.and_then(|range| range.source)
        {
            super::notify_unit_text(
                scripts,
                operator.source.unit,
                &format!(
                    "Using range data from {}",
                    super::command_network::display_id(&scripts.world.borrow(), shooter, source)?
                ),
            )?;
        }
        let name = report
            .weapon
            .name()
            .split_once('.')
            .expect("catalog namespace")
            .1;
        let destination = if let Some(hex) = report.coordinate {
            format!("({},{})", hex.x, hex.y)
        } else {
            let target = report.target.expect("unit or coordinate target");
            format!(
                "{}{}",
                super::command_network::display_id(&scripts.world.borrow(), shooter, target)?,
                super::aimed_target::suffix(
                    &scripts.world.borrow(),
                    shooter,
                    target,
                    report.weapon
                )?
            )
        };
        let number = report
            .target_number
            .map_or("Out of range.".into(), |number| format!("BTH: {number}"));
        let cover = if report.partial_cover {
            " (Partial cover)"
        } else {
            ""
        };
        super::notify_unit_text(
            scripts,
            operator.source.unit,
            &format!("You aim {name} at {destination} - {number}{cover}"),
        )?;
        Ok(report)
    })
}

/// Native sighting uses ordinary weapon/target grammar without admitting firing or revealing cover.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result: Result<()> = (|| {
        let shooter = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        super::combat_operator::admit_running(&ctx.scripts.world.borrow(), shooter, ctx.player)?;
        let (number, arguments) = input
            .args
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((input.args.trim(), ""));
        let index = number.parse().context("Invalid weapon number")?;
        action(
            ctx.scripts,
            ctx.config,
            shooter,
            ctx.player,
            index,
            super::fire_target::FireTargetRequest::Arguments(arguments),
        )?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Prepare one sighting on a private world candidate, sharing all target-kind arithmetic.
fn resolve(
    world: &mut World,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    request: super::fire_target::FireTargetRequest<'_>,
) -> Result<BattleSightReport> {
    let operator = super::combat_operator::admit_running(world, shooter, pilot)?;
    super::spotter::check_firing_role(world, shooter)?;
    let (mechanics, disabled, mode, mut dice) =
        super::with_unit!(world.btech.unit(shooter).expect("admitted unit"), |unit| {
            (
                unit.weapon_mechanics(index)?,
                unit.weapon_failures.get(&index) == Some(&BattleEquipmentFailure::Disabled),
                unit.fire_mode(index)?,
                unit.dice.clone(),
            )
        });
    let weapon = mechanics.check_sight(disabled)?;
    let requested = request.resolve_for_source(world, operator.source, index)?;
    let target = if weapon.is_artillery() {
        None
    } else {
        Some(super::fire_target::resolve_conventional_for_source(
            world,
            operator.source,
            index,
            requested,
        )?)
    };
    check_busy(world, shooter)?;
    // Sighting still draws preparation intensity, with no supply check or cap.
    let gatling_roll = (mode == BattleFireMode::Gatling).then(|| dice.d6());
    let rules = BattleAimRules::configured(&config.battletech);
    let (target, coordinate, aim, number, distance, partial_cover) = match target {
        None => {
            let prepared =
                super::artillery_firing::prepare(world, config, shooter, pilot, index, requested)?;
            (
                None,
                Some(prepared.coordinate),
                BattleSightAim::Artillery(prepared.aim),
                Some(prepared.aim.target_number).filter(|number| *number <= 900),
                prepared.distance,
                false,
            )
        }
        Some(super::fire_target::ResolvedFireTarget::Hex(hex)) => {
            check_bearing(world, operator.source, index, hex, rules)?;
            let aim = super::hex_aim::modifiers_for_source(
                world,
                operator.source,
                hex,
                index,
                super::unit_gunnery_target(
                    world,
                    shooter,
                    index,
                    config.battletech.extended_gunnery != 0,
                )?,
                rules,
            )?;
            ensure!(aim.visible, "Target hex is not visible");
            let number = aim.subtotal();
            let distance = aim.modifiers.distance;
            (
                None,
                Some(hex),
                BattleSightAim::Hex(aim),
                number,
                distance,
                false,
            )
        }
        Some(super::fire_target::ResolvedFireTarget::Unit {
            unit: target,
            coordinate,
        }) => {
            check_unit_target(world, operator.source, target, index, weapon, rules)?;
            let gunnery = super::unit_gunnery_target(
                world,
                shooter,
                index,
                config.battletech.extended_gunnery != 0,
            )?;
            let aim = super::aim::aim_modifiers_for_source(
                world,
                operator.source,
                target,
                index,
                gunnery,
                rules,
            )?;
            super::aim::ensure_perceived(&aim)?;
            let partial_cover = super::unit_terrain_los(world, shooter, target)?.partial_cover;
            let number = aim.subtotal().filter(|number| {
                if coordinate.is_some() {
                    *number <= 900
                } else {
                    *number < 900
                }
            });
            let distance = aim.distance;
            (
                Some(target),
                coordinate,
                BattleSightAim::Unit(aim),
                number,
                distance,
                partial_cover,
            )
        }
    };
    let roll = super::launch_roll::attack_roll(weapon, distance, &mut dice);
    crate::btech::with_unit_mut!(world.btech.unit_mut(shooter).unwrap(), |unit| {
        unit.dice = dice;
    });
    Ok(BattleSightReport {
        shooter,
        weapon_index: index,
        weapon,
        target,
        coordinate,
        aim,
        target_number: number,
        roll,
        gatling_roll,
        partial_cover,
    })
}

/// Preparation still rejects crew stun and ongoing manual recovery work when sighting.
fn check_busy(world: &World, shooter: ObjectId) -> Result<()> {
    let (stunned, turret, unjam, pods) = if let Some(unit) = world.btech.vehicles().get(&shooter) {
        (
            unit.crew_stunned(),
            !unit.turret_repairs().is_empty(),
            unit.unjam().is_some(),
            unit.pod_removal().is_some(),
        )
    } else {
        let unit = &world.btech.constructed_units()[&shooter];
        (
            unit.stun_remaining() > 0,
            false,
            unit.unjam().is_some(),
            false,
        )
    };
    ensure!(!stunned, "You are too stunned to fire a weapon!");
    ensure!(!turret, "You are too busy unjamming your turret!");
    ensure!(!unjam, "You are too busy unjamming a weapon!");
    ensure!(!pods, "You are too busy removing iNARC pods!");
    Ok(())
}

/// Coordinate geometry is independent of weapon recycling, ammunition and sighting dice.
fn check_bearing(
    world: &World,
    targeting: super::fire_target::TargetSource,
    index: usize,
    hex: HexCoordinate,
    rules: BattleAimRules,
) -> Result<()> {
    let shooter = targeting.unit;
    let facts = super::scanner::scanner_unit(world, shooter).context("Shooter is unavailable")?;
    let position = facts.position.context("Shooter is not placed")?;
    let map = &world.btech.maps()[&position.map];
    map.base_hex(i64::from(hex.x), i64::from(hex.y))?;
    let super::weapon_geometry::WeaponGeometry {
        weapon,
        submerged,
        bears,
    } = super::weapon_geometry::geometry(world, shooter, index, hex.center())?;
    super::weapon_geometry::check_water(weapon, submerged)?;
    ensure!(
        rules.override_weapon_arcs
            || super::spotter::indirect_hex_for_source(world, targeting, index)?.is_some()
            || bears,
        "Target is outside weapon arc"
    );
    Ok(())
}

/// Unit-target permissions retain team safeties, stable stealth locks and indirect-fire geometry.
fn check_unit_target(
    world: &World,
    targeting: super::fire_target::TargetSource,
    target: ObjectId,
    index: usize,
    weapon: BattleWeapon,
    rules: BattleAimRules,
) -> Result<()> {
    let shooter = targeting.unit;
    let source = super::scanner::scanner_unit(world, shooter).context("Shooter is unavailable")?;
    let recipient = super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
    ensure!(!recipient.destroyed, "Unit is destroyed");
    let coolant = weapon == BattleWeapon::CoolantGun;
    ensure!(shooter != target || coolant, "A unit cannot fire on itself");
    super::fire_target::check_target_safety_for_source(world, targeting, target, weapon)?;
    let ammunition = crate::btech::with_unit!(world.btech.unit(shooter).unwrap(), |unit| {
        unit.ammunition_mode(index)?
    });
    if ammunition.munition() == BattleAmmunitionMode::Stinger {
        ensure!(
            super::stinger::target_airborne(world, target),
            "Stinger missiles can only engage airborne targets!"
        );
    }
    super::torpedo::check_target(world, weapon, target)?;
    let indirect = super::spotter::indirect_target_for_source(world, targeting, index)?;
    if indirect.is_some() {
        super::spotter::check_indirect_water(world, shooter, target)?;
    }
    let position = recipient.position.context("Target is not placed")?;
    ensure!(
        source.position.context("Shooter is not placed")?.map == position.map,
        "Units are on different maps"
    );
    let geometry = super::weapon_geometry::geometry(
        world,
        shooter,
        index,
        recipient.point.context("Target is not placed")?,
    )?;
    super::weapon_geometry::check_water(geometry.weapon, geometry.submerged)?;
    ensure!(
        (coolant && shooter == target)
            || indirect.is_some()
            || rules.override_weapon_arcs
            || geometry.bears,
        "Target is outside weapon arc"
    );
    Ok(())
}

/// Typed Lua requests use the same transaction and target parser as the native cockpit command.
pub(crate) fn resolve_action(
    scripts: &Scripts,
    config: &Config,
    shooter: ObjectId,
    pilot: ObjectId,
    index: usize,
    target: BattleFireTarget,
) -> Result<BattleSightReport> {
    action(
        scripts,
        config,
        shooter,
        pilot,
        index,
        super::fire_target::FireTargetRequest::Target(target),
    )
}
