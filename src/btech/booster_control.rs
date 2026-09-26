//! Shared booster controls and committed overload checks with distinct mechanical failures.
use super::{BattleNotice, BattlePower};
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Durable overload history and next check; hardware failure survives shutdown.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleBoosterState {
    /// Pilot-selected operating mode.
    pub enabled: bool,
    /// Overload checks made, less completed recovery intervals.
    pub counter: u8,
    /// Committed seconds until the next overload or recovery event.
    pub remaining: u8,
    /// Permanent failure independent of still-installed critical slots.
    pub failed: bool,
}

impl BattleBoosterState {
    /// Reject impossible timer or overload combinations on load.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            self.counter <= 6
                && self.remaining <= 60
                && (!self.enabled || (self.remaining > 0 && !self.failed && self.counter < 6)),
            "Invalid booster state"
        );
        Ok(())
    }
    /// Clear operating history while retaining failed hardware.
    pub(super) fn shutdown(&mut self) {
        self.enabled = false;
        self.counter = 0;
        self.remaining = 0;
    }
}

impl super::BattleUnit {
    /// Owned activation and overload state; hardware inspection remains separate.
    pub fn masc(&self) -> BattleBoosterState {
        self.masc
    }

    /// A working powered booster contributes to the current movement ceiling.
    pub fn masc_active(&self) -> bool {
        self.masc.enabled
            && self.power() == BattlePower::Running
            && self.masc_operational().unwrap_or(false)
    }
}

/// Toggle MASC and proportionally adjust the signed desired throttle.
fn toggle(world: &mut World, id: ObjectId, pilot: ObjectId, kind: Booster) -> Result<BattleNotice> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == BattlePower::Running, "Start the unit first");
    ensure!(
        kind.operational(unit)?,
        "Your toy ain't prepared for what you're askin' it!"
    );
    ensure!(unit.motion().is_some(), "Unit is not placed");
    let enabled = !kind.state(unit).enabled;
    ensure!(
        !enabled || unit.movement_maximum_speed() >= 10.75,
        "{}",
        if kind == Booster::Masc {
            "You can't move. How is MASC going to work?"
        } else {
            "How much can you Supercharge if you can't move?"
        }
    );
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    kind.state_mut(unit).enabled = enabled;
    kind.state_mut(unit).remaining = if enabled { 1 } else { 60 };
    unit.supercharger_scheduled_last = kind == Booster::Supercharger;
    unit.motion.as_mut().unwrap().desired_speed *= if enabled { 4.0 / 3.0 } else { 3.0 / 4.0 };
    Ok(BattleNotice {
        unit: id,
        text: format!(
            "{} has been turned {}.",
            kind.name(),
            if enabled { "on" } else { "off" }
        ),
    })
}

/// Cockpit activation shares notification rollback with other Lua/native controls.
fn control(
    scripts: &Scripts,
    id: ObjectId,
    pilot: ObjectId,
    kind: Booster,
) -> Result<BattleNotice> {
    scripts.atomic(|_| {
        let notice = toggle(&mut scripts.world_mut(), id, pilot, kind)?;
        super::notify_unit(scripts, notice.clone())?;
        Ok(notice)
    })
}

/// Advance overload or recovery and publish all falls inside a single host transaction.
pub fn advance_boosters_action(scripts: &Scripts, config: &Config) -> Result<Vec<BattleNotice>> {
    let ids: Vec<_> = scripts
        .world()
        .btech
        .constructed_units()
        .iter()
        .flat_map(|(&id, unit)| {
            let kinds = if unit.supercharger_scheduled_last {
                [Booster::Masc, Booster::Supercharger]
            } else {
                [Booster::Supercharger, Booster::Masc]
            };
            kinds
                .into_iter()
                .filter(move |kind| kind.state(unit).remaining > 0)
                .map(move |kind| (id, kind))
        })
        .collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    scripts.atomic(|before| {
        let mut notices = Vec::new();
        let mut private = Vec::new();
        let mut falls = Vec::new();
        for (id, kind) in ids {
            let mut world = scripts.world_mut();
            let available = world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(crate::Flag::Going));
            let unit = &world.btech.constructed_units()[&id];
            if !available || unit.power() != BattlePower::Running || unit.is_destroyed() {
                kind.state_mut(world.btech.constructed.get_mut(&id).unwrap())
                    .shutdown();
                continue;
            }
            if kind.state(unit).enabled && !kind.operational(unit)? {
                let unit = world.btech.constructed.get_mut(&id).unwrap();
                kind.state_mut(unit).enabled = false;
                kind.state_mut(unit).remaining = 60;
                unit.supercharger_scheduled_last = kind == Booster::Supercharger;
            }
            let pilot = world.btech.constructed_units()[&id].pilot();
            let wizard = pilot
                .and_then(|pilot| world.objects.get(&pilot))
                .is_some_and(|object| object.flags.contains(crate::Flag::Wizard));
            let toughness = pilot
                .and_then(|pilot| world.btech.character_values().get(&pilot))
                .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            kind.state(unit).validate()?;
            if kind.state(unit).remaining == 0 {
                continue;
            }
            kind.state_mut(unit).remaining -= 1;
            if kind.state(unit).remaining > 0 {
                continue;
            }
            if !kind.state(unit).enabled {
                kind.state_mut(unit).counter = kind.state(unit).counter.saturating_sub(1);
                kind.state_mut(unit).remaining = if kind.state(unit).counter > 0 { 60 } else { 0 };
                unit.supercharger_scheduled_last = kind == Booster::Supercharger;
                continue;
            }
            let needed = 2 * (1 + kind.state(unit).counter);
            kind.state_mut(unit).counter += 1;
            let mut roll = unit.dice.generic_roll();
            if kind.other_active(unit) {
                roll -= 1;
            }
            if wizard && needed < 10 {
                roll = needed + unit.dice.die(u16::from(12 - needed))? as u8;
            }
            notices.push(BattleNotice {
                unit: id,
                text: format!(
                    "{}: BTH {}{}, Roll: {roll}",
                    kind.name(),
                    needed + 1,
                    if kind == Booster::Masc { "+" } else { "" }
                ),
            });
            if roll > needed {
                kind.state_mut(unit).remaining = 60;
                unit.supercharger_scheduled_last = kind == Booster::Supercharger;
                continue;
            }
            kind.state_mut(unit).enabled = false;
            kind.state_mut(unit).failed = true;
            if kind == Booster::Supercharger {
                fail_supercharger(&mut world, id, &mut notices)?;
                continue;
            }
            let speed = unit.motion().map_or(0.0, |motion| motion.speed);
            let falling = speed.abs() > 10.75;
            notices.push(BattleNotice {
                unit: id,
                text: if falling {
                    "Your leg actuators freeze suddenly, and you fall!"
                } else {
                    "Your leg actuators freeze suddenly!"
                }
                .into(),
            });
            if falling || speed > 0.0 {
                notices.extend(super::broadcast::observer_notices(
                    &world,
                    id,
                    if falling {
                        "stops and falls in mid-step!"
                    } else {
                        "stops suddenly!"
                    },
                ));
            }
            let settings = &config.battletech;
            let rules = super::BattleFallRules {
                vehicle_impact: crate::BattleVehicleImpactRules::configured(settings, false),
                stacking: super::BattleStackingRules {
                    mode: settings.stacking,
                    damage_percent: settings.stackdamage,
                    hit_arcs: settings.hit_arcs,
                },
                stagger: super::BattleStaggerMode::from_setting(settings.newstagger),
                hit: super::BattleHitRules {
                    inferno_penalty: settings.inferno_penalty != 0,
                    exile_stun_mode: settings.exile_stun_code.clamp(0, 2) as u8,
                },
                extended_piloting: settings.extended_piloting != 0,
                toughness,
            };
            let fall = if falling {
                Some(
                    if world.objects[&id].flags.contains(crate::Flag::InCharacter) {
                        super::fall::resolve_character_fall(&mut world, id, 1, rules)?
                    } else {
                        super::resolve_fall(&mut world, id, 1, rules)?
                    },
                )
            } else {
                None
            };
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            unit.damage_masc_hips()?;
            unit.reconcile_damage();
            drop(world);
            if let Some(fall) = fall {
                fall.append_notices(id, &mut notices, &mut private);
                falls.push(fall);
            }
        }
        super::piloting::publish_ordered_notices(scripts, &notices, &private)?;
        for fall in &falls {
            super::evacuation::publish_fall_consequences(scripts, config, fall)?;
        }
        super::evacuation::publish_new_casualties(scripts, config, &before)?;
        scripts.world().validate_action(config)?;
        Ok(notices)
    })
}

/// Native toggle resolves the invoking player's cockpit.
fn command_for(ctx: &crate::CommandContext<'_>, kind: Booster) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        control(ctx.scripts, id, ctx.player, kind)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Toggle MASC through the pure world control.
pub fn toggle_masc(world: &mut World, id: ObjectId, pilot: ObjectId) -> Result<BattleNotice> {
    toggle(world, id, pilot, Booster::Masc)
}
/// Toggle a supercharger through the pure world control.
pub fn toggle_supercharger(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<BattleNotice> {
    toggle(world, id, pilot, Booster::Supercharger)
}
/// Toggle and publish MASC atomically.
pub fn masc(scripts: &Scripts, id: ObjectId, pilot: ObjectId) -> Result<BattleNotice> {
    control(scripts, id, pilot, Booster::Masc)
}
/// Toggle and publish the supercharger atomically.
pub fn supercharger(scripts: &Scripts, id: ObjectId, pilot: ObjectId) -> Result<BattleNotice> {
    control(scripts, id, pilot, Booster::Supercharger)
}
/// Native MASC control.
pub(crate) fn masc_command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, Booster::Masc)
}
/// Native supercharger control.
pub(crate) fn supercharger_command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, Booster::Supercharger)
}

/// The two independently scheduled booster devices.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Booster {
    Masc,
    Supercharger,
}
impl Booster {
    fn name(self) -> &'static str {
        match self {
            Self::Masc => "MASC",
            Self::Supercharger => "Supercharger",
        }
    }
    fn state(self, unit: &super::BattleUnit) -> &BattleBoosterState {
        match self {
            Self::Masc => &unit.masc,
            Self::Supercharger => &unit.supercharger,
        }
    }
    fn state_mut(self, unit: &mut super::BattleUnit) -> &mut BattleBoosterState {
        match self {
            Self::Masc => &mut unit.masc,
            Self::Supercharger => &mut unit.supercharger,
        }
    }
    fn operational(self, unit: &super::BattleUnit) -> Result<bool> {
        match self {
            Self::Masc => unit.masc_operational(),
            Self::Supercharger => Ok(unit.supercharger_operational()),
        }
    }
    fn other_active(self, unit: &super::BattleUnit) -> bool {
        match self {
            Self::Masc => unit.supercharger_active(),
            Self::Supercharger => unit.masc_active(),
        }
    }
}

/// Destroy the compressor and one to four surviving center-torso engine slots in order.
fn fail_supercharger(
    world: &mut World,
    id: ObjectId,
    notices: &mut Vec<BattleNotice>,
) -> Result<()> {
    use super::{BattleSection, BattleSystem};
    notices.push(BattleNotice {
        unit: id,
        text: "Your supercharger overloads and explodes!".into(),
    });
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    let loadout = unit.loadout()?;
    for part in loadout.systems.iter().filter(|part| {
        part.location.section == BattleSection::CenterTorso
            && part.system == BattleSystem::Supercharger
    }) {
        unit.destroy_critical(part.location)?;
    }
    let count = usize::from(unit.dice.die(4)?);
    let engines: Vec<_> = loadout
        .systems
        .iter()
        .filter(|part| {
            part.location.section == BattleSection::CenterTorso
                && part.system == BattleSystem::Engine
                && !unit.critical_destroyed(part.location)
        })
        .take(count)
        .map(|part| part.location)
        .collect();
    for location in engines {
        let unit = &world.btech.constructed_units()[&id];
        let hits = unit.system_hits(BattleSystem::Engine);
        if !unit.is_destroyed() && unit.power() == BattlePower::Running {
            notices.extend(super::broadcast::observer_notices(
                world,
                id,
                "'s center torso spews black smoke!",
            ));
        }
        world
            .btech
            .constructed
            .get_mut(&id)
            .unwrap()
            .destroy_critical(location)?;
        if hits < 2 {
            notices.push(BattleNotice {
                unit: id,
                text: "Your engine shielding takes a hit!  It's getting hotter in here!!".into(),
            });
        } else if hits < 3 {
            notices.push(BattleNotice {
                unit: id,
                text: "Your engine is destroyed!!".into(),
            });
        }
    }
    Ok(())
}
