//! BattleTech diagnostic messages staged through the ordinary transactional channel service.
use super::{BattleGunneryAwardRequest, BattleGunneryExperienceMode, BattleShotExperienceAward};
use crate::{Config, Scripts, World, config::XpConfig};
use anyhow::Result;
use serde::Serialize;

/// Implemented diagnostic destinations; ordinary channel administration owns their existence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleChannel {
    MapErrors,
    Debug,
    Economy,
    AttackExperience,
    Experience,
    PilotingExperience,
    Frequencies,
    ZeroFrequencies,
}

impl BattleChannel {
    /// Canonical channel spelling used in diagnostic headers.
    pub fn name(self) -> &'static str {
        match self {
            Self::MapErrors => "MapErrors",
            Self::Debug => "MechDebugInfo",
            Self::Economy => "MechEconInfo",
            Self::AttackExperience => "MechAttackXP",
            Self::Experience => "MechXP",
            Self::PilotingExperience => "MechPilotXP",
            Self::Frequencies => "MechFreqs",
            Self::ZeroFrequencies => "ZeroFrequencies",
        }
    }
}

/// An immutable diagnostic captured before damage or host callbacks can change participant names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleChannelMessage {
    pub channel: BattleChannel,
    pub text: String,
}

impl BattleChannelMessage {
    /// Diagnostic channel records are single-line even when a participant name contains a newline.
    pub(super) fn new(channel: BattleChannel, text: String) -> Self {
        Self {
            channel,
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
) -> BattleChannelMessage {
    BattleChannelMessage::new(
        BattleChannel::Economy,
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
    request: BattleGunneryAwardRequest,
    config: &XpConfig,
    attempt: Option<&BattleShotExperienceAward>,
) -> Vec<BattleChannelMessage> {
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
                BattleGunneryExperienceMode::BattleValue {
                    difficulty_modifier: false,
                },
            )
        {
            return vec![BattleChannelMessage::new(
                BattleChannel::Experience,
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
        BattleShotExperienceAward::Classic(report) => format!(
            "{pilot} gained {} gun XP from feat of {:.6} % difficulty ({} occurences) against {target}",
            report.amount.unwrap(),
            report.chance.difficulty,
            request.damage
        ),
        BattleShotExperienceAward::BattleValue(report) => {
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
    let mut messages = vec![BattleChannelMessage::new(
        BattleChannel::AttackExperience,
        message,
    )];
    if matches!(attempt, BattleShotExperienceAward::BattleValue(_)) && config.noisy_xpgain != 0 {
        messages.push(BattleChannelMessage::new(
            BattleChannel::Experience,
            format!(
                "#{} in #{} {} damage #{}",
                request.pilot.0, request.attacker.0, request.damage, request.target.0
            ),
        ));
    }
    messages
}

/// Publish a shot's diagnostics under its enclosing world/effects checkpoint, ignoring absent channels.
pub(super) fn publish_shot(
    scripts: &Scripts,
    config: &Config,
    report: &super::BattleShotReport,
) -> Result<()> {
    publish(scripts, config, &report.experience_messages)
}

/// Shared delivery for captured combat diagnostics; the enclosing host action owns rollback.
pub(super) fn publish(
    scripts: &Scripts,
    config: &Config,
    messages: &[BattleChannelMessage],
) -> Result<()> {
    if messages.is_empty() {
        return Ok(());
    }
    let service = scripts.communication(config);
    for message in messages {
        if service.name(message.channel.name()).is_err() {
            continue;
        }
        service.emit(message.channel.name(), &message.text, false)?;
    }
    Ok(())
}
