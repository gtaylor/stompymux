//! The pilot's shutdown as a host transaction. A Mech or vehicle shuts down only once stopped
//! on the ground, so shutting down never moves, damages or injures anything.
use crate::{Config, ObjectId, Scripts};
use anyhow::Result;

/// Shut down a piloted unit transactionally and publish its notices.
pub fn stop_unit_action(
    scripts: &Scripts,
    config: &Config,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<Vec<super::Notice>> {
    scripts.atomic(|before| {
        super::power::check_shutdown_control(before, id, pilot)?;
        let notices = super::power::power_down_unit(&mut scripts.world_mut(), id)?;
        super::piloting::publish_ordered_notices(scripts, &notices, &[])?;
        scripts.world().validate_action(config)?;
        scripts.effects.validate()?;
        Ok(notices)
    })
}
