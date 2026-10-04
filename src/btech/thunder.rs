//! Thunder minelaying rounds: LRM salvos fired at a hex seed minefields instead of damaging it.
use super::{
    BattleAmmunitionMode, BattleMineKind, BattleMinefield, BattleNotice, BattleWeapon,
    HexCoordinate,
};
use crate::{ObjectId, World};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

/// Densest field that repeated Thunder salvos may build up in one hex.
pub const THUNDER_MAXIMUM_STRENGTH: i16 = 30;

impl BattleWeapon {
    /// LRM launchers that accept the specialized Thunder rounds.
    pub fn supports_thunder(self) -> bool {
        matches!(
            self,
            Self::Lrm5
                | Self::Lrm10
                | Self::Lrm15
                | Self::Lrm20
                | Self::Nlrm5
                | Self::Nlrm10
                | Self::Nlrm15
                | Self::Nlrm20
                | Self::ClanLrm5
                | Self::ClanLrm10
                | Self::ClanLrm15
                | Self::ClanLrm20
        )
    }
}

impl BattleAmmunitionMode {
    /// Missile rounds that lay a minefield when fired at a hex. Plain mine rounds are the
    /// original Thunder munition.
    pub fn is_thunder(self) -> bool {
        matches!(
            self.munition(),
            Self::Mine | Self::ThunderAugmented | Self::ThunderVibrabomb | Self::ThunderActive
        )
    }

    /// Cockpit feedback shared by native commands and Lua.
    pub(crate) fn thunder_message(self, index: usize) -> String {
        let name = match self.munition() {
            Self::ThunderAugmented => "Thunder-Augmented",
            Self::ThunderVibrabomb => "Thunder-Vibrabomb",
            Self::ThunderActive => "Thunder-Active",
            _ => return format!("Weapon {index} has been set to fire normal missiles"),
        };
        format!("Weapon {index} has been set to fire {name} missiles.")
    }
}

/// Toggle a controlled, intact and recycled LRM launcher between normal rounds and one of
/// the specialized Thunder rounds.
pub fn toggle_thunder(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
    mode: BattleAmmunitionMode,
) -> Result<BattleAmmunitionMode> {
    ensure!(
        matches!(
            mode,
            BattleAmmunitionMode::ThunderAugmented
                | BattleAmmunitionMode::ThunderVibrabomb
                | BattleAmmunitionMode::ThunderActive
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
fn command_mode(name: &str) -> BattleAmmunitionMode {
    if name.eq_ignore_ascii_case("fireaugmented") {
        BattleAmmunitionMode::ThunderAugmented
    } else if name.eq_ignore_ascii_case("firevibrabomb") {
        BattleAmmunitionMode::ThunderVibrabomb
    } else {
        BattleAmmunitionMode::ThunderActive
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
pub struct BattleThunderField {
    pub ordinal: u32,
    pub mine: BattleMinefield,
}

/// Minefields seeded by one Thunder salvo, and the shooter's feedback.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BattleThunderReport {
    pub fields: Vec<BattleThunderField>,
}

impl BattleThunderReport {
    /// Tell the shooter what their salvo left behind.
    pub(super) fn notices(&self, shooter: ObjectId) -> Vec<BattleNotice> {
        let Some(center) = self.fields.first() else {
            return Vec::new();
        };
        let kind = match center.mine.kind {
            BattleMineKind::Vibra => "vibrabomb ",
            BattleMineKind::Active => "active ",
            _ => "",
        };
        let spread = if self.fields.len() > 1 {
            " and the surrounding hexes"
        } else {
            ""
        };
        vec![BattleNotice {
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
fn field_type(
    world: &World,
    shooter: ObjectId,
    mode: BattleAmmunitionMode,
) -> Result<(BattleMineKind, i32)> {
    Ok(match mode.munition() {
        BattleAmmunitionMode::Mine | BattleAmmunitionMode::ThunderAugmented => {
            (BattleMineKind::Standard, 0)
        }
        BattleAmmunitionMode::ThunderActive => (BattleMineKind::Active, 0),
        BattleAmmunitionMode::ThunderVibrabomb => {
            let mass = if let Some(vehicle) = world.btech.vehicles().get(&shooter) {
                vehicle.effective_mass()?
            } else {
                world
                    .btech
                    .constructed_units()
                    .get(&shooter)
                    .context("Shooter is not constructed")?
                    .effective_mass()?
            };
            let tons = i32::try_from(mass / 1024)?;
            (BattleMineKind::Vibra, (tons + 1).clamp(10, 100))
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
    mode: BattleAmmunitionMode,
    damage: u16,
) -> Result<BattleThunderReport> {
    let (kind, extra) = field_type(world, shooter, mode)?;
    let mut cells = Vec::new();
    if mode.munition() == BattleAmmunitionMode::ThunderAugmented {
        let spread = damage.div_ceil(2);
        cells.push((coordinate, spread));
        let record = world.btech.maps().get(&map).context("Map not found")?;
        for neighbor in coordinate.neighbors()? {
            if record
                .base_hex(i64::from(neighbor.x), i64::from(neighbor.y))
                .is_ok()
            {
                cells.push((neighbor, spread));
            }
        }
    } else {
        cells.push((coordinate, damage));
    }
    let mut report = BattleThunderReport::default();
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
            let field = BattleMinefield {
                coordinate,
                kind,
                strength,
                extra,
                owner: shooter,
            };
            (super::insert_minefield(world, map, field)?, field)
        };
        report.fields.push(BattleThunderField { ordinal, mine });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only LRM families take specialized Thunder rounds, and plain mine rounds count as Thunder.
    #[test]
    fn thunder_rounds_belong_to_lrm_launchers() {
        for weapon in [
            BattleWeapon::Lrm20,
            BattleWeapon::ClanLrm5,
            BattleWeapon::Nlrm10,
        ] {
            assert!(weapon.supports_thunder());
            assert!(BattleAmmunitionMode::ThunderActive.supports(weapon));
        }
        for weapon in [
            BattleWeapon::Srm6,
            BattleWeapon::Mml9,
            BattleWeapon::ClanStreakLrm20,
            BattleWeapon::Elrm10,
        ] {
            assert!(!BattleAmmunitionMode::ThunderAugmented.supports(weapon));
        }
        assert!(BattleAmmunitionMode::Mine.is_thunder());
        assert!(BattleAmmunitionMode::ThunderVibrabomb.is_thunder());
        assert!(!BattleAmmunitionMode::Smoke.is_thunder());
        assert_eq!(
            command_mode("FIREVIBRABOMB"),
            BattleAmmunitionMode::ThunderVibrabomb
        );
        assert_eq!(
            BattleAmmunitionMode::ThunderAugmented.thunder_message(2),
            "Weapon 2 has been set to fire Thunder-Augmented missiles."
        );
    }
}
