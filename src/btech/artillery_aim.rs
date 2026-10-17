//! Artillery-specific range and skill arithmetic, separate from conventional weapon aiming.
//!
//! Artillery is fired directly or indirectly (Tactical Operations: Advanced Rules pp. 146-153).
//! A piece within seventeen hexes of a hex it can see fires directly: Gunnery +4, with the
//! attacker's movement, its own condition and the terrain it fires through, but no range,
//! target movement or target-hex terms, and never at a hex within six hexes (p. 153). Every
//! other shot is indirect: Gunnery +7, plus the spotter's term (and +1 when the spotter also
//! attacked that turn) less one per adjusted shot (pp. 149-150), plus the attacker's movement
//! when the hex lies within seventeen hexes (p. 153). Once a piece strikes a hex it hits that hex automatically until it moves (p. 150).
//!
//! Artillery cannons are ballistic weapons that only deal artillery damage (Tactical
//! Operations: Advanced Units & Equipment p. 97): a cannon that sees its hex aims as an ordinary
//! weapon, with its own range brackets and minimum range and no immobile-target modifier.
use super::Weapon;
use anyhow::{Result, ensure};
use serde::Serialize;

/// Hexes within which a piece that sees its target hex fires directly (Tactical Operations:
/// Advanced Rules pp. 147, 153); beyond them all artillery fire is indirect.
pub const DIRECT_ARTILLERY_RANGE: f64 = 17.0;

/// A direct-fire artillery attack cannot be made at a hex within this many hexes (Tactical
/// Operations: Advanced Rules p. 153).
pub const DIRECT_ARTILLERY_MINIMUM_RANGE: f64 = 6.0;

/// Base modifier of a direct-fire artillery attack (Tactical Operations: Advanced Rules p. 153).
const DIRECT_MODIFIER: i32 = 4;

/// Base modifier of an indirect artillery attack (Tactical Operations: Advanced Rules p. 150).
const INDIRECT_MODIFIER: i32 = 7;

/// Observer availability distinguishes an unassisted shot from a stale selected observer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtilleryObserver {
    /// No selected observer, or the shooter is its own observer.
    Unassisted,
    /// A selected observer identity no longer resolves to a unit.
    Unavailable,
    /// Current spotting skill target for a distinct selected observer.
    Spotting(i16),
}

/// Live inputs to the artillery hit calculation.
#[derive(Debug, Clone, Copy)]
pub struct ArtilleryAimInput {
    pub distance: f64,
    pub submerged: bool,
    /// The firing unit has line of sight to the target hex.
    pub visible: bool,
    pub gunnery: i16,
    pub observer: ArtilleryObserver,
    pub adjustment: u8,
    /// Electromagnetic interference covers the firing unit: +2, as for every ranged weapon
    /// attack (Tactical Operations: Advanced Rules p. 53).
    pub interference: bool,
    /// The attacker movement modifier: +1 walked, +2 ran, +3 jumped (Total Warfare p. 106).
    pub attacker_movement: u8,
    /// The firing unit's own condition: heat, sensor, crew and actuator damage. Direct fire and
    /// artillery cannons take it (Tactical Operations: Advanced Rules p. 153).
    pub attacker_condition: i16,
    /// Woods, smoke and other terrain between the firing unit and the hex (Total Warfare
    /// p. 107); direct fire pays it, indirect fire does not.
    pub intervening: i16,
    /// The piece has already struck this hex and not moved since, so indirect fire at it hits
    /// automatically (Tactical Operations: Advanced Rules p. 150).
    pub registered: bool,
    /// The spotter attacked in the turn it spots: +1 to the artillery attack (Tactical
    /// Operations: Advanced Rules p. 149).
    pub spotter_attacked: bool,
}

/// Range admission diagnostics. Only an in-range shot may be fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtilleryRange {
    InRange,
    OutOfRange,
    Underwater,
    /// A visible hex within six hexes: direct fire cannot be made and indirect fire is not
    /// allowed at a hex the piece sees within seventeen hexes (Tactical Operations: Advanced
    /// Rules p. 153).
    InsideMinimum,
}

/// How the shot is resolved, which decides how far a miss scatters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtilleryFire {
    /// Direct fire at a visible hex within seventeen hexes; a miss scatters 1D6 hexes.
    Direct,
    /// Indirect fire; a miss scatters one hex per point of the margin of failure.
    Indirect,
    /// An artillery cannon; a miss scatters half the margin of failure, rounded up, but never
    /// farther than the target is from the cannon.
    Cannon,
}

/// Complete artillery aim result. Range failure is distinct from launch admission or expenditure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ArtilleryAim {
    pub target_number: i32,
    pub maximum_range: u16,
    pub range: ArtilleryRange,
    pub fire: ArtilleryFire,
    /// The hex is registered: the shot hits without a roll.
    pub automatic: bool,
}

impl ArtilleryAim {
    /// The number the launch roll must reach; a registered hex is struck whatever is rolled.
    pub fn roll_target(&self) -> i32 {
        if self.automatic {
            2
        } else {
            self.target_number
        }
    }
}

/// Artillery aim for launchers whose impacts use delayed area effects.
pub trait ArtilleryAiming {
    /// Compute artillery aim without consulting or consuming random state.
    fn artillery_aim(self, input: ArtilleryAimInput) -> Result<ArtilleryAim>;
}

impl ArtilleryAiming for Weapon {
    fn artillery_aim(self, input: ArtilleryAimInput) -> Result<ArtilleryAim> {
        ensure!(self.is_artillery(), "Weapon is not artillery");
        calculate(self, input)
    }
}

/// Apply range boundaries before skill or correction; integer spotting division truncates
/// toward zero.
fn calculate(weapon: Weapon, input: ArtilleryAimInput) -> Result<ArtilleryAim> {
    ensure!(
        input.distance.is_finite() && input.distance >= 0.0,
        "Invalid artillery range"
    );
    let maximum_range = weapon.effective_range();
    let cannon = !weapon.is_indirect_artillery();
    let refused = |target_number, range| ArtilleryAim {
        target_number,
        maximum_range,
        range,
        fire: if cannon {
            ArtilleryFire::Cannon
        } else {
            ArtilleryFire::Indirect
        },
        automatic: false,
    };
    if input.submerged {
        return Ok(refused(5000, ArtilleryRange::Underwater));
    }
    if input.distance > f64::from(maximum_range) {
        return Ok(refused(1000, ArtilleryRange::OutOfRange));
    }
    let interference = if input.interference {
        i32::from(super::planetary_conditions::INTERFERENCE_AIM_MODIFIER)
    } else {
        0
    };
    let gunnery = i32::from(input.gunnery);
    let movement = i32::from(input.attacker_movement);
    let direct_terms =
        movement + i32::from(input.attacker_condition) + i32::from(input.intervening);
    if cannon && input.visible {
        let range = weapon
            .range_modifier(input.distance)?
            .map_or(0, |range| i32::from(range.modifier));
        return Ok(ArtilleryAim {
            target_number: gunnery + range + direct_terms + interference,
            maximum_range,
            range: ArtilleryRange::InRange,
            fire: ArtilleryFire::Cannon,
            automatic: false,
        });
    }
    let close = input.distance <= DIRECT_ARTILLERY_RANGE;
    if !cannon && input.visible && close {
        if input.distance <= DIRECT_ARTILLERY_MINIMUM_RANGE {
            return Ok(ArtilleryAim {
                fire: ArtilleryFire::Direct,
                ..refused(1000, ArtilleryRange::InsideMinimum)
            });
        }
        return Ok(ArtilleryAim {
            target_number: gunnery + DIRECT_MODIFIER + direct_terms + interference,
            maximum_range,
            range: ArtilleryRange::InRange,
            fire: ArtilleryFire::Direct,
            automatic: input.registered,
        });
    }
    let spotter = match input.observer {
        ArtilleryObserver::Unassisted | ArtilleryObserver::Unavailable => 0,
        ArtilleryObserver::Spotting(target) => {
            (i32::from(target) - 4) / 2 + i32::from(input.spotter_attacked)
        }
    };
    // Indirect fire at a hex within seventeen hexes pays for the attacker's movement
    // (Tactical Operations: Advanced Rules p. 153).
    let movement = if close { movement } else { 0 };
    Ok(ArtilleryAim {
        target_number: gunnery + INDIRECT_MODIFIER + spotter + movement + interference
            - i32::from(input.adjustment),
        maximum_range,
        range: ArtilleryRange::InRange,
        fire: if cannon {
            ArtilleryFire::Cannon
        } else {
            ArtilleryFire::Indirect
        },
        automatic: !cannon && input.registered,
    })
}

/// Read artillery gunnery independently of the extended conventional gunnery setting.
pub fn unit_artillery_gunnery_target(world: &crate::World, unit: crate::ObjectId) -> Result<i16> {
    super::skills::operator_artillery_gunnery_target(
        world,
        super::skills::active_pilot(world, unit)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An unspotted indirect shot from a stationary piece far from its target.
    fn base() -> ArtilleryAimInput {
        ArtilleryAimInput {
            distance: 40.0,
            submerged: false,
            visible: false,
            gunnery: 4,
            observer: ArtilleryObserver::Unassisted,
            adjustment: 0,
            interference: false,
            attacker_movement: 2,
            attacker_condition: 1,
            intervening: 2,
            registered: false,
            spotter_attacked: false,
        }
    }

    /// Indirect fire is Gunnery +7 with no extra term for an unspotted shot; movement counts
    /// only within seventeen hexes (Tactical Operations: Advanced Rules pp. 150, 153).
    #[test]
    fn indirect_fire_takes_movement_only_within_seventeen_hexes() {
        let far = Weapon::ArrowIv.artillery_aim(base()).unwrap();
        assert_eq!(far.target_number, 11);
        assert_eq!(far.fire, ArtilleryFire::Indirect);
        let near = Weapon::ArrowIv
            .artillery_aim(ArtilleryAimInput {
                distance: 12.0,
                ..base()
            })
            .unwrap();
        assert_eq!(near.target_number, 13);
        let spotted = Weapon::ArrowIv
            .artillery_aim(ArtilleryAimInput {
                observer: ArtilleryObserver::Spotting(2),
                adjustment: 2,
                ..base()
            })
            .unwrap();
        assert_eq!(spotted.target_number, 8);
        // A spotter that also attacks makes the artillery attack one harder (p. 149).
        let busy = Weapon::ArrowIv
            .artillery_aim(ArtilleryAimInput {
                observer: ArtilleryObserver::Spotting(2),
                adjustment: 2,
                spotter_attacked: true,
                ..base()
            })
            .unwrap();
        assert_eq!(busy.target_number, 9);
    }

    /// Direct fire within seventeen hexes is Gunnery +4 with movement, condition and
    /// intervening terrain, and cannot be made within six hexes (p. 153).
    #[test]
    fn direct_fire_has_its_own_terms_and_minimum_range() {
        let direct = Weapon::Sniper
            .artillery_aim(ArtilleryAimInput {
                distance: 17.0,
                visible: true,
                ..base()
            })
            .unwrap();
        assert_eq!(direct.target_number, 4 + 4 + 2 + 1 + 2);
        assert_eq!(direct.fire, ArtilleryFire::Direct);
        let beyond = Weapon::Sniper
            .artillery_aim(ArtilleryAimInput {
                distance: 18.0,
                visible: true,
                ..base()
            })
            .unwrap();
        assert_eq!(beyond.fire, ArtilleryFire::Indirect);
        assert_eq!(beyond.target_number, 11);
        let close = Weapon::Sniper
            .artillery_aim(ArtilleryAimInput {
                distance: 6.0,
                visible: true,
                ..base()
            })
            .unwrap();
        assert_eq!(close.range, ArtilleryRange::InsideMinimum);
        let unseen = Weapon::Sniper
            .artillery_aim(ArtilleryAimInput {
                distance: 3.0,
                ..base()
            })
            .unwrap();
        assert_eq!(unseen.range, ArtilleryRange::InRange);
        assert_eq!(unseen.target_number, 13);
    }

    /// A registered hex is struck automatically; cannons never register hexes.
    #[test]
    fn registered_hexes_hit_automatically() {
        let registered = ArtilleryAimInput {
            registered: true,
            ..base()
        };
        let aim = Weapon::LongTom.artillery_aim(registered).unwrap();
        assert!(aim.automatic);
        assert_eq!(aim.roll_target(), 2);
        let cannon = Weapon::LongTomCannon
            .artillery_aim(ArtilleryAimInput {
                distance: 10.0,
                ..registered
            })
            .unwrap();
        assert!(!cannon.automatic);
    }

    /// A cannon that sees its hex aims with its own range brackets and minimum range
    /// (Tactical Operations: Advanced Units & Equipment p. 97).
    #[test]
    fn cannons_aim_as_ballistic_weapons() {
        let aim = |distance| {
            Weapon::LongTomCannon
                .artillery_aim(ArtilleryAimInput {
                    distance,
                    visible: true,
                    ..base()
                })
                .unwrap()
        };
        // Medium range (7-13 hexes) is +2.
        assert_eq!(aim(10.0).target_number, 4 + 2 + 2 + 1 + 2);
        // Two hexes inside a four-hex minimum range is +3.
        assert_eq!(aim(2.0).target_number, 4 + 3 + 2 + 1 + 2);
        assert_eq!(aim(2.0).fire, ArtilleryFire::Cannon);
        assert_eq!(aim(21.0).range, ArtilleryRange::OutOfRange);
    }
}
