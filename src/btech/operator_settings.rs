//! Shared operator edits pair typed mutations with commit-staged wizard audits.
use crate::{
    ObjectId, Scripts,
    logging::{AuditRecord, targets},
};
use anyhow::Result;

/// Stage an operator-change audit only when the active filter would write it.
fn audit(scripts: &Scripts, message: String) -> Result<()> {
    if tracing::enabled!(target: targets::WIZARD, tracing::Level::INFO) {
        scripts.effects.stage_record(AuditRecord { message })?;
    }
    Ok(())
}

/// Change a skill threshold with canonical naming and atomic audit admission.
pub fn edit_skill_threshold(
    scripts: &Scripts,
    actor: ObjectId,
    name: &str,
    threshold: i64,
) -> Result<()> {
    let mut candidate = scripts.world().clone();
    super::set_skill_threshold(&mut candidate, actor, name, threshold)?;
    let skill = super::skill_definition(name).expect("validated skill name");
    audit(
        scripts,
        format!(
            "Exp threshold for {} changed to {} by #{}",
            skill.name, threshold, actor.0
        ),
    )?;
    *scripts.world_mut() = candidate;
    Ok(())
}
