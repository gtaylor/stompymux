//! Compact status export with independent weapon rows and grouped live ammunition columns.
use super::{AmmunitionMode as Mode, Mech, Weapon};
use anyhow::Result;
use std::fmt::Write;

/// Stable mode letters used by the compact ammunition column.
pub(super) fn mode_letter(mode: Mode, weapon: Weapon) -> char {
    match mode {
        Mode::Normal => ' ',
        Mode::INarcExplosive => 'X',
        Mode::INarcHaywire => 'Y',
        Mode::INarcEcm => 'E',
        Mode::INarcNemesis => 'Z',
        Mode::Cluster if weapon.is_artillery() => 'C',
        Mode::Cluster => 'L',
        Mode::Smoke => 'S',
        Mode::Mine => 'M',
        Mode::Artemis => 'A',
        Mode::SemiGuided => 'G',
        Mode::Swarm => 'W',
        Mode::Swarm1 => '1',
        Mode::Stinger => 'T',
        Mode::MmlLrm => 'L',
        // Special MML long-range rounds report their round letter, matching the reference precedence.
        Mode::MmlLrmArtemis
        | Mode::MmlLrmNarc
        | Mode::MmlLrmSwarm
        | Mode::MmlLrmSwarm1
        | Mode::MmlLrmSemiGuided
        | Mode::MmlLrmStinger => mode_letter(mode.munition(), weapon),
        Mode::ExtendedRange => 'R',
        Mode::HighExplosive => 'X',
        Mode::Narc => {
            if weapon.is_narc() {
                'E'
            } else {
                'N'
            }
        }
        Mode::Precision => 'P',
        Mode::Flechette => 'F',
        Mode::ArmorPiercing => 'R',
        Mode::Caseless => 'U',
        Mode::Incendiary => 'D',
        Mode::Inferno => 'I',
        Mode::ThunderAugmented => 'H',
        Mode::ThunderVibrabomb => 'V',
        Mode::ThunderActive => 'K',
    }
}

/// One ammunition kind, retaining first-encounter order across sections and slots.
pub(super) struct Ammunition {
    pub(super) weapon: Weapon,
    pub(super) mode: Mode,
    pub(super) rounds: u16,
    pub(super) capacity: u16,
}

/// Export chassis and optionally weapons. Armor selections do not change this compact record.
pub(super) fn render(unit: &Mech, weapons: bool) -> Result<String> {
    let definition = unit.definition();
    let running = (unit.mobility().maximum_speed / 10.75) as u16;
    let jumping = (unit.jump_capacity(100)?.speed / 10.75) as u16;
    let sinks = unit.cooling_capacity();
    let record = format!(
        "{} {} {} {}/{}/{} {} ",
        crate::text::escape(&definition.reference),
        crate::text::escape(&definition.name),
        definition.tons,
        running * 2 / 3,
        running,
        jumping,
        sinks
    );
    if !weapons {
        return Ok(record);
    }
    let loadout = unit.loadout()?;
    let ammunition = loadout
        .ammunition
        .iter()
        .enumerate()
        .map(|(index, bin)| Ammunition {
            weapon: bin.weapon,
            mode: bin.mode,
            capacity: bin.capacity,
            rounds: if unit.critical_unavailable(bin.location) {
                0
            } else {
                unit.ammunition()[index]
            },
        });
    let mounts = loadout.weapons.iter().map(|mount| {
        (
            mount.weapon,
            unit.chassis().section_name(mount.criticals[0].section),
        )
    });
    Ok(append_equipment(record, mounts, ammunition))
}

/// Vehicle anatomy supplies rows to the same export grouping and formatting rules.
pub(super) fn render_vehicle(unit: &super::Vehicle, weapons: bool) -> Result<String> {
    let definition = unit.definition();
    let running = (unit.maximum_speed() / 10.75) as u16;
    let loadout = unit.loadout()?;
    let sinks = unit.cooling_capacity()?;
    let record = format!(
        "{} {} {} {}/{}/0 {} ",
        crate::text::escape(&definition.reference),
        crate::text::escape(&definition.name),
        definition.tons,
        running * 2 / 3,
        running,
        sinks
    );
    if !weapons {
        return Ok(record);
    }
    let ammunition = loadout
        .ammunition
        .iter()
        .enumerate()
        .map(|(index, bin)| Ammunition {
            weapon: bin.weapon,
            mode: bin.mode,
            capacity: bin.capacity,
            rounds: if unit.critical_destroyed(bin.location) {
                0
            } else {
                unit.ammunition()[index]
            },
        });
    let mounts = loadout
        .weapons
        .iter()
        .map(|mount| (mount.weapon, mount.criticals[0].section.name()));
    Ok(append_equipment(record, mounts, ammunition))
}

/// Group surviving ammunition in encounter order and pair it with independent weapon rows.
fn append_equipment<'a>(
    mut record: String,
    mounts: impl Iterator<Item = (Weapon, &'a str)>,
    bins: impl Iterator<Item = Ammunition>,
) -> String {
    let ammunition = group_ammunition(bins);
    let mut mount_count = 0;
    for (index, (weapon, section)) in mounts.enumerate() {
        mount_count += 1;
        let name = weapon.name().split_once('.').unwrap().1;
        write!(record, "{name}|{}", section).unwrap();
        if let Some(ammo) = ammunition.get(index) {
            write!(
                record,
                "|{}|{}",
                ammo.weapon.name().split_once('.').unwrap().1,
                ammo.rounds
            )
            .unwrap();
            let letter = mode_letter(ammo.mode, ammo.weapon);
            if letter != ' ' {
                write!(record, "|{letter}").unwrap();
            }
        }
        record.push(' ');
    }
    let mut extra = Vec::new();
    for ammo in ammunition.iter().skip(mount_count) {
        let percentage = u32::from(ammo.rounds) * 100 / u32::from(ammo.capacity.max(1));
        let color = if percentage >= 50 {
            "[fg=green bold]"
        } else if percentage >= 25 {
            "[fg=yellow bold]"
        } else {
            "[fg=red bold]"
        };
        let name = ammo.weapon.name().split_once('.').unwrap().1;
        extra.push(format!(
            "                                                  || {:16.16} {}  {color}{:3}[reset]",
            name,
            mode_letter(ammo.mode, ammo.weapon),
            ammo.rounds
        ));
    }
    extra.push(record);
    extra.join("\r\n")
}

/// Aggregate live supplies once for compact export and the cockpit table.
pub(super) fn group_ammunition(bins: impl IntoIterator<Item = Ammunition>) -> Vec<Ammunition> {
    let mut ammunition: Vec<Ammunition> = Vec::new();
    for bin in bins {
        if let Some(group) = ammunition
            .iter_mut()
            .find(|a| a.weapon == bin.weapon && a.mode == bin.mode)
        {
            group.rounds = group.rounds.saturating_add(bin.rounds);
            group.capacity = group.capacity.saturating_add(bin.capacity);
        } else {
            ammunition.push(bin);
        }
    }
    ammunition.retain(|a| a.rounds > 0);
    ammunition
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Artillery letters distinguish its payloads from the shared conventional catalogue labels.
    #[test]
    fn artillery_payload_letters() {
        assert_eq!(mode_letter(Mode::Cluster, Weapon::ArrowIv), 'C');
        assert_eq!(mode_letter(Mode::Cluster, Weapon::Lbx10), 'L');
        assert_eq!(mode_letter(Mode::Smoke, Weapon::ArrowIv), 'S');
        assert_eq!(mode_letter(Mode::Mine, Weapon::ArrowIv), 'M');
        assert_eq!(mode_letter(Mode::Artemis, Weapon::Lrm5), 'A');
        assert_eq!(mode_letter(Mode::Narc, Weapon::Srm4), 'N');
    }
}
