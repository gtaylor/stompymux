//! Read-only combat warning decisions; reports own delivery and transaction rollback.
use super::{BattleSection, BattleUnit, BattleWeaponUse};

/// Reference thresholds use integer division before comparing remaining armor.
pub(super) fn armor_severity(original: u16, remaining: u16) -> u8 {
    if original == 0 {
        return 0;
    }
    if remaining == 0 {
        return 3;
    }
    if remaining < original / 4 {
        return 2;
    }
    if remaining < original / 2 {
        return 1;
    }
    0
}

/// Select front or rear armor exactly as material damage does.
pub(super) fn armor_level(unit: &BattleUnit, section: BattleSection, rear: bool) -> u8 {
    let original = &unit.definition().sections[&section];
    let state = &unit.sections()[&section];
    if rear
        && matches!(
            section,
            BattleSection::LeftTorso | BattleSection::RightTorso | BattleSection::CenterTorso
        )
    {
        return armor_severity(original.rear, state.rear);
    }
    armor_severity(original.armor, state.armor)
}

/// A warning describes only the final severity reached by this armor phase.
pub(super) fn armor_message(section: BattleSection, rear: bool, severity: u8) -> String {
    let location = match section {
        BattleSection::LeftArm => "LA",
        BattleSection::RightArm => "RA",
        BattleSection::LeftTorso => "LT",
        BattleSection::RightTorso => "RT",
        BattleSection::CenterTorso => "CT",
        BattleSection::LeftLeg => "LL",
        BattleSection::RightLeg => "RL",
        BattleSection::Head => "H",
    };
    armor_text(location, rear, severity)
}

/// Shared warning severity text with the caller's anatomy-specific location label.
fn armor_text(location: &str, rear: bool, severity: u8) -> String {
    let (color, state) = match severity {
        1 => ("[fg=green bold]", "low."),
        2 => ("[fg=yellow bold]", "critical!"),
        3 => ("[fg=red]", "BREACHED!"),
        _ => unreachable!("warning requires nonzero severity"),
    };
    format!(
        "{color}WARNING: {location}{} Armor {state}[reset]",
        if rear { " (Rear)" } else { "" }
    )
}

/// Match the warning window before expenditure, including double-rate and gatling offsets.
fn ammunition_severity(capacity: u32, remaining: u32, offset: u32) -> Option<bool> {
    let low = (capacity / 8).clamp(3, 30);
    let crosses = |threshold| remaining >= threshold && remaining <= threshold + offset;
    if crosses(low) {
        return Some(true);
    }
    crosses(low * 2).then_some(false)
}

/// Half-slot warning weights follow the reference's preference accounting, not usable rounds.
fn ammunition_weight(half_ton: bool, mode: super::BattleAmmunitionMode) -> u32 {
    use super::BattleAmmunitionMode as Mode;
    if half_ton || matches!(mode, Mode::ArmorPiercing | Mode::Precision) {
        return 2;
    }
    if mode == Mode::Caseless {
        return 1;
    }
    4
}

/// Check all installed bins of this weapon, independent of currently selected ammunition mode.
pub(super) fn ammunition_message(
    unit: &BattleUnit,
    expenditure: &BattleWeaponUse,
) -> Option<String> {
    if !unit.ammunition_warning() || expenditure.ammunition.is_empty() {
        return None;
    }
    let offset = launch_offset(expenditure.gatling_damage, expenditure.fire_mode);
    ammunition_warning(
        &unit.loadout().ok()?.ammunition,
        unit.ammunition(),
        expenditure.weapon,
        offset,
    )
}

/// Dumping checks each bin before ejection using the number of extra rounds in that step.
pub(super) fn dumping_message(
    unit: &BattleUnit,
    weapon: super::BattleWeapon,
    rounds: u16,
) -> Option<String> {
    if !unit.ammunition_warning() || rounds == 0 {
        return None;
    }
    ammunition_warning(
        &unit.loadout().ok()?.ammunition,
        unit.ammunition(),
        weapon,
        u32::from(rounds - 1),
    )
}

/// Shared installed-bin accounting for firing and ejection warnings.
fn ammunition_warning<L>(
    bins: &[super::AmmunitionBin<L>],
    rounds: &[u16],
    weapon: super::BattleWeapon,
    offset: u32,
) -> Option<String> {
    let mut capacity = 0;
    let mut remaining = 0;
    for (index, bin) in bins.iter().enumerate() {
        if bin.weapon != weapon {
            continue;
        }
        let units = ammunition_weight(bin.half_ton, bin.mode);
        capacity += u32::from(
            bin.weapon
                .profile_for_ammunition(bin.mode)
                .ammunition_per_ton,
        ) * units;
        remaining += u32::from(rounds[index]);
    }
    let severe = ammunition_severity(capacity / 2, remaining, offset)?;
    let name = weapon.name();
    let name = name.strip_prefix("IS.").unwrap_or(name);
    Some(format!(
        "{}WARNING: Ammo for {name} is running low.[reset]",
        if severe {
            "[fg=red bold]"
        } else {
            "[fg=yellow bold]"
        }
    ))
}

/// Firing warning windows depend on effective burst behavior, never the unit's anatomy.
fn launch_offset(gatling: Option<u8>, mode: super::BattleFireMode) -> u32 {
    u32::from(gatling.unwrap_or(0)).max(u32::from(mode.is_double_shot()))
}

/// Vehicle launches use the same installed-bin weights and pre-expenditure warning windows.
pub(super) fn vehicle_ammunition_message(
    unit: &super::BattleVehicle,
    expenditure: &super::BattleVehicleWeaponUse,
) -> Option<String> {
    if !unit.ammunition_warning() || expenditure.ammunition.is_empty() {
        return None;
    }
    ammunition_warning(
        &unit.loadout().ok()?.ammunition,
        unit.ammunition(),
        expenditure.weapon,
        launch_offset(expenditure.gatling_damage, expenditure.fire_mode),
    )
}

/// Vehicle armor locations supply labels to the common severity message.
pub(super) fn vehicle_armor_message(section: super::BattleVehicleSection, severity: u8) -> String {
    let label: String = section
        .name()
        .chars()
        .filter(char::is_ascii_uppercase)
        .collect();
    armor_text(&label, false, severity)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn armor_integer_thresholds_and_empty_original() {
        for (left, severity) in [(11, 0), (5, 0), (4, 1), (2, 1), (1, 2), (0, 3)] {
            assert_eq!(armor_severity(11, left), severity);
        }
        assert_eq!(armor_severity(0, 0), 0);
        assert_eq!(armor_severity(3, 1), 0);
        assert_eq!(armor_severity(3, 0), 3);
    }
    #[test]
    fn ammunition_bin_weights_keep_special_mode_and_half_ton_precedence() {
        use super::super::BattleAmmunitionMode as Mode;
        for (mode, expected) in [
            (Mode::Normal, 4),
            (Mode::ArmorPiercing, 2),
            (Mode::Precision, 2),
            (Mode::Caseless, 1),
        ] {
            assert_eq!(ammunition_weight(false, mode), expected);
            assert_eq!(ammunition_weight(true, mode), 2);
        }
    }
    #[test]
    fn ammunition_windows_preserve_single_double_and_gatling_offsets() {
        for offset in [0, 1, 6] {
            for remaining in 0..60 {
                let expected = if (25..=25 + offset).contains(&remaining) {
                    Some(true)
                } else if (50..=50 + offset).contains(&remaining) {
                    Some(false)
                } else {
                    None
                };
                assert_eq!(ammunition_severity(200, remaining, offset), expected);
            }
        }
        assert_eq!(ammunition_severity(5, 3, 0), Some(true));
        assert_eq!(ammunition_severity(1000, 60, 0), Some(false));
    }
}
