//! BattleTech diagnostics, captured into action reports and logged once the action commits.
use super::{GunneryAwardRequest, GunneryExperienceMode, ShotExperienceAward};
use crate::{
    Scripts, World,
    config::XpConfig,
    logging::{TraceRecord, TraceTopic},
};
use serde::Serialize;

/// An immutable diagnostic captured before damage or host callbacks can change participant names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticMessage {
    pub topic: TraceTopic,
    pub text: String,
}

impl DiagnosticMessage {
    /// Diagnostics are single-line even when a participant name contains a newline.
    pub(super) fn new(topic: TraceTopic, text: String) -> Self {
        Self {
            topic,
            text: text.replace('\n', " "),
        }
    }
}

/// Stock corrections and transfers share the same signed economy record.
pub(super) fn stock_message(
    actor: crate::ObjectId,
    holder: crate::ObjectId,
    name: &str,
    change: i32,
) -> DiagnosticMessage {
    DiagnosticMessage::new(
        TraceTopic::Economy,
        format!(
            "#{} {} {} {} {} #{}.",
            actor.0,
            if change > 0 { "added" } else { "removed" },
            change.unsigned_abs(),
            name,
            if change > 0 { "to" } else { "from" },
            holder.0,
        ),
    )
}

/// Format accepted awards and the battle-value formula's optional trivial-hit diagnostic.
pub(super) fn gunnery_messages(
    world: &World,
    request: GunneryAwardRequest,
    config: &XpConfig,
    attempt: Option<&ShotExperienceAward>,
) -> Vec<DiagnosticMessage> {
    let Some(attempt) = attempt else {
        if config.oldxpsystem == 0
            && config.bthmod != 0
            && config.noisy_xpgain != 0
            && request.damage > 0
            && request.base_to_hit < 3
            && super::gunnery_experience_eligible(
                world,
                request.attacker,
                request.pilot,
                request.target,
                request.base_to_hit,
                GunneryExperienceMode::BattleValue {
                    difficulty_modifier: false,
                },
            )
        {
            return vec![DiagnosticMessage::new(
                TraceTopic::Experience,
                format!(
                    "#{} in #{} 1 noxp #{}",
                    request.pilot.0, request.attacker.0, request.target.0
                ),
            )];
        }
        return Vec::new();
    };
    if !attempt.award().is_some_and(|award| award.accepted) {
        return Vec::new();
    }
    let pilot = &world.objects[&request.pilot].name;
    let target = &world.objects[&request.target].name;
    let message = match attempt {
        ShotExperienceAward::Classic(report) => format!(
            "{pilot} gained {} gun XP from feat of {:.6} % difficulty ({} occurences) against {target}",
            report.amount.unwrap(),
            report.chance.difficulty,
            request.damage
        ),
        ShotExperienceAward::BattleValue(report) => {
            let difficulty = report
                .calculation
                .difficulty
                .map_or_else(|| "undefined".into(), |value| format!("{value:.6}"));
            format!(
                "{pilot} gained {} gun XP from feat of {difficulty}/100 difficulty ({} damage) against {target}",
                report.amount, request.damage
            )
        }
    };
    let mut messages = vec![DiagnosticMessage::new(
        TraceTopic::GunneryExperience,
        message,
    )];
    if matches!(attempt, ShotExperienceAward::BattleValue(_)) && config.noisy_xpgain != 0 {
        messages.push(DiagnosticMessage::new(
            TraceTopic::Experience,
            format!(
                "#{} in #{} {} damage #{}",
                request.pilot.0, request.attacker.0, request.damage, request.target.0
            ),
        ));
    }
    messages
}

/// Stage a shot's diagnostics under its enclosing world/effects checkpoint.
pub(super) fn publish_shot(scripts: &Scripts, report: &super::MechShotReport) {
    publish(scripts, &report.experience_messages);
}

/// Stage captured diagnostics for logging once the enclosing host action commits; a
/// rolled-back action discards them with its other effects.
pub(super) fn publish(scripts: &Scripts, messages: &[DiagnosticMessage]) {
    for message in messages {
        scripts.effects.stage_trace(TraceRecord {
            topic: message.topic,
            message: message.text.clone(),
        });
    }
}
