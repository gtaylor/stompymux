//! Thunder minelaying rounds: LRM salvos fired at a hex seed minefields instead of damaging it.
use super::{AmmunitionFeedback, AmmunitionMode, HexCoordinate, MineKind, Minefield, Notice};
use crate::{ObjectId, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

/// Densest field that repeated Thunder salvos may build up in one hex.
pub const THUNDER_MAXIMUM_STRENGTH: i16 = 30;

/// Toggle a controlled, intact and recycled LRM launcher between normal rounds and one of
/// the specialized Thunder rounds.
pub fn toggle_thunder(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    mode: AmmunitionMode,
) -> Result<AmmunitionMode> {
    ensure!(
        matches!(
            mode,
            AmmunitionMode::ThunderAugmented
                | AmmunitionMode::ThunderVibrabomb
                | AmmunitionMode::ThunderActive
        ),
        "Invalid Thunder round"
    );
    super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        super::weapon_controls::selectable_munition(world, id, index, mode),
        "That weapon cannot fire Thunder rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world, id, index, mode,
    ))
}

/// The Thunder round each cockpit command selects.
fn command_mode(name: &str) -> AmmunitionMode {
    if name.eq_ignore_ascii_case("fireaugmented") {
        AmmunitionMode::ThunderAugmented
    } else if name.eq_ignore_ascii_case("firevibrabomb") {
        AmmunitionMode::ThunderVibrabomb
    } else {
        AmmunitionMode::ThunderActive
    }
}

/// Use the ordinary bounded multi-weapon selection and world/effect checkpoint.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let mode = command_mode(&input.name);
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_thunder(world, id, pilot, index, mode).map(|mode| mode.thunder_message(index))
    })
}

/// One minefield a Thunder salvo created or reinforced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ThunderField {
    pub ordinal: u32,
    pub mine: Minefield,
}

/// Minefields seeded by one Thunder salvo, and the shooter's feedback.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ThunderReport {
    pub fields: Vec<ThunderField>,
}

impl ThunderReport {
    /// Tell the shooter what their salvo left behind.
    pub(super) fn notices(&self, shooter: ObjectId) -> Vec<Notice> {
        let Some(center) = self.fields.first() else {
            return Vec::new();
        };
        let kind = match center.mine.kind {
            MineKind::Vibra => "vibrabomb ",
            MineKind::Active => "active ",
            _ => "",
        };
        let spread = if self.fields.len() > 1 {
            " and the surrounding hexes"
        } else {
            ""
        };
        vec![Notice {
            unit: shooter,
            text: format!(
                "Your Thunder salvo seeds a {kind}minefield at {}, {}{spread}.",
                center.mine.coordinate.x, center.mine.coordinate.y
            ),
        }]
    }
}

/// The mine kind and weight threshold a Thunder round lays. Vibrabomb fields trip on units
/// heavier than the unit that laid them, so the layer can cross its own field.
fn field_type(world: &World, shooter: ObjectId, mode: AmmunitionMode) -> Result<(MineKind, i32)> {
    Ok(match mode.munition() {
        AmmunitionMode::Mine | AmmunitionMode::ThunderAugmented => (MineKind::Standard, 0),
        AmmunitionMode::ThunderActive => (MineKind::Active, 0),
        AmmunitionMode::ThunderVibrabomb => {
            let mass = crate::btech::with_unit!(
                world
                    .btech
                    .unit(shooter)
                    .context("Shooter is not constructed")?,
                |unit| { unit.effective_mass()? }
            );
            let tons = i32::try_from(mass / 1024)?;
            (MineKind::Vibra, (tons + 1).clamp(10, 100))
        }
        _ => bail!("Not a Thunder round"),
    })
}

/// Seed or reinforce Thunder minefields after a salvo lands on `coordinate`. The field's
/// strength is the salvo's damage. Augmented rounds spread half of it, rounding up, over the
/// target hex and each adjacent hex on the map. A salvo landing on a field it matches
/// thickens that field instead of adding another, up to the maximum strength.
pub(super) fn lay(
    world: &mut World,
    shooter: ObjectId,
    map: ObjectId,
    coordinate: HexCoordinate,
    mode: AmmunitionMode,
    damage: u16,
) -> Result<ThunderReport> {
    let (kind, extra) = field_type(world, shooter, mode)?;
    let mut cells = Vec::new();
    if mode.munition() == AmmunitionMode::ThunderAugmented {
        let spread = damage.div_ceil(2);
        cells.push((coordinate, spread));
        let record = world.btech.maps().get(&map).context("Map not found")?;
        for neighbor in record.neighbors(coordinate)?.into_iter().flatten() {
            cells.push((neighbor, spread));
        }
    } else {
        cells.push((coordinate, damage));
    }
    let mut report = ThunderReport::default();
    for (coordinate, strength) in cells {
        if strength == 0 {
            continue;
        }
        let strength = i16::try_from(strength)
            .unwrap_or(i16::MAX)
            .min(THUNDER_MAXIMUM_STRENGTH);
        let existing = world.btech.maps()[&map]
            .ordered_minefields()
            .find(|(_, field)| {
                field.coordinate == coordinate
                    && field.kind == kind
                    && field.extra == extra
                    && field.owner == shooter
            })
            .map(|(ordinal, field)| (*ordinal, *field));
        let (ordinal, mine) = if let Some((ordinal, mut field)) = existing {
            field.strength = field
                .strength
                .saturating_add(strength)
                .min(THUNDER_MAXIMUM_STRENGTH);
            super::set_minefield(world, map, ordinal, Some(field))?;
            (ordinal, field)
        } else {
            let field = Minefield {
                coordinate,
                kind,
                strength,
                extra,
                owner: shooter,
            };
            (super::insert_minefield(world, map, field)?, field)
        };
        report.fields.push(ThunderField { ordinal, mine });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Weapon;

    /// Only LRM families take specialized Thunder rounds, and plain mine rounds count as Thunder.
    #[test]
    fn thunder_rounds_belong_to_lrm_launchers() {
        for weapon in [Weapon::Lrm20, Weapon::ClanLrm5, Weapon::Nlrm10] {
            assert!(weapon.supports_thunder());
            assert!(AmmunitionMode::ThunderActive.supports(weapon));
        }
        for weapon in [
            Weapon::Srm6,
            Weapon::Mml9,
            Weapon::ClanStreakLrm20,
            Weapon::Elrm10,
        ] {
            assert!(!AmmunitionMode::ThunderAugmented.supports(weapon));
        }
        assert!(AmmunitionMode::Mine.is_thunder());
        assert!(AmmunitionMode::ThunderVibrabomb.is_thunder());
        assert!(!AmmunitionMode::Smoke.is_thunder());
        assert_eq!(
            command_mode("FIREVIBRABOMB"),
            AmmunitionMode::ThunderVibrabomb
        );
        assert_eq!(
            AmmunitionMode::ThunderAugmented.thunder_message(2),
            "Weapon 2 has been set to fire Thunder-Augmented missiles."
        );
    }
}
