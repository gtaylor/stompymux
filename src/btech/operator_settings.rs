//! Shared operator edits pair typed mutations with commit-staged wizard diagnostics.
use crate::{
    Config, ObjectId, Scripts,
    logging::{Category, Record},
};
use anyhow::Result;

/// Record successful operator changes only when wizard diagnostics are enabled.
fn audit(scripts: &Scripts, config: &Config, message: String) -> Result<()> {
    if Category::Wizard.enabled(config) {
        scripts
            .effects
            .stage_record(Record::new(config, "WIZ", "CHANGE", &message))?;
    }
    Ok(())
}

/// Change weapon settings through the shared typed control and audit recycle edits.
pub fn edit_weapon_settings(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    name: &str,
    value: i64,
    recycle: bool,
) -> Result<super::BattleWeaponValues> {
    let mut candidate = scripts.world().clone();
    let values = if recycle {
        super::set_weapon_recycle(&mut candidate, actor, name, value)?
    } else {
        super::set_weapon_battle_value(&mut candidate, actor, name, value)?
    };
    if recycle {
        let weapon = super::BattleWeapon::parse_operator_name(name)?;
        audit(
            scripts,
            config,
            format!(
                "VRT for {} set to {} by #{}",
                weapon.name(),
                values.recycle_seconds,
                actor.0
            ),
        )?;
    }
    *scripts.world_mut() = candidate;
    Ok(values)
}

/// Change a skill threshold with canonical naming and atomic audit admission.
pub fn edit_skill_threshold(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    name: &str,
    threshold: i64,
) -> Result<()> {
    let mut candidate = scripts.world().clone();
    super::set_skill_threshold(&mut candidate, actor, name, threshold)?;
    let skill = super::skill_definition(name).expect("validated skill name");
    audit(
        scripts,
        config,
        format!(
            "Exp threshold for {} changed to {} by #{}",
            skill.name, threshold, actor.0
        ),
    )?;
    *scripts.world_mut() = candidate;
    Ok(())
}
