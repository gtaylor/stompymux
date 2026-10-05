//! BattleTech diagnostics: debug traces staged until commit, and the few topics still
//! published through the ordinary transactional channel service.
use super::{GunneryAwardRequest, GunneryExperienceMode, ShotExperienceAward};
use crate::{
    Config, Scripts, World,
    config::XpConfig,
    logging::{TraceRecord, TraceTopic},
};
use anyhow::Result;
use serde::Serialize;

/// What a captured diagnostic is about, which also decides where it is delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticTopic {
    MapErrors,
    PilotingRolls,
    SelfDestruct,
    Economy,
    GunneryExperience,
    Experience,
    PilotingExperience,
    RadioFrequencies,
    ZeroFrequencies,
}

impl DiagnosticTopic {
    /// The channel that receives this topic, for topics players and staff subscribe to.
    pub fn channel(self) -> Option<&'static str> {
        match self {
            Self::MapErrors => Some("MapErrors"),
            Self::ZeroFrequencies => Some("ZeroFrequencies"),
            _ => None,
        }
    }

    /// The debug trace topic, for topics written to the server log.
    pub fn trace(self) -> Option<TraceTopic> {
        match self {
            Self::MapErrors | Self::ZeroFrequencies => None,
            Self::PilotingRolls => Some(TraceTopic::PilotingRolls),
            Self::SelfDestruct => Some(TraceTopic::SelfDestruct),
            Self::Economy => Some(TraceTopic::Economy),
            Self::GunneryExperience => Some(TraceTopic::GunneryExperience),
            Self::Experience => Some(TraceTopic::Experience),
            Self::PilotingExperience => Some(TraceTopic::PilotingExperience),
            Self::RadioFrequencies => Some(TraceTopic::RadioFrequencies),
        }
    }
}

/// An immutable diagnostic captured before damage or host callbacks can change participant names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticMessage {
    pub topic: DiagnosticTopic,
    pub text: String,
}

impl DiagnosticMessage {
    /// Diagnostics are single-line even when a participant name contains a newline.
    pub(super) fn new(topic: DiagnosticTopic, text: String) -> Self {
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
        DiagnosticTopic::Economy,
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
                DiagnosticTopic::Experience,
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
        DiagnosticTopic::GunneryExperience,
        message,
    )];
    if matches!(attempt, ShotExperienceAward::BattleValue(_)) && config.noisy_xpgain != 0 {
        messages.push(DiagnosticMessage::new(
            DiagnosticTopic::Experience,
            format!(
                "#{} in #{} {} damage #{}",
                request.pilot.0, request.attacker.0, request.damage, request.target.0
            ),
        ));
    }
    messages
}

/// Deliver a shot's diagnostics under its enclosing world/effects checkpoint.
pub(super) fn publish_shot(
    scripts: &Scripts,
    config: &Config,
    report: &super::MechShotReport,
) -> Result<()> {
    publish(scripts, config, &report.experience_messages)
}

/// Shared delivery for captured diagnostics; the enclosing host action owns rollback.
/// Debug traces wait for commit, and channel topics skip channels that do not exist.
pub(super) fn publish(
    scripts: &Scripts,
    config: &Config,
    messages: &[DiagnosticMessage],
) -> Result<()> {
    let mut service = None;
    for message in messages {
        if let Some(topic) = message.topic.trace() {
            scripts.effects.stage_trace(TraceRecord {
                topic,
                message: message.text.clone(),
            });
            continue;
        }
        let Some(channel) = message.topic.channel() else {
            continue;
        };
        let service = service.get_or_insert_with(|| scripts.communication(config));
        if service.name(channel).is_err() {
            continue;
        }
        service.emit(channel, &message.text, false)?;
    }
    Ok(())
}
