//! Durable ammunition dumping, shared by cockpit controls and committed simulation steps.
use super::{BattleNotice, BattlePower, BattleSection, BattleUnit, BattleWeapon};
use crate::{ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Bins selected by physical location or weapon family; ammunition modes remain independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleDumpSelection {
    All,
    Weapon(BattleWeapon),
    Section(BattleSection),
    Slot(super::CriticalLocation),
}

/// Saved cadence for an active dumping attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleDump {
    pub selection: BattleDumpSelection,
    /// Committed seconds since this attempt began, for rate scheduling.
    pub phase: u64,
}

impl BattleUnit {
    /// Active ammunition ejection, including its durable cadence.
    pub fn dumping(&self) -> Option<BattleDump> {
        self.dumping
    }
}

impl BattleDumpSelection {
    pub(super) fn matches(self, bin: &super::AmmunitionBin) -> bool {
        match self {
            Self::All => true,
            Self::Weapon(weapon) => bin.weapon == weapon,
            Self::Section(section) => bin.location.section == section,
            Self::Slot(location) => bin.location == location,
        }
    }

    /// Completion wording reflects the requested granularity.
    fn completion(self) -> String {
        match self {
            Self::All => "All ammunition dumped.".into(),
            Self::Weapon(weapon) => format!("Ammunition for {} dumped!", weapon.name()),
            Self::Section(section) => format!(
                "All ammunition in {} dumped.",
                section.name().replace('_', " ")
            ),
            Self::Slot(location) => format!(
                "Ammunition in {} crit {} dumped!",
                location.section.name().replace('_', " "),
                location.slot + 1
            ),
        }
    }

    fn description(self) -> String {
        match self {
            Self::All => "all ammunition".into(),
            Self::Weapon(weapon) => format!("{} ammunition", weapon.name()),
            Self::Section(section) => format!("ammunition in {}", section.name().replace('_', " ")),
            Self::Slot(location) => format!(
                "ammunition in {} crit {}",
                location.section.name().replace('_', " "),
                location.slot + 1
            ),
        }
    }
}

/// Check persisted selections without requiring that any selected ammunition remains usable.
pub(super) fn validate(unit: &BattleUnit) -> Result<()> {
    let Some(dump) = unit.dumping else {
        return Ok(());
    };
    ensure!(dump.phase < u64::MAX, "Invalid ammunition dump cadence");
    let loadout = unit.loadout()?;
    ensure!(
        loadout
            .ammunition
            .iter()
            .any(|bin| dump.selection.matches(bin)),
        "Invalid ammunition dump selection"
    );
    Ok(())
}

/// Resolve user-facing selectors without storing command strings in simulation state.
fn selection(unit: &BattleUnit, words: &[&str]) -> Result<BattleDumpSelection> {
    ensure!(
        !words.is_empty() && words.len() <= 2,
        "Specify all, stop, a weapon number, or a section and optional slot"
    );
    if words[0].eq_ignore_ascii_case("all") {
        ensure!(words.len() == 1, "All takes no slot");
        return Ok(BattleDumpSelection::All);
    }
    if let Ok(index) = words[0].parse::<usize>() {
        ensure!(words.len() == 1, "Weapon number takes no slot");
        let loadout = unit.loadout()?;
        let mount = loadout
            .weapons
            .get(index)
            .context("Invalid weapon number!")?;
        ensure!(
            mount.weapon.profile().ammunition_per_ton > 0,
            "That weapon doesn't use ammunition!"
        );
        return Ok(BattleDumpSelection::Weapon(mount.weapon));
    }
    let section = unit.chassis().parse_location(words[0])?;
    if words.len() == 1 {
        return Ok(BattleDumpSelection::Section(section));
    }
    let slot = words[1].parse::<u8>().context("Invalid ammunition slot!")?;
    ensure!(
        (1..=unit.chassis().critical_slots(section)).contains(&slot),
        "Invalid ammunition slot!"
    );
    Ok(BattleDumpSelection::Slot(super::CriticalLocation {
        section,
        slot: slot - 1,
    }))
}

/// Start, replace with all, or stop an attempt. No ammunition leaves until the next step.
pub fn begin_dump(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    argument: &str,
) -> Result<Vec<BattleNotice>> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == BattlePower::Running, "Start the unit first");
    ensure!(!unit.airborne(), "You can't dump ammo while jumping!");
    ensure!(
        unit.motion().is_some_and(
            |motion| motion.desired_speed <= unit.movement_maximum_speed() * 2.0 / 3.0 + 0.1
        ),
        "You can't dump ammo while running!"
    );
    let words: Vec<_> = argument.split_whitespace().collect();
    let (dump, text, observer) = if words.len() == 1 && words[0].eq_ignore_ascii_case("stop") {
        ensure!(unit.dumping.is_some(), "You aren't dumping anything!");
        (
            None,
            "Ammo dumping halted.".into(),
            "no longer has ammo dumping from hatches on its back.",
        )
    } else {
        let selected = selection(unit, &words)?;
        ensure!(
            unit.dumping.is_none()
                || (selected == BattleDumpSelection::All
                    && unit.dumping.unwrap().selection != selected),
            "You're already dumping some ammo!"
        );
        ensure!(
            unit.loadout()?
                .ammunition
                .iter()
                .enumerate()
                .any(|(index, bin)| selected.matches(bin)
                    && unit.ammunition[index] > 0
                    && !unit.critical_unavailable(bin.location)),
            "You have no ammo to dump!"
        );
        (
            Some(BattleDump {
                selection: selected,
                phase: 0,
            }),
            match selected {
                BattleDumpSelection::Weapon(_) => {
                    format!("Starting dumping {}..", selected.description())
                }
                _ => format!("Starting dumping of {}..", selected.description()),
            },
            "starts dumping ammo from hatches on its back.",
        )
    };
    world.btech.constructed.get_mut(&id).unwrap().dumping = dump;
    let mut notices = vec![BattleNotice { unit: id, text }];
    notices.extend(super::broadcast::observer_notices(world, id, observer));
    Ok(notices)
}

/// Advance all attempts atomically; unavailable bins never eject ammunition.
pub fn advance_dumping(world: &mut World) -> Result<Vec<BattleNotice>> {
    world.attempt(|world| {
        let ids: Vec<_> = world
            .btech
            .constructed_units()
            .iter()
            .filter_map(|(&id, unit)| unit.dumping.is_some().then_some(id))
            .collect();
        let mut notices = Vec::new();
        for id in ids {
            let available = world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(crate::Flag::Going));
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            validate(unit)?;
            let mut dump = unit.dumping.unwrap();
            if !available || unit.power() != BattlePower::Running || unit.is_destroyed() {
                unit.dumping = None;
                continue;
            }
            dump.phase = dump
                .phase
                .checked_add(1)
                .context("Ammunition dump clock overflow")?;
            let mut remaining = false;
            for (index, bin) in unit.loadout()?.ammunition.iter().enumerate() {
                if !dump.selection.matches(bin)
                    || unit.critical_unavailable(bin.location)
                    || unit.ammunition[index] == 0
                {
                    continue;
                }
                let capacity = bin
                    .weapon
                    .profile_for_ammunition(bin.mode)
                    .ammunition_per_ton;
                let amount = if capacity >= 30 {
                    u16::from(capacity / 30)
                } else {
                    u16::from(dump.phase.is_multiple_of(u64::from(30 / capacity)))
                };
                let amount = amount.min(unit.ammunition[index]);
                if let Some(text) =
                    super::combat_warnings::dumping_message(unit, bin.weapon, amount)
                {
                    notices.push(BattleNotice { unit: id, text });
                }
                unit.ammunition[index] -= amount;
                unit.live_mass.invalidate();
                remaining |= unit.ammunition[index] > 0;
            }
            unit.dumping = remaining.then_some(dump);
            if !remaining {
                notices.push(BattleNotice {
                    unit: id,
                    text: dump.selection.completion(),
                });
                notices.extend(super::broadcast::observer_notices(
                    world,
                    id,
                    "no longer has ammo dumping from hatches on its back.",
                ));
            }
        }
        Ok(notices)
    })
}

/// Publish a cockpit action with state and notification rollback on failure.
pub fn dump(
    scripts: &Scripts,
    id: ObjectId,
    pilot: ObjectId,
    argument: &str,
) -> Result<Vec<BattleNotice>> {
    scripts.atomic(|_| {
        let notices = begin_dump(&mut scripts.world_mut(), id, pilot, argument)?;
        for notice in &notices {
            super::notify_unit(scripts, notice.clone())?;
        }
        Ok(notices)
    })
}

/// Native adapter resolves the player's current cockpit.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        dump(ctx.scripts, id, ctx.player, &input.args)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// A single dumped salvo ignited against rear armor during a weapon hit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleDumpIgnition {
    pub bin_index: usize,
    pub weapon: BattleWeapon,
    pub section: BattleSection,
    pub damage: u16,
}

/// Select an available round from the active operation using the unit's saved combat dice.
pub(super) fn ignition(
    unit: &mut BattleUnit,
    section: BattleSection,
) -> Result<Option<BattleDumpIgnition>> {
    let Some(dump) = unit.dumping else {
        return Ok(None);
    };
    let loadout = unit.loadout()?;
    let mut bins: Vec<_> = loadout
        .ammunition
        .iter()
        .enumerate()
        .filter(|(index, bin)| {
            dump.selection.matches(bin)
                && unit.ammunition[*index] > 0
                && !unit.critical_unavailable(bin.location)
        })
        .collect();
    // Whole-unit and weapon requests traverse sections and slots from the rear of storage order.
    if matches!(
        dump.selection,
        BattleDumpSelection::All | BattleDumpSelection::Weapon(_)
    ) {
        bins.sort_by_key(|(_, bin)| std::cmp::Reverse(bin.location));
    }
    if bins.is_empty() {
        return Ok(None);
    }
    let index = usize::from(unit.dice.die(u16::try_from(bins.len())?)? - 1);
    let (bin_index, bin) = bins[index];
    let profile = bin.weapon.profile_for_ammunition(bin.mode);
    let damage = u16::from(profile.damage) * u16::from(profile.missiles.max(1));
    if damage == 0 {
        return Ok(None);
    }
    Ok(Some(BattleDumpIgnition {
        bin_index,
        weapon: bin.weapon,
        section,
        damage,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignition_selects_only_live_requested_bins_without_rolling_for_empty_requests() {
        let mut unit = BattleUnit::from_template(
            super::super::BattleTemplate::parse(
                "AS7-D",
                include_str!("../../tests/fixtures/btech/mechs/AS7-D.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        let loadout = unit.loadout().unwrap();
        let (index, bin) = loadout
            .ammunition
            .iter()
            .enumerate()
            .find(|(_, bin)| bin.weapon == BattleWeapon::Srm6)
            .unwrap();
        for selection in [
            BattleDumpSelection::Weapon(bin.weapon),
            BattleDumpSelection::Section(bin.location.section),
            BattleDumpSelection::Slot(bin.location),
        ] {
            unit.dumping = Some(BattleDump {
                selection,
                phase: 0,
            });
            for _ in 0..20 {
                let hit = ignition(&mut unit, BattleSection::CenterTorso)
                    .unwrap()
                    .unwrap();
                assert!(selection.matches(&loadout.ammunition[hit.bin_index]));
                assert!(hit.damage > 0);
            }
        }
        unit.dumping = Some(BattleDump {
            selection: BattleDumpSelection::Slot(bin.location),
            phase: 0,
        });
        let remaining = unit.ammunition[index];
        unit.ammunition[index] = 0;
        let dice = unit.dice.clone();
        assert!(
            ignition(&mut unit, BattleSection::CenterTorso)
                .unwrap()
                .is_none()
        );
        assert_eq!(unit.dice, dice);
        unit.ammunition[index] = remaining;
        unit.lost_criticals.insert(bin.location);
        assert!(
            ignition(&mut unit, BattleSection::CenterTorso)
                .unwrap()
                .is_none()
        );
        assert_eq!(unit.dice, dice);
    }
}
