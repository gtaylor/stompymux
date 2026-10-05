//! Conventional piloting checks, independent of the caller's fall or movement consequences.
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// All control checks share power, unit-owned recovery and assigned-pilot recovery gates.
/// Callers retain their distinct automatic-success rules and destruction policy.
pub(super) fn controls_blocked(world: &World, unit: ObjectId, power: super::Power) -> bool {
    power != super::Power::Running || super::crew::unit_unconscious(world, unit)
}

/// A control check; prone units succeed automatically, otherwise stopped or unconscious crew fail without dice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[must_use = "Apply the failed check's movement or fall consequences in the enclosing action"]
pub struct PilotingCheck {
    pub skill: i16,
    pub damage: u8,
    /// Construction penalty from a small cockpit, separate from damage.
    pub cockpit: u8,
    /// Hardened armor adds one to Mech piloting and vehicle driving rolls.
    pub armor: u8,
    pub situational: i32,
    pub absent_character_pilot: u8,
    pub target: i32,
    pub roll: Option<u8>,
    pub success: bool,
    /// Skill mutation for callers that apply the successful-check XP policy.
    pub experience: Option<super::ExperienceAward>,
}

impl PilotingCheck {
    /// Capture the control subtotal for diagnostic subscribers; skipped rolls remain silent.
    /// Awarding checks use the `(noxp)` diagnostic label.
    pub(super) fn diagnostic(&self, awards_experience: bool) -> Option<super::DiagnosticMessage> {
        self.roll.map(|_| super::DiagnosticMessage::new(
            super::DiagnosticChannel::Debug,
            format!("Attempting to make pilot{} skill roll. SPilot: {}, mods: {}, MechPilot: {}, BTH: {}",
                if awards_experience { " (noxp)" } else { "" },
                self.skill, self.situational, self.damage, self.target),
        ))
    }

    /// Feedback for an actual roll; automatic and blocked checks remain silent.
    pub fn messages(&self) -> Option<[String; 2]> {
        self.roll.map(|roll| roll_messages(self.target, roll))
    }
}

/// Shared reference wording for ordinary control checks and orbital landing checks.
pub(super) fn roll_messages(target: i32, roll: u8) -> [String; 2] {
    [
        "You make a piloting skill roll!".into(),
        format!("Modified Pilot Skill: BTH {target}\tRoll: {roll}"),
    ]
}

/// Private feedback interleaved with an action's ordered cockpit and observer notices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PilotNotice {
    /// Index of the next ordinary notice; its vector length means after the final notice.
    pub before_notice: usize,
    pub pilot: ObjectId,
    pub text: String,
}

/// Capture the roll audience before consequences can change crew assignment or containment.
pub(super) fn capture_feedback(
    unit: ObjectId,
    pilot: Option<ObjectId>,
    check: &PilotingCheck,
    notices: &mut Vec<super::Notice>,
    private: &mut Vec<PilotNotice>,
) {
    let Some(messages) = check.messages() else {
        return;
    };
    for text in messages {
        if let Some(pilot) = pilot {
            private.push(PilotNotice {
                before_notice: notices.len(),
                pilot,
                text,
            });
        } else {
            notices.push(super::Notice { unit, text });
        }
    }
}

/// Retain insertion positions when a nested report joins an existing notice stream.
pub(super) fn append_feedback(
    destination: &mut Vec<PilotNotice>,
    incoming: impl IntoIterator<Item = PilotNotice>,
    offset: usize,
) {
    destination.extend(incoming.into_iter().map(|mut notice| {
        notice.before_notice += offset;
        notice
    }));
}

/// Publish captured pilot feedback at its original position without expanding its audience.
pub(super) fn publish_ordered_notices(
    scripts: &crate::Scripts,
    notices: &[super::Notice],
    private: &[PilotNotice],
) -> Result<()> {
    publish_interleaved(scripts, notices, private, |_| Ok(()))
}

/// Publish an admitted maneuver's initial diagnostic at the captured roll-feedback position.
/// These maneuver callers always capture their assigned pilot before applying consequences.
pub(super) fn publish_maneuver_feedback(
    scripts: &crate::Scripts,
    config: &crate::Config,
    notices: &[super::Notice],
    private: &[PilotNotice],
    check: Option<&PilotingCheck>,
    awards_experience: bool,
) -> Result<()> {
    let position = private.first().map_or(0, |notice| notice.before_notice);
    publish_diagnostic_feedback(
        scripts,
        config,
        notices,
        private,
        check
            .and_then(|check| check.diagnostic(awards_experience))
            .map(|message| (position, message)),
    )
}

/// Reports with cockpit fallback retain an explicit diagnostic position independent of pilot presence.
pub(super) fn publish_diagnostic_feedback(
    scripts: &crate::Scripts,
    config: &crate::Config,
    notices: &[super::Notice],
    private: &[PilotNotice],
    diagnostic: Option<(usize, super::DiagnosticMessage)>,
) -> Result<()> {
    publish_interleaved(scripts, notices, private, |index| {
        if let Some((position, diagnostic)) = &diagnostic
            && index == *position
        {
            super::channels::publish(scripts, config, std::slice::from_ref(diagnostic))?;
        }
        Ok(())
    })
}

/// Retain one notification-order implementation for ordinary and diagnostic-bearing reports.
fn publish_interleaved(
    scripts: &crate::Scripts,
    notices: &[super::Notice],
    private: &[PilotNotice],
    mut before_feedback: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    let mut private = private.iter().peekable();
    for index in 0..=notices.len() {
        before_feedback(index)?;
        while private
            .peek()
            .is_some_and(|notice| notice.before_notice == index)
        {
            let notice = private.next().unwrap();
            super::notify_message(
                scripts,
                super::MessageTarget::Player(notice.pilot),
                &notice.text,
            )?;
        }
        if let Some(notice) = notices.get(index) {
            super::notify_unit(scripts, notice.clone())?;
        }
    }
    Ok(())
}

/// Roll a Mech or vehicle control check without XP awards or notifications; prone Mechs succeed without rolling.
/// Missing/disconnected pilots use skill six; shutdown, blindness and either recovery owner fail without consuming dice.
/// All validation occurs before RNG is published. The caller owns fall effects and the world commit.
pub fn roll_piloting(
    world: &mut World,
    unit: ObjectId,
    modifier: i16,
    extended: bool,
) -> Result<PilotingCheck> {
    roll_piloting_i32(world, unit, i32::from(modifier), extended)
}

/// C contract entry point whose public modifier is a full signed `int`.
pub(super) fn roll_piloting_i32(
    world: &mut World,
    unit: ObjectId,
    modifier: i32,
    extended: bool,
) -> Result<PilotingCheck> {
    if world.btech.vehicles().contains_key(&unit) {
        return super::vehicle_piloting::roll(world, unit, modifier, extended);
    }
    roll_check(world, unit, modifier, extended, false)
}

/// Standing can succeed without dice when the chassis has intact automatic support.
pub(super) fn roll_standing(
    world: &mut World,
    unit: ObjectId,
    modifier: i16,
    extended: bool,
) -> Result<PilotingCheck> {
    let automatic = !world
        .btech
        .constructed_units()
        .get(&unit)
        .context("Unit construction state is unavailable")?
        .stand_requires_roll()?;
    roll_check(world, unit, i32::from(modifier), extended, automatic)
}

/// Shared check evaluation; automatic success still respects blocked crew except the prone rule.
fn roll_check(
    world: &mut World,
    unit: ObjectId,
    modifier: i32,
    extended: bool,
    automatic: bool,
) -> Result<PilotingCheck> {
    let object = world.objects.get(&unit).context("Unit is unavailable")?;
    ensure!(!object.flags.contains(Flag::Going), "Unit is unavailable");
    let state = world
        .btech
        .constructed_units()
        .get(&unit)
        .context("Unit construction state is unavailable")?;
    state.validate()?;
    let skill = super::skills::control_target(world, unit, extended)?;
    let damage = state.mobility().piloting_modifier;
    let cockpit = state.cockpit_piloting_modifier();
    let armor = state.hardened_piloting_modifier();
    let absent_character_pilot = if object.flags.contains(Flag::InCharacter)
        && state
            .pilot()
            .and_then(|pilot| world.objects.get(&pilot))
            .is_none_or(|pilot| pilot.location != Some(unit))
    {
        5
    } else {
        0
    };
    let target = i32::from(skill)
        .wrapping_add(i32::from(damage))
        .wrapping_add(i32::from(cockpit))
        .wrapping_add(i32::from(armor))
        .wrapping_add(modifier)
        .wrapping_add(i32::from(absent_character_pilot));
    let blocked = controls_blocked(world, unit, state.power());
    let prone = state.posture() == super::Posture::Prone;
    let automatic_success = prone || (automatic && !blocked);
    let roll = if blocked || automatic_success {
        None
    } else {
        Some(
            world
                .btech
                .constructed
                .get_mut(&unit)
                .unwrap()
                .dice
                .generic_roll(),
        )
    };
    Ok(PilotingCheck {
        skill,
        damage,
        cockpit,
        armor,
        situational: modifier,
        absent_character_pilot,
        target,
        roll,
        success: automatic_success || roll.is_some_and(|roll| i32::from(roll) >= target),
        experience: None,
    })
}

impl super::Mech {
    /// Construction-only control penalty; it does not count as mobility damage.
    pub fn cockpit_piloting_modifier(&self) -> u8 {
        u8::from(
            self.definition()
                .has_technology(super::Technology::SmallCockpit),
        )
    }
}

/// Potential XP for an actual successful roll; automatic successes and trivial targets earn none.
fn experience_amount(check: &PilotingCheck) -> Option<u32> {
    if !check.success || check.roll.is_none() || check.target <= 2 {
        return None;
    }
    Some((check.target - 7).clamp(1, (1 + check.situational).max(2)) as u32)
}

/// Apply successful-check XP within the caller's candidate and snapshot its diagnostic.
/// This does not consume dice or modify the movement XP coordinate mark.
pub(super) fn award_control_check(
    world: &mut World,
    id: ObjectId,
    check: &mut PilotingCheck,
    extended: bool,
) -> Result<Option<super::DiagnosticMessage>> {
    let Some(amount) = experience_amount(check) else {
        return Ok(None);
    };
    let (award, message) = award_reason(world, id, amount, extended)?;
    check.experience = award;
    Ok(message)
}

/// Award an explicit successful-control reason using the shared active-crew and skill policy.
/// This bypasses movement-coordinate gating, as do ordinary successful control checks.
pub(super) fn award_reason(
    world: &mut World,
    id: ObjectId,
    amount: u32,
    extended: bool,
) -> Result<(
    Option<super::ExperienceAward>,
    Option<super::DiagnosticMessage>,
)> {
    if world.objects.get(&id).is_none_or(|unit| {
        !unit.flags.contains(Flag::InCharacter) || unit.flags.contains(Flag::Going)
    }) {
        return Ok((None, None));
    }
    let unit = world.btech.unit(id).expect("in-character unit");
    let (pilot, skill) = (unit.pilot(), unit.piloting_skill(extended));
    let (Some(pilot), Some(skill)) = (pilot, skill) else {
        return Ok((None, None));
    };
    if world.objects.get(&pilot).is_none_or(|pilot| {
        pilot.location != Some(id)
            || !pilot.flags.contains(Flag::Connected)
            || pilot.flags.contains(Flag::Going)
    }) {
        return Ok((None, None));
    }
    let award = super::award_skill_experience(
        world,
        pilot,
        skill,
        amount,
        crate::clock::wall_time(),
        false,
    )?;
    let message = award.accepted.then(|| {
        super::DiagnosticMessage::new(
            super::DiagnosticChannel::PilotingExperience,
            format!("{} gained {amount} {skill} XP", world.objects[&pilot].name),
        )
    });
    Ok((Some(award), message))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Diagnostics expose the subtotal inputs, retain distinct labels, and omit skipped checks.
    #[test]
    fn control_diagnostics_follow_roll_and_award_policy() {
        let mut check = PilotingCheck {
            skill: 6,
            damage: 2,
            cockpit: 1,
            armor: 0,
            situational: -1,
            absent_character_pilot: 5,
            target: 13,
            roll: Some(8),
            success: false,
            experience: None,
        };
        for (awards, label) in [(false, ""), (true, " (noxp)")] {
            let message = check.diagnostic(awards).unwrap();
            assert_eq!(message.channel, super::super::DiagnosticChannel::Debug);
            assert_eq!(
                message.text,
                format!(
                    "Attempting to make pilot{label} skill roll. SPilot: 6, mods: -1, MechPilot: 2, BTH: 13"
                )
            );
        }
        check.roll = None;
        assert!(check.diagnostic(true).is_none());
        check.success = true;
        assert!(check.diagnostic(false).is_none());
    }

    /// Automatic support skips dice and XP but cannot start a stopped unit.
    #[test]
    fn automatic_support_preserves_dice_and_respects_power() {
        let config = crate::Config::load("tests/fixtures/game").unwrap();
        let mut world = World::default();
        let id = world.create(&config, "Mech".into(), crate::Kind::Thing);
        let template = crate::MechTemplate::parse(
            "JR7-D",
            include_str!("../../tests/fixtures/btech/units/JR7-D.toml"),
        )
        .unwrap();
        let mut unit = crate::Mech::from_template(template).unwrap();
        unit.position = Some(crate::Position {
            map: ObjectId(99),
            x: 0,
            y: 0,
        });
        unit.map_slot = Some(0);
        unit.power = super::super::Power::Running;
        world.btech.constructed.insert(id, unit);
        let before = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
        let check = roll_check(&mut world, id, 20, false, true).unwrap();
        assert!(check.success);
        assert_eq!(check.roll, None);
        assert_eq!(experience_amount(&check), None);
        assert!(check.messages().is_none());
        assert_eq!(
            world.btech.constructed_units()[&id]
                .dice
                .generic_roll_statistics()
                .total(),
            0
        );
        assert_eq!(
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap(),
            before
        );
        world.btech.constructed.get_mut(&id).unwrap().power = super::super::Power::Off;
        let check = roll_check(&mut world, id, 0, false, true).unwrap();
        assert!(!check.success);
        assert_eq!(check.roll, None);
        assert!(check.messages().is_none());
        assert_eq!(
            world.btech.constructed_units()[&id]
                .dice
                .generic_roll_statistics()
                .total(),
            0
        );
        world.btech.constructed.get_mut(&id).unwrap().power = super::super::Power::Running;
        let mut expected = world.btech.constructed_units()[&id].dice.clone();
        let expected_roll = expected.two_d6();
        let check = roll_check(&mut world, id, 0, false, false).unwrap();
        assert_eq!(check.roll, Some(expected_roll));
        let dice = &world.btech.constructed_units()[&id].dice;
        assert_eq!(dice, &expected);
        assert_eq!(dice.generic_roll_statistics().total(), 1);
        assert_eq!(
            dice.generic_roll_statistics().counts()[usize::from(expected_roll - 2)],
            1
        );
    }

    /// The situational cap is independent of skill/damage; prone and trivial checks never award XP.
    #[test]
    fn control_experience_uses_success_target_and_modifier() {
        let mut check = PilotingCheck {
            skill: 6,
            damage: 0,
            cockpit: 0,
            armor: 0,
            situational: 0,
            absent_character_pilot: 0,
            target: 6,
            roll: Some(12),
            success: true,
            experience: None,
        };
        for (target, modifier, expected) in [
            (2, 0, None),
            (3, 0, Some(1)),
            (8, 0, Some(1)),
            (9, 0, Some(2)),
            (12, 0, Some(2)),
            (12, 4, Some(5)),
            (12, -2, Some(2)),
            (10, 4, Some(3)),
        ] {
            check.target = target;
            check.situational = modifier;
            assert_eq!(experience_amount(&check), expected);
        }
        check.success = false;
        assert_eq!(experience_amount(&check), None);
        check.success = true;
        check.roll = None;
        assert_eq!(experience_amount(&check), None);
    }
}
