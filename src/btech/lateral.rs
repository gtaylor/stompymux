//! Delayed BattleMech lateral movement controls and committed travel direction.
use crate::{ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Travel direction relative to the chassis, without changing weapon facing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LateralMode {
    #[default]
    None,
    FrontLeft,
    FrontRight,
    RearLeft,
    RearRight,
}

impl LateralMode {
    /// Compass offset used by movement, collisions and contact displays.
    pub fn offset(self) -> u16 {
        match self {
            Self::None => 0,
            Self::FrontLeft => 300,
            Self::FrontRight => 60,
            Self::RearLeft => 240,
            Self::RearRight => 120,
        }
    }

    /// Player-facing direction name.
    pub fn description(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::FrontLeft => "Front/Left",
            Self::FrontRight => "Front/Right",
            Self::RearLeft => "Rear/Left",
            Self::RearRight => "Rear/Right",
        }
    }

    /// Accept the directional aliases used by cockpit controls.
    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value.trim().to_ascii_lowercase().as_str() {
            "-" => Self::None,
            "nw" | "fl" => Self::FrontLeft,
            "ne" | "fr" => Self::FrontRight,
            "sw" | "rl" => Self::RearLeft,
            "se" | "rr" => Self::RearRight,
            _ => anyhow::bail!("Invalid mode!"),
        })
    }
}

/// Durable active direction and optional six-second changeover.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LateralState {
    pub active: LateralMode,
    pub pending: Option<LateralMode>,
    pub remaining: u8,
}

impl LateralState {
    /// Reject impossible countdowns before admitting saved unit state.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            self.remaining <= 6 && self.pending.is_some() == (self.remaining > 0),
            "Invalid lateral countdown"
        );
        ensure!(
            self.pending != Some(self.active),
            "Redundant lateral change"
        );
        Ok(())
    }
}

impl super::Mech {
    /// Current direction and pending transition, independent of chassis heading.
    pub fn lateral(&self) -> LateralState {
        self.lateral
    }

    /// Signed-speed travel axis; reverse movement follows the opposite direction.
    pub fn travel_heading(&self) -> Option<f64> {
        self.motion().map(|motion| {
            (motion.heading + f64::from(self.lateral.active.offset())).rem_euclid(360.0)
        })
    }
}

/// Only intact quads can select lateral movement.
pub fn set_lateral(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    mode: LateralMode,
) -> Result<super::Notice> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(
        unit.power() == super::Power::Running,
        "Start the unit first"
    );
    ensure!(
        unit.chassis() == super::MechChassis::Quad
            && unit
                .chassis()
                .legs()
                .iter()
                .all(|leg| !unit.leg_unavailable(*leg)),
        "You cannot alter your lateral movement!"
    );
    unit.position().context("Unit is not on a battlefield")?;
    let state = unit.lateral;
    let (next, text) = if mode == state.active {
        ensure!(state.pending.is_some(), "You are going that way already!");
        (
            LateralState {
                active: state.active,
                ..Default::default()
            },
            "Lateral mode change aborted.".into(),
        )
    } else {
        (
            LateralState {
                active: state.active,
                pending: Some(mode),
                remaining: 6,
            },
            format!(
                "Wanted lateral movement mode changed to {} ({} offset).",
                mode.description(),
                mode.offset()
            ),
        )
    };
    world.btech.constructed.get_mut(&id).unwrap().lateral = next;
    Ok(super::Notice { unit: id, text })
}

/// Advance queued changes once per simulation second; stopped units discard the request when its timer expires.
pub(super) fn advance(world: &mut World) -> Vec<super::Notice> {
    let mut notices = Vec::new();
    for (&id, unit) in &mut world.btech.constructed {
        let Some(mode) = unit.lateral.pending else {
            continue;
        };
        if world
            .objects
            .get(&id)
            .is_none_or(|object| object.flags.contains(crate::Flag::Going))
        {
            continue;
        }
        unit.lateral.remaining -= 1;
        if unit.lateral.remaining != 0 {
            continue;
        }
        unit.lateral.pending = None;
        if unit.power() != super::Power::Running {
            continue;
        }
        unit.lateral.active = mode;
        notices.push(super::Notice {
            unit: id,
            text: format!(
                "Lateral movement mode change to {} ({} offset) completed.",
                mode.description(),
                mode.offset()
            ),
        });
    }
    notices
}

/// Apply the control and occupant notification as one rollback-safe action.
pub fn lateral(
    scripts: &Scripts,
    unit: ObjectId,
    pilot: ObjectId,
    argument: &str,
) -> Result<super::Notice> {
    scripts.atomic(|_| {
        let notice = set_lateral(
            &mut scripts.world.borrow_mut(),
            unit,
            pilot,
            LateralMode::parse(argument)?,
        )?;
        super::notify_unit_text(scripts, unit, &notice.text)?;
        Ok(notice)
    })
}

/// Native control uses the invoking pilot's cockpit.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let unit = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        lateral(ctx.scripts, unit, ctx.player, &input.args)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
