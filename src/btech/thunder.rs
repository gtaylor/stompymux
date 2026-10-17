//! Thunder minelaying rounds: LRM salvos fired at a hex seed minefields instead of damaging it.
use super::{AmmunitionFeedback, AmmunitionMode, HexCoordinate, MineKind, Minefield, Notice};
use crate::{ObjectId, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

/// Densest total that Thunder salvos may lay in one hex (Tactical Operations: Advanced Units &
/// Equipment p. 185); only a pre-designated field may be built up further, to thirty.
pub const THUNDER_MAXIMUM_STRENGTH: i16 = 20;

/// Density a scattered Thunder salvo loses (Tactical Operations: Advanced Units & Equipment
/// p. 185).
const SCATTER_DENSITY_LOSS: u16 = 5;

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

/// Seed or reinforce Thunder minefields after a salvo lands on `coordinate` (Tactical
/// Operations: Advanced Units & Equipment p. 185). A salvo lays a field as dense as its
/// launcher is large; Augmented rounds lay half that, rounding up, in the target hex and each
/// adjacent hex on the map; a salvo that scattered lays five points less. Weapon-delivered
/// fields in one hex total at most twenty points, joining a field of the same kind and
/// owner; landing on a pre-designated field of the same kind thickens it to at most thirty.
pub(super) fn lay(
    world: &mut World,
    shooter: ObjectId,
    map: ObjectId,
    coordinate: HexCoordinate,
    mode: AmmunitionMode,
    (launcher, scattered): (u16, bool),
) -> Result<ThunderReport> {
    let (kind, extra) = field_type(world, shooter, mode)?;
    let mut density = if mode.munition() == AmmunitionMode::ThunderAugmented {
        launcher.div_ceil(2)
    } else {
        launcher
    };
    if scattered {
        density = density.saturating_sub(SCATTER_DENSITY_LOSS);
    }
    let mut cells = vec![coordinate];
    if mode.munition() == AmmunitionMode::ThunderAugmented {
        let record = world.btech.maps().get(&map).context("Map not found")?;
        cells.extend(record.neighbors(coordinate)?.into_iter().flatten());
    }
    let mut report = ThunderReport::default();
    if density == 0 {
        return Ok(report);
    }
    let density = i16::try_from(density).unwrap_or(i16::MAX);
    for coordinate in cells {
        let here: Vec<_> = world.btech.maps()[&map]
            .ordered_minefields()
            .filter(|(_, field)| field.coordinate == coordinate)
            .map(|(ordinal, field)| (*ordinal, *field))
            .collect();
        let predesignated = here.iter().find(|(_, field)| {
            field.kind == kind
                && field.extra == extra
                && !super::minefield::weapon_delivered(world, field)
        });
        let (ordinal, mine) = if let Some(&(ordinal, field)) = predesignated {
            let strength = field
                .strength
                .saturating_add(density)
                .min(super::artillery_impact::PREDESIGNATED_MAXIMUM_DENSITY.max(field.strength));
            if strength == field.strength {
                continue;
            }
            let field = Minefield { strength, ..field };
            super::set_minefield(world, map, ordinal, Some(field))?;
            (ordinal, field)
        } else {
            let laid: i16 = here
                .iter()
                .filter(|(_, field)| super::minefield::weapon_delivered(world, field))
                .map(|(_, field)| field.strength.max(0))
                .sum();
            let strength = density.min(THUNDER_MAXIMUM_STRENGTH - laid);
            if strength <= 0 {
                continue;
            }
            let existing = here.iter().find(|(_, field)| {
                field.kind == kind && field.extra == extra && field.owner == shooter
            });
            if let Some(&(ordinal, field)) = existing {
                let field = Minefield {
                    strength: field.strength + strength,
                    ..field
                };
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
            }
        };
        report.fields.push(ThunderField { ordinal, mine });
    }
    Ok(report)
}

/// Where a missed Thunder salvo lands, scattered as artillery is (Tactical Operations: Advanced
/// Units & Equipment p. 185; Advanced Rules pp. 150, 153): a die picks the hexside on the
/// Scatter Diagram, clockwise from north, and the salvo travels its margin of failure in hexes
/// when fired indirectly, or 1D6 hexes when fired directly. `None` when it leaves the map.
pub(super) fn scatter(
    world: &mut World,
    shooter: ObjectId,
    map: ObjectId,
    target: HexCoordinate,
    indirect_margin: Option<u16>,
) -> Result<Option<HexCoordinate>> {
    let dice = super::dice::unit_dice_mut(world, shooter)?;
    let hexside = usize::from(dice.die(6)? - 1);
    let hexes = match indirect_margin {
        Some(margin) => margin,
        None => dice.die(6)?,
    };
    let mut landing = target;
    for _ in 0..hexes {
        landing = landing.neighbors()?[hexside];
    }
    let record = world.btech.maps().get(&map).context("Map not found")?;
    Ok(record
        .base_hex(i64::from(landing.x), i64::from(landing.y))
        .ok()
        .map(|_| landing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Weapon;

    /// Only LRM families take specialized Thunder rounds, and plain mine rounds count as Thunder.
    #[test]
    fn thunder_rounds_belong_to_lrm_launchers() {
        for weapon in [Weapon::Lrm20, Weapon::Nlrm10] {
            assert!(weapon.supports_thunder());
            assert!(AmmunitionMode::ThunderActive.supports(weapon));
        }
        // Clan launchers fire plain Thunder rounds but not the Inner Sphere variants (Tactical
        // Operations: Advanced Units & Equipment p. 185).
        assert!(AmmunitionMode::Mine.supports(Weapon::ClanLrm5));
        assert!(!AmmunitionMode::ThunderActive.supports(Weapon::ClanLrm5));
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
