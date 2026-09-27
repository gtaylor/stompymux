//! Shutdown host transactions publish fall, collision and crew effects through shared combat resolvers.
use crate::{Config, ObjectId, Scripts};
use anyhow::Result;

/// Secondary shutdown consequences retained until the enclosing host action can publish them.
#[derive(Default)]
pub(super) struct ShutdownEffects {
    pub pilot_notices: Vec<super::BattlePilotNotice>,
    pub falls: Vec<super::BattleFallReport>,
    pub vehicle_falls: Vec<super::BattleVehicleFallReport>,
    pub stacking: super::stacking::StackingEffects,
}

/// Publish nested shutdown effects once, before newly lethal crew are evacuated.
pub(super) fn publish(scripts: &Scripts, config: &Config, effects: &ShutdownEffects) -> Result<()> {
    for fall in &effects.falls {
        super::evacuation::publish_fall_consequences(scripts, config, fall)?;
    }
    for fall in &effects.vehicle_falls {
        super::evacuation::publish_vehicle_fall_consequences(scripts, config, fall)?;
    }
    super::evacuation::publish_stacking_consequences(scripts, config, &effects.stacking)
}

/// Shut down a piloted unit with transactional damage, notices, personal injury and evacuation.
pub fn stop_unit_action(
    scripts: &Scripts,
    config: &Config,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<Vec<super::BattleNotice>> {
    scripts.atomic(|before| {
        super::power::check_shutdown_control(before, id, pilot)?;
        let mut effects = ShutdownEffects::default();
        let notices = super::power::stop_admitted_in_action(
            &mut scripts.world_mut(),
            id,
            super::BattleFallRules::configured(config),
            &mut effects,
        )?;
        super::piloting::publish_ordered_notices(scripts, &notices, &effects.pilot_notices)?;
        publish(scripts, config, &effects)?;
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world().validate_action(config)?;
        scripts.effects.validate()?;
        Ok(notices)
    })
}
