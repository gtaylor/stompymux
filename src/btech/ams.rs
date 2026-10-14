//! Anti-missile systems (Total Warfare pp. 129-130), with expenditure owned by the enclosing shot.
//!
//! An AMS whose firing arc covers the incoming attack may engage one missile attack per turn
//! (the mount's recycle). Engaging applies -4 to the attack's Cluster Hits roll, never below 2;
//! a Streak launcher is treated as a roll of 11 before the -4; a single-missile attack is
//! destroyed on a 1d6 roll of 1-3. Each engagement spends one round of ammunition (laser AMS
//! none) and the system's heat. The `ams` switch is the defender's standing choice to engage.
use super::{Mech, Notice, Power, Weapon};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use serde::Serialize;

/// One engagement against an admitted missile hit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AmsReport {
    pub weapon_index: usize,
    /// Bin that fed the defense; laser AMS has none.
    pub ammunition_bin: Option<usize>,
    /// The 1d6 rolled against a single-missile attack (1-3 destroys it); a cluster engagement
    /// rolls no die and shifts the Cluster Hits roll by -4 instead.
    pub roll: Option<u8>,
    /// One round per engagement when the system uses ammunition.
    pub ammunition_spent: u16,
    /// Missiles the engagement kept off the target: the single missile when destroyed, otherwise
    /// the hits the -4 removed from the Cluster Hits result. Filled in after cluster resolution.
    pub shot_down: u8,
}

impl AmsReport {
    /// Whether a single-missile attack was destroyed before reaching the target.
    pub fn destroyed_missile(&self) -> bool {
        self.roll.is_some_and(|roll| roll <= 3)
    }
}

impl Mech {
    /// Pilot-selected automatic defense state, initially disabled.
    pub fn ams_enabled(&self) -> bool {
        self.ams_enabled
    }
}

impl super::Vehicle {
    /// Pilot-selected automatic defense state, initially disabled and saved with the vehicle.
    pub fn ams_enabled(&self) -> bool {
        self.ams_enabled
    }
}

/// Set the whole unit's defense state after ordinary cockpit and power checks.
pub fn set_ams(world: &mut World, id: ObjectId, pilot: ObjectId, enabled: bool) -> Result<()> {
    super::targeting::controlled(world, id, pilot)?;
    ensure!(
        super::ams_unit::available(world, id)?,
        "This mech is not equipped with AMS"
    );
    super::ams_unit::set_enabled(world, id, enabled)
}

/// Engage an admitted missile hit with the first ready mount whose arc covers the attacker,
/// fed by its first usable normal-ammunition bin. The attacker owns the single-missile die.
pub(super) fn intercept(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    weapon: Weapon,
) -> Result<Option<AmsReport>> {
    if weapon.profile().missiles == 0
        || !super::ams_unit::enabled(world, target)?
        || super::scanner::scanner_unit(world, target)
            .is_none_or(|unit| unit.power != Power::Running)
        || !super::ams_unit::available(world, target)?
    {
        return Ok(None);
    }
    let Some(super::ams_unit::Defense {
        index,
        weapon: system,
        bin,
    }) = super::ams_unit::select(world, target, attacker)?
    else {
        return Ok(None);
    };
    let roll = (weapon.profile().missiles == 1)
        .then(|| super::dice::unit_dice_mut(world, attacker).map(|dice| dice.d6()))
        .transpose()?;
    let spent = super::ams_unit::expend(world, target, index, system, bin)?;
    Ok(Some(AmsReport {
        weapon_index: index,
        ammunition_bin: bin,
        roll,
        ammunition_spent: spent,
        shot_down: 0,
    }))
}

impl AmsReport {
    /// Defense feedback after cluster resolution determines how many hits the engagement removed.
    /// `hits` is the cluster result before the engagement; None for a single missile.
    pub(super) fn notices(
        &self,
        attacker: ObjectId,
        target: ObjectId,
        hits: Option<u8>,
    ) -> Vec<Notice> {
        if self.roll.is_some() {
            return if self.destroyed_missile() {
                vec![
                    Notice {
                        unit: attacker,
                        text: "Your missile is shot down by the target!".into(),
                    },
                    Notice {
                        unit: target,
                        text:
                            "Your Anti-Missile System activates and destroys the incoming missile!"
                                .into(),
                    },
                ]
            } else {
                vec![
                    Notice {
                        unit: attacker,
                        text: "The target's Anti-Missile System fails to stop your missile!".into(),
                    },
                    Notice {
                        unit: target,
                        text: "Your Anti-Missile System activates but the incoming missile gets through!"
                            .into(),
                    },
                ]
            };
        }
        let Some(hits) = hits else {
            return vec![Notice {
                unit: target,
                text: "Your Anti-Missile System activates and shoots at the incoming missiles!"
                    .into(),
            }];
        };
        let all = self.shot_down >= hits;
        vec![
            Notice {
                unit: attacker,
                text: if all {
                    "All of your missiles are shot down by the target!".into()
                } else {
                    format!(
                        "The target shoots down {} of your missiles!",
                        self.shot_down
                    )
                },
            },
            Notice {
                unit: target,
                text: if all {
                    "Your Anti-Missile System activates and shoots all the incoming missiles!"
                        .into()
                } else {
                    format!(
                        "Your Anti-Missile System activates and shoots down {} incoming missiles!",
                        self.shot_down
                    )
                },
            },
        ]
    }
}

/// Shared native/Lua switch publication, including rollback when cockpit delivery fails.
pub(crate) fn configure(
    scripts: &crate::Scripts,
    id: ObjectId,
    pilot: ObjectId,
    enabled: Option<bool>,
) -> Result<bool> {
    scripts.atomic(|before| {
        super::targeting::controlled(before, id, pilot)?;
        let enabled = enabled.unwrap_or(!super::ams_unit::enabled(before, id)?);
        set_ams(&mut scripts.world.borrow_mut(), id, pilot, enabled)?;
        super::notify_unit_text(
            scripts,
            id,
            if enabled {
                "Anti-Missile System turned ON"
            } else {
                "Anti-Missile System turned OFF"
            },
        )?;
        Ok(enabled)
    })
}

/// The cockpit command toggles the whole defense system; the optional weapon argument is ignored.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let id = ctx
        .scripts
        .world
        .borrow()
        .objects
        .get(&ctx.player)
        .and_then(|p| p.location);
    let result = id
        .ok_or_else(|| anyhow::anyhow!("Enter a unit first"))
        .and_then(|id| configure(ctx.scripts, id, ctx.player, None));
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a single-missile engagement's die decides destruction; cluster engagements shift the
    /// Cluster Hits roll and report the hits removed instead.
    #[test]
    fn single_missile_destruction_follows_the_die() {
        for roll in 1..=6 {
            let report = AmsReport {
                weapon_index: 0,
                ammunition_bin: None,
                roll: Some(roll),
                ammunition_spent: 0,
                shot_down: 0,
            };
            assert_eq!(report.destroyed_missile(), roll <= 3);
            let notices = report.notices(ObjectId(1), ObjectId(2), None);
            assert_eq!(notices.len(), 2);
            assert_eq!(
                notices[0].text.contains("shot down"),
                report.destroyed_missile()
            );
        }
        let cluster = AmsReport {
            weapon_index: 0,
            ammunition_bin: Some(0),
            roll: None,
            ammunition_spent: 1,
            shot_down: 2,
        };
        assert!(!cluster.destroyed_missile());
        let notices = cluster.notices(ObjectId(1), ObjectId(2), Some(5));
        assert!(notices[0].text.contains("shoots down 2"));
        let all = AmsReport {
            shot_down: 5,
            ..cluster
        };
        assert!(
            all.notices(ObjectId(1), ObjectId(2), Some(5))[0]
                .text
                .starts_with("All of your missiles")
        );
    }
}
