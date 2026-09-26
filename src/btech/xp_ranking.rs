//! Read-only skill XP rankings use the same saved balance representation as progression.
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// One ranked player and their share of all counted balances.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleXpRank {
    pub player: ObjectId,
    pub name: String,
    pub experience: u32,
    pub percentage: f64,
}

/// Bounded leaderboard; the total includes counted players outside the displayed top sixteen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleXpRanking {
    pub skill: String,
    pub counted: usize,
    pub total: u64,
    pub entries: Vec<BattleXpRank>,
    pub text: String,
}

/// Publish a wizard's leaderboard without awarding XP or changing character state.
pub fn xp_ranking_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    skill: &str,
) -> Result<BattleXpRanking> {
    scripts.atomic(|_| {
        let world = scripts.world();
        ensure!(
            crate::authority::is_wizard(&world, actor),
            "Permission denied."
        );
        let skill = super::skill_definition(skill.trim()).context("Unknown skill")?;
        let mut entries = Vec::new();
        for (&id, object) in &world.objects {
            if object.kind != crate::Kind::Player || crate::authority::is_wizard(&world, id) {
                continue;
            }
            let value = world
                .btech
                .character_values()
                .get(&id)
                .and_then(|values| values.get(skill.name))
                .copied()
                .unwrap_or_default();
            if value.experience == 0 {
                continue;
            }
            entries.push(BattleXpRank {
                player: id,
                name: object.name.clone(),
                experience: value.experience_balance(),
                percentage: 0.0,
            });
            if entries.len() == 10_000 {
                break;
            }
        }
        let counted = entries.len();
        let total = entries
            .iter()
            .map(|entry| u64::from(entry.experience))
            .sum::<u64>();
        // A later strictly greater balance takes the current rank immediately.
        // Equal balances never swap directly, but displaced entries retain that exchange order.
        for rank in 0..entries.len() {
            for candidate in rank + 1..entries.len() {
                if entries[candidate].experience > entries[rank].experience {
                    entries.swap(rank, candidate);
                }
            }
        }
        entries.truncate(16);
        for entry in &mut entries {
            entry.percentage = if total == 0 {
                0.0
            } else {
                100.0 * f64::from(entry.experience) / total as f64
            };
        }
        let display = super::menu::pairs(
            entries.iter().enumerate().map(|(index, entry)| {
                (
                    crate::text::escape(&format!("{:3}. {}", index + 1, entry.name)),
                    format!("{} ({:.3} %)", entry.experience, entry.percentage),
                )
            }),
            (total > 0).then(|| format!("Grand total: {total} points")),
        );
        let report = BattleXpRanking {
            skill: skill.name.into(),
            counted,
            total,
            entries,
            text: crate::text::plain(&display),
        };
        drop(world);
        for line in display.lines() {
            super::notify_message(scripts, super::BattleMessageTarget::Player(actor), line)?;
        }
        scripts.world().validate_action(config)?;
        scripts.effects.validate()?;
        Ok(report)
    })
}

/// XPTOP takes one complete skill name or catalogue alias.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let name = input
            .args
            .trim_start_matches(|character: char| character.is_ascii_whitespace());
        ensure!(!name.is_empty(), "Invalid argument!");
        let name = super::character_names::resolve(name).context("Invalid value name!")?;
        let skill =
            super::skill_definition(name).context("Only skills have XP (for now at least)")?;
        xp_ranking_action(ctx.scripts, ctx.config, ctx.player, skill.name)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => crate::CommandAction::Report(crate::CommandReport::Reply(error.to_string())),
    })
}
