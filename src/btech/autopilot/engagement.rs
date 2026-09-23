//! Stable engagement regions derived only from own weapons and observed contacts.
use super::{
    AutopilotRangeBand, alignment,
    navigation::{Goal, Hex},
    observations::AutopilotObservation,
};
use crate::{BattleHexCoordinate, BattlePosition, ObjectId, World};

/// Temporary weapon readiness does not change the preferred position.
pub(crate) fn preferred(observation: &AutopilotObservation) -> AutopilotRangeBand {
    let maximum = observation
        .own
        .weapons
        .iter()
        .filter(|w| alignment::available(w))
        .map(|w| w.weapon.profile().long_range)
        .max()
        .unwrap_or(1);
    let value = |range: u16| {
        observation
            .own
            .weapons
            .iter()
            .filter(|w| alignment::available(w))
            .map(|w| alignment::effectiveness(w.weapon, f64::from(range)))
            .sum::<f64>()
    };
    let best = (1..=maximum)
        .map(|r| value(u16::from(r)))
        .fold(0.0, f64::max);
    if best <= 0.0 {
        return AutopilotRangeBand {
            minimum: 1,
            maximum: 1,
        };
    }
    let farthest = (1..=maximum)
        .rev()
        .find(|r| value(u16::from(*r)) >= best * 0.9)
        .unwrap_or(1);
    let maximum = u16::from(farthest);
    let minimum = if maximum > 1 && value(maximum - 1) >= best * 0.9 {
        maximum - 1
    } else {
        maximum
    };
    AutopilotRangeBand { minimum, maximum }
}

/// Search intent; both the target and leash come from admitted, filtered information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Engagement {
    pub target: BattlePosition,
    pub target_id: ObjectId,
    pub aim: BattlePosition,
    pub band: AutopilotRangeBand,
    pub leash: Option<BattlePosition>,
    pub maximum: u16,
}
impl Engagement {
    pub fn goal(self, fallback: bool) -> Goal {
        Goal::annulus(
            Hex::new(self.aim.x, self.aim.y),
            if fallback {
                1
            } else {
                u32::from(self.band.minimum)
            },
            if fallback {
                u32::from(self.maximum)
            } else {
                u32::from(self.band.maximum)
            },
        )
    }
    /// Live settling always uses the actual visible target.
    pub fn observed_goal(self, fallback: bool) -> Goal {
        Self {
            aim: self.target,
            ..self
        }
        .goal(fallback)
    }
    /// Ordinary sighting changes must not discard an unchanged prediction route.
    pub fn same_navigation(self, other: Self) -> bool {
        self.target_id == other.target_id
            && self.aim == other.aim
            && self.band == other.band
            && self.leash == other.leash
            && self.maximum == other.maximum
    }
    pub fn navigation_usable(
        self,
        world: &World,
        observation: &AutopilotObservation,
        hex: Hex,
    ) -> bool {
        Self {
            target: self.aim,
            ..self
        }
        .usable(world, observation, hex)
    }
    pub fn permits(self, hex: Hex) -> bool {
        self.leash
            .is_none_or(|origin| Hex::new(origin.x, origin.y).distance(hex) <= 6)
    }
    /// Prospective geometry reads public terrain and own weapon capability only.
    pub fn usable(self, world: &World, observation: &AutopilotObservation, hex: Hex) -> bool {
        if !self.permits(hex) {
            return false;
        }
        let Some(map) = world.btech.maps().get(&self.target.map) else {
            return false;
        };
        let here = BattleHexCoordinate {
            x: i32::from(hex.x),
            y: i32::from(hex.y),
        };
        let there = BattleHexCoordinate {
            x: i32::from(self.target.x),
            y: i32::from(self.target.y),
        };
        let Ok(range) = here.center().range(there.center()) else {
            return false;
        };
        observation
            .own
            .weapons
            .iter()
            .any(|w| alignment::available(w) && alignment::effectiveness(w.weapon, range) > 0.0)
            && crate::btech::ground_terrain_los(map, here, there).is_ok_and(|los| !los.blocked)
    }
}

/// Resolve explicit attack precedence before optional opportunistic diversion.
pub(crate) fn resolve(
    world: &World,
    id: ObjectId,
    record: &super::AutopilotOrderRecord,
    config: &super::AutopilotConfig,
    observation: &AutopilotObservation,
) -> Option<Engagement> {
    let (target, range, leash) = match record.order {
        super::AutopilotOrder::Attack { target, range } => (
            super::combat_policy::choose_target(observation, Some(target), None)?,
            range,
            None,
        ),
        super::AutopilotOrder::AttackMove { .. } => {
            if config.fire_mode != super::AutopilotFireMode::Opportunistic {
                return None;
            }
            let selected = crate::btech::scanner::scanner_unit(world, id).and_then(|u| u.selected);
            let target = super::runtime::opportunistic_target(config, observation, selected)?;
            if Some(target) == record.progress.attack_move_suppressed_target {
                return None;
            }
            let current = observation.position?;
            (
                target,
                None,
                Some(record.progress.attack_move_origin.unwrap_or(current)),
            )
        }
        _ => return None,
    };
    let contact = observation
        .contacts
        .iter()
        .find(|c| c.unit == target && c.identified && !c.friendly && !c.known_destroyed)?;
    if leash.is_some_and(|origin| {
        origin.map != contact.position.map
            || Hex::new(origin.x, origin.y)
                .distance(Hex::new(contact.position.x, contact.position.y))
                > 6
    }) {
        return None;
    }
    Some(Engagement {
        target: contact.position,
        aim: contact.position,
        target_id: target,
        band: range
            .or(config.preferred_range)
            .unwrap_or_else(|| preferred(observation)),
        leash,
        maximum: observation
            .own
            .weapons
            .iter()
            .filter(|w| alignment::available(w))
            .map(|w| u16::from(w.weapon.profile().long_range))
            .max()
            .unwrap_or(1),
    })
}
