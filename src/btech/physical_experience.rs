//! Eligibility and piloting experience for direct biped physical damage packets.
use crate::{Flag, ObjectId, World};
use anyhow::Result;

/// Skill mutation and the accepted award's pre-impact diagnostic snapshot.
pub(super) struct PhysicalExperience {
    pub award: super::ExperienceAward,
    pub message: Option<super::DiagnosticMessage>,
}

/// Award before damage so lethal hits remain eligible and failed cascades restore the award.
pub(super) fn award(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    damage: u16,
    now: i64,
    extended_piloting: bool,
) -> Result<Option<PhysicalExperience>> {
    if damage == 0 || attacker == target {
        return Ok(None);
    }
    for id in [attacker, target] {
        if world.objects.get(&id).is_none_or(|object| {
            !object.flags.contains(Flag::InCharacter) || object.flags.contains(Flag::Going)
        }) {
            return Ok(None);
        }
    }
    let source = &world.btech.constructed_units()[&attacker];
    let victim = &world.btech.constructed_units()[&target];
    if victim.is_destroyed()
        || source.signature().team == victim.signature().team
        || source.pilot() != Some(pilot)
        || world.objects.get(&pilot).is_none_or(|object| {
            object.location != Some(attacker)
                || !object.flags.contains(Flag::Connected)
                || object.flags.contains(Flag::Going)
        })
    {
        return Ok(None);
    }
    let skill = source.chassis().piloting_skill(extended_piloting);
    let amount = u32::from(damage / 3).max(1);
    let award = super::award_skill_experience(world, pilot, skill, amount, now, false)?;
    let message = award.accepted.then(|| {
        super::DiagnosticMessage::new(
            super::DiagnosticTopic::PilotingExperience,
            format!("{} gained {amount} {skill} XP", world.objects[&pilot].name),
        )
    });
    Ok(Some(PhysicalExperience { award, message }))
}
