//! Shared fixed-column weapon and ammunition display; combat diagnostics remain authoritative.
use crate::btech::{
    self,
    firing::BattleWeaponInspection,
    status_export::{Ammunition, group_ammunition, mode_letter},
    *,
};
use crate::{ObjectId, World, text};
use anyhow::Result;

/// Render both chassis families using the same condition, mode and supply columns.
pub(super) fn render(world: &World, id: ObjectId) -> Result<String> {
    let diagnostics = btech::weapon_reports::weapon_diagnostics(world, id)?;
    let mut lines = super::technology::lines(world, id)?;
    let (rows, bins, ams) = if let Some(u) = world.btech.constructed_units().get(&id) {
        let quad = u.chassis() == BattleMechChassis::Quad;
        let mut limbs = Vec::new();
        for (section, label) in [
            (BattleSection::LeftArm, if quad { "FLLEG" } else { "LARM" }),
            (BattleSection::RightArm, if quad { "FRLEG" } else { "RARM" }),
            (BattleSection::LeftLeg, if quad { "RLLEG" } else { "LLEG" }),
            (BattleSection::RightLeg, if quad { "RRLEG" } else { "RLEG" }),
        ] {
            let value = if u.sections()[&section].internal == 0 {
                "[fg=black bold]*****[reset]".into()
            } else if let Some(seconds) = u.limb_recycle().get(&section) {
                format!("{:<5}", seconds.div_ceil(2))
            } else {
                "[fg=green]Ready[reset]".into()
            };
            limbs.push(format!("{label}: {value}"));
        }
        limbs.extend(physical_readiness(u)?);
        lines.push(format!("{} ", limbs.join(" ")));
        if u.facing().arms_flipped {
            lines.push("*** Mech arms are flipped into the rear arc ***".into());
        }
        let rows = btech::firing::weapon_states(world, id)?
            .into_iter()
            .map(|r| {
                let location = u.chassis().section_name(r.section).replace('_', " ");
                (normalize(r), location)
            })
            .collect::<Vec<_>>();
        let bins = u
            .loadout()?
            .ammunition
            .iter()
            .enumerate()
            .map(|(i, b)| Ammunition {
                weapon: b.weapon,
                mode: b.mode,
                rounds: if u.critical_unavailable(b.location) {
                    0
                } else {
                    u.ammunition()[i]
                },
                capacity: b.capacity,
            })
            .collect::<Vec<_>>();
        (rows, bins, u.ams_enabled())
    } else {
        let u = &world.btech.vehicles()[&id];
        let rows = btech::firing::vehicle_weapon_states(world, id)?
            .into_iter()
            .map(|r| {
                let location = r.section.name().replace('_', " ");
                (normalize(r), location)
            })
            .collect::<Vec<_>>();
        let bins = u
            .loadout()?
            .ammunition
            .iter()
            .enumerate()
            .map(|(i, b)| Ammunition {
                weapon: b.weapon,
                mode: b.mode,
                rounds: if u.critical_unavailable(b.location) {
                    0
                } else {
                    u.ammunition()[i]
                },
                capacity: b.capacity,
            })
            .collect::<Vec<_>>();
        (rows, bins, u.ams_enabled())
    };
    let ammo = group_ammunition(bins);
    lines.push(
        "==================WEAPON SYSTEMS===========================AMMUNITION========".into(),
    );
    lines.push(
        "------ Weapon --------- [##] Location ---- Status ||--- Ammo Type ---- Rounds".into(),
    );
    for (i, (row, section)) in rows.iter().enumerate() {
        let weapon = row.readiness.weapon;
        let name = weapon.name().split_once('.').unwrap().1;
        let name = if row.one_shot {
            format!("OS {name}")
        } else {
            name.into()
        };
        let linked = if let Some(u) = world.btech.constructed_units().get(&id) {
            u.loadout()?.weapons[i].on_targeting_computer
        } else {
            world.btech.vehicles()[&id].loadout()?.weapons[i].on_targeting_computer
        };
        let computer =
            linked && super::technology::device(world, id, BattleSystem::TargetingComputer)?.1;
        let modes = if weapon.is_ams() {
            if ams { "  ON ".into() } else { " OFF ".into() }
        } else {
            format!(
                "{}{}{}{}{}",
                if row.rear_mount { 'R' } else { ' ' },
                if row.readiness.spent {
                    '-'
                } else if row.one_shot {
                    'O'
                } else {
                    ' '
                },
                mode_letter(row.ammunition_mode, weapon),
                fire_letter(row.fire_mode),
                if computer { 'T' } else { ' ' }
            )
        };
        let condition = diagnostics[i].condition;
        let status = match condition {
            BattleEquipmentCondition::Destroyed => "[fg=black bold]*****[reset]  ".into(),
            BattleEquipmentCondition::Disabled => "[fg=red]DISABLE[reset]".into(),
            BattleEquipmentCondition::Jammed => "[fg=red]JAMMED[reset] ".into(),
            BattleEquipmentCondition::Shorted => "[fg=red]SHORTED[reset]".into(),
            BattleEquipmentCondition::Broken => "[fg=red]DUD[reset]    ".into(),
            BattleEquipmentCondition::Empty => " [fg=red]EMPTY[reset] ".into(),
            BattleEquipmentCondition::AmmoJam => "[fg=red]AMMOJAM[reset]".into(),
            _ if row.readiness.spent => "[fg=black bold]Empty[reset]  ".into(),
            _ if row.readiness.recycle_remaining > 0 => {
                format!(" {:2}    ", row.readiness.recycle_remaining.div_ceil(2))
            }
            BattleEquipmentCondition::Damaged => "[fg=red]DAMAGED[reset]".into(),
            _ => "[fg=green]Ready[reset]  ".into(),
        };
        let supply = ammo.get(i).map_or_else(|| "   ".into(), ammunition_column);
        lines.push(format!(
            " {:16.16} {modes} [{:2}] {:14.14}{status}|| {supply}",
            text::escape(&name),
            row.index,
            section
        ));
    }
    for bin in ammo.iter().skip(rows.len()) {
        lines.push(format!(
            "                                                  || {}",
            ammunition_column(bin)
        ));
    }
    Ok(lines.join("\r\n"))
}

/// The cockpit lists installed physical weapons even when their limb is unusable.
/// Availability here describes the limb, independently of attack admission and part damage.
fn physical_readiness(unit: &BattleUnit) -> Result<Vec<String>> {
    let loadout = unit.loadout()?;
    let mut entries = Vec::new();
    for (system, name) in [
        (BattleSystem::Axe, "Axe"),
        (BattleSystem::Sword, "Sword"),
        (BattleSystem::Claw, "Claw"),
        (BattleSystem::Mace, "Mace"),
        (BattleSystem::DualSaw, "Saw"),
    ] {
        let minimum = if system == BattleSystem::DualSaw {
            7
        } else {
            unit.definition().tons / 15
        };
        for (section, arm) in [
            (BattleSection::LeftArm, "LA"),
            (BattleSection::RightArm, "RA"),
        ] {
            let count = loadout
                .systems
                .iter()
                .filter(|part| part.system == system && part.location.section == section)
                .count();
            if count < usize::from(minimum) {
                continue;
            }
            let shoulder =
                btech::physical::actuator(unit, section, 0, BattleSystem::ShoulderOrHip)?;
            let hand =
                btech::physical::actuator(unit, section, 3, BattleSystem::HandOrFootActuator)?;
            let usable = unit.sections()[&section].internal > 0
                && (system == BattleSystem::Claw || shoulder)
                && (matches!(system, BattleSystem::Claw | BattleSystem::DualSaw) || hand);
            let status = if !usable {
                "[fg=red bold]XX[reset]".into()
            } else if let Some(seconds) = unit.limb_recycle().get(&section).filter(|s| **s > 0) {
                format!("{:<3}", seconds.div_ceil(2))
            } else {
                "[fg=green]Rdy[reset]".into()
            };
            entries.push(format!("{name}[{arm}]: {status}"));
        }
    }
    Ok(entries)
}

/// Discard anatomy after its location label has been projected.
fn normalize<S>(row: BattleWeaponInspection<S>) -> BattleWeaponInspection<()> {
    BattleWeaponInspection {
        index: row.index,
        name: row.name,
        section: (),
        rear_mount: row.rear_mount,
        preferred_ammunition_section: row.preferred_ammunition_section,
        one_shot: row.one_shot,
        fire_mode: row.fire_mode,
        ammunition_mode: row.ammunition_mode,
        readiness: row.readiness,
        failure: row.failure,
    }
}

/// Supply color uses total rounds against capacity, independently of the paired weapon row.
fn ammunition_column(bin: &Ammunition) -> String {
    let ratio = u32::from(bin.rounds) * 100 / u32::from(bin.capacity.max(1));
    let color = if ratio >= 50 {
        "green bold"
    } else if ratio >= 25 {
        "yellow bold"
    } else {
        "red bold"
    };
    format!(
        "{:16.16} {}  [fg={color}]{:3}[reset]",
        bin.weapon.name().split_once('.').unwrap().1,
        mode_letter(bin.mode, bin.weapon),
        bin.rounds
    )
}

/// Single-character fire-mode indicators occupy one column in every weapon row.
fn fire_letter(mode: BattleFireMode) -> char {
    match mode {
        BattleFireMode::Normal => ' ',
        BattleFireMode::Heat => 'H',
        BattleFireMode::Hotload => 'H',
        BattleFireMode::Ultra => 'U',
        BattleFireMode::Rapid => 'F',
        BattleFireMode::Rotary2 => '2',
        BattleFireMode::Rotary4 => '4',
        BattleFireMode::Rotary6 => '6',
        BattleFireMode::Gatling => 'G',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Physical labels remain visible after part damage and use each weapon's actuator needs.
    #[test]
    fn physical_equipment_uses_reference_order_and_readiness() {
        for (equipment, label, needs_hand, needs_shoulder) in [
            ("Axe", "Axe", true, true),
            ("Sword", "Sword", true, true),
            ("Claw", "Claw", false, false),
            ("Mace", "Mace", true, true),
            ("Dual_Saw", "Saw", false, true),
        ] {
            let source = include_str!("../../../game/mechs/AXM-2N")
                .replace("    CRIT_5\t\t  { IS.MediumLaser - - 3 }\n", "")
                .replace("    CRIT_6\t\t  { IS.MediumLaser - - 3 }\n", "")
                .replace("    CRIT_7\t\t  { IS.MediumLaser - - 3 }\n", "")
                .replace(
                    "CRIT_8-12\t\t  { Axe",
                    &format!("CRIT_5-12\t\t  {{ {equipment}"),
                );
            let unit = BattleUnit::from_template(BattleTemplate::parse(&source).unwrap()).unwrap();
            assert_eq!(
                text::plain(&physical_readiness(&unit).unwrap().join("")),
                format!("{label}[RA]: Rdy")
            );
            for (slot, broken) in [(0, needs_shoulder), (3, needs_hand), (4, false)] {
                let mut damaged = unit.clone();
                damaged
                    .destroy_critical(CriticalLocation {
                        section: BattleSection::RightArm,
                        slot,
                    })
                    .unwrap();
                let before = damaged.clone();
                assert_eq!(
                    text::plain(&physical_readiness(&damaged).unwrap().join("")),
                    format!("{label}[RA]: {}", if broken { "XX" } else { "Rdy" })
                );
                assert_eq!(damaged, before);
            }
            let mut recycling = unit.clone();
            recycling.limb_recycle.insert(BattleSection::RightArm, 3);
            assert_eq!(
                text::plain(&physical_readiness(&recycling).unwrap().join("")),
                format!("{label}[RA]: 2  ")
            );
        }
    }
}
