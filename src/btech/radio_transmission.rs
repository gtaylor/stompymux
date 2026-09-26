//! Shared native/Lua radio admission, delivery publication and command-mine transaction.
use super::*;
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Delivery modes retain their distinct interference and relay diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", content = "report", rename_all = "snake_case")]
pub enum BattleRadioDelivery {
    Analog(BattleAnalogRadioReport),
    Digital(BattleDigitalRadioReport),
}

/// One accepted transmission, including the subsequent frequency-matched mine phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleRadioTransmission {
    pub delivery: BattleRadioDelivery,
    pub mines: BattleCommandMineReport,
    /// Administrative diagnostics captured before delivery and committed with the transmission.
    pub audit_messages: Vec<BattleChannelMessage>,
    /// Accepted communication XP diagnostics committed with delivery.
    pub experience_messages: Vec<BattleChannelMessage>,
}

/// Send from a conscious assigned cockpit pilot, publishing delivery before mine consequences.
/// Shutdown radios remain usable. Stun, destroyed hardware, invalid channels and malformed
/// messages are rejected before expenditure. A late mine/publication error restores all state
/// and staged notifications, including analog dice already consumed by reception.
pub fn send_radio_action(
    scripts: &Scripts,
    config: &Config,
    sender: ObjectId,
    pilot: ObjectId,
    channel: u8,
    message: &str,
) -> Result<BattleRadioTransmission> {
    let before = scripts.world.borrow().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let (digital, frequency) = {
            let world = scripts.world.borrow();
            super::radio::controlled(&world, sender, pilot)?;
            let unit = super::radio::unit(&world, sender)?;
            ensure!(
                !unit.is_destroyed(),
                "Your communication gear is inoperative."
            );
            ensure!(!unit.stunned(), "You are too stunned to use the radio!");
            ensure!(
                !message.is_empty(),
                "Invalid format! Usage: sendchannel <letter>=<string>"
            );
            let selected = unit
                .radio_channels()
                .get(usize::from(channel))
                .context("Invalid channel-letter!")?;
            (selected.mode.digital, selected.frequency)
        };
        let audit_messages = super::radio_audit::transmission(
            &scripts.world.borrow(),
            sender,
            pilot,
            channel,
            frequency,
            message,
        )?;
        let (delivery, notices) = if digital {
            let report = resolve_digital_radio(&scripts.world.borrow(), sender, channel, message)?;
            let notices = report.notices();
            (BattleRadioDelivery::Digital(report), notices)
        } else {
            let report =
                resolve_analog_radio(&mut scripts.world.borrow_mut(), sender, channel, message)?;
            let notices = report.notices();
            (BattleRadioDelivery::Analog(report), notices)
        };
        super::channels::publish(scripts, config, &audit_messages)?;
        for notice in notices {
            super::notify_unit(scripts, notice)?;
        }
        let experience_messages = match &delivery {
            BattleRadioDelivery::Analog(report) => super::award_radio_experience(
                &mut scripts.world.borrow_mut(),
                report,
                crate::clock::wall_time(),
            )?,
            BattleRadioDelivery::Digital(_) => Vec::new(),
        };
        super::channels::publish(scripts, config, &experience_messages)?;
        let settings = &config.battletech;
        let rules = BattleFallRules {
            vehicle_impact: crate::BattleVehicleImpactRules::configured(settings, false),
            stacking: BattleStackingRules {
                mode: settings.stacking,
                damage_percent: settings.stackdamage,
                hit_arcs: settings.hit_arcs,
            },
            stagger: BattleStaggerMode::from_setting(settings.newstagger),
            hit: BattleHitRules {
                inferno_penalty: settings.inferno_penalty != 0,
                exile_stun_mode: settings.exile_stun_code.clamp(0, 2) as u8,
            },
            extended_piloting: settings.extended_piloting != 0,
            toughness: false,
        };
        let mines = super::evacuation::detonate_command_mines_action(
            scripts,
            config,
            sender,
            frequency as i32,
            rules,
        )?;
        Ok(BattleRadioTransmission {
            delivery,
            mines,
            audit_messages,
            experience_messages,
        })
    })();
    if result.is_err() {
        *scripts.world.borrow_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Letter-based cockpit input delegates to the same host transaction as Lua.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let (channel, message) = super::radio::selection(&input.args)?;
        let sender = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        send_radio_action(
            ctx.scripts,
            ctx.config,
            sender,
            ctx.player,
            channel,
            message,
        )
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
