//! Administrative preference masks project typed gameplay settings without separate saved flags.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Bits for the six shared preferences, in their persistent field's public order.
const MASKS: [u32; 6] = [2, 4, 8, 16, 64, 128];

/// Project the same booleans read by cockpit controls and combat services.
pub(super) fn read(world: &World, id: ObjectId) -> Result<u32> {
    let values = super::with_unit!(
        world.btech.unit(id).context("Unit is unavailable")?,
        |unit| {
            [
                unit.searchlight_warning(),
                unit.auto_fall(),
                !unit.armor_warning(),
                !unit.ammunition_warning(),
                unit.autocon_shutdown(),
                unit.friendly_fire_safety(),
            ]
        }
    );
    Ok(values.into_iter().zip(MASKS).fold(
        super::auxiliary_preferences::bits(world, id)?,
        |bits, (enabled, mask)| bits | if enabled { mask } else { 0 },
    ))
}

/// Update existing authoritative settings after wizard admission and before whole-world validation.
pub(super) fn set(world: &mut World, id: ObjectId, bits: u32) -> Result<()> {
    let allowed = MASKS
        .into_iter()
        .fold(super::auxiliary_preferences::MASK, |bits, mask| bits | mask);
    ensure!(
        bits & !allowed == 0,
        "Preference bitvector includes settings not implemented for this chassis"
    );
    super::auxiliary_preferences::set_bits(world, id, bits)?;
    let fields = super::with_unit_mut!(
        world.btech.unit_mut(id).context("Unit is unavailable")?,
        |unit| {
            [
                &mut unit.searchlight_warning,
                &mut unit.auto_fall,
                &mut unit.no_armor_warning,
                &mut unit.no_ammunition_warning,
                &mut unit.autocon_shutdown,
                &mut unit.friendly_fire_safety,
            ]
        }
    );
    for (field, mask) in fields.into_iter().zip(MASKS) {
        *field = bits & mask != 0;
    }
    Ok(())
}
