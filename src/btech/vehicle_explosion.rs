//! Catastrophic vehicle explosions destroy local sections with aircraft rear CASE containment.
use super::{DamagePhase, Notice, System, Vehicle, VehicleSection};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Completed catastrophe; occupant and visibility notices remain caller-published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VehicleExplosion {
    pub contained: bool,
    pub destroyed_sections: Vec<VehicleSection>,
    pub notices: Vec<Notice>,
    pub broadcasts: Vec<Notice>,
}

impl Vehicle {
    /// Installed CASE provides rear power-plant containment; Clan technology alone does not.
    /// Configuration text does not install CASE equipment or add containment by itself.
    pub fn has_powerplant_containment(&self) -> bool {
        self.definition().sections.values().any(|layout| {
            layout.criticals.values().any(|part| {
                matches!(
                    System::named(&part.equipment),
                    Some(System::Case | System::CaseIi)
                )
            })
        })
    }
}

/// Resolve a catastrophe inside a private critical candidate; character evacuation is separate.
pub(super) fn explode_in_candidate(
    world: &mut World,
    id: ObjectId,
    contained: bool,
) -> Result<VehicleExplosion> {
    ensure!(
        !world
            .btech
            .vehicles()
            .get(&id)
            .context("Vehicle is unavailable")?
            .is_destroyed(),
        "Vehicle is destroyed"
    );
    explode_followup_in_candidate(world, id, contained)
}

/// An admitted blast can destroy remaining material on an existing wreck.
pub(super) fn explode_followup_in_candidate(
    world: &mut World,
    id: ObjectId,
    contained: bool,
) -> Result<VehicleExplosion> {
    let object = world.objects.get(&id).context("Vehicle is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going),
        "Vehicle is unavailable"
    );
    if !contained {
        ensure!(
            !world
                .objects
                .iter()
                .any(|(child, object)| object.location == Some(id)
                    && world
                        .btech
                        .units()
                        .get(child)
                        .is_some_and(|unit| unit.class_code == 8)),
            "Vehicle explosion requires carried battlesuit casualty handling; attack must remain uncommitted"
        );
    }
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    let sections: Vec<_> = vehicle
        .sections()
        .iter()
        .filter(|(section, state)| {
            state.internal > 0 && (!contained || **section == VehicleSection::Rear)
        })
        .map(|(&section, state)| (section, state.internal))
        .collect();
    let mut result = VehicleExplosion {
        contained,
        destroyed_sections: Vec::new(),
        notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    let pilot = vehicle.pilot();
    if !contained {
        vehicle.kill_crew();
    }
    for (section, amount) in sections {
        let damage = vehicle.damage_phase(section, amount, DamagePhase::Internal)?;
        result.destroyed_sections.extend(damage.destroyed_sections);
        let name = section.name().replace('_', " ");
        result.notices.push(Notice {
            unit: id,
            text: format!("Your {name} has been destroyed!"),
        });
        result.broadcasts.push(Notice {
            unit: id,
            text: format!("'s {name} has been destroyed!"),
        });
    }
    if !contained
        && let Some(pilot) = pilot
        && let Some(recovery) = world.btech.recoveries.get_mut(&pilot)
    {
        recovery.remaining = 0;
    }
    Ok(result)
}
