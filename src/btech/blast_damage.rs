//! Shared mine and artillery packets, with anatomy-specific location and heat responses.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Anatomy-specific consequences retained for blast reports and character publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "report", rename_all = "snake_case")]
pub enum BattleBlastImpact {
    Mech(BattleTacticalImpact),
    Vehicle(BattleVehicleImpact),
}

impl BattleBlastImpact {
    /// Preserve packet-local pilot messages when a caller joins material reports.
    pub(super) fn append_feedback(&self, private: &mut Vec<BattlePilotNotice>, offset: usize) {
        let notices = match self {
            Self::Mech(impact) => &impact.pilot_notices,
            Self::Vehicle(impact) => &impact.pilot_notices,
        };
        super::piloting::append_feedback(private, notices.iter().cloned(), offset);
    }
}

/// One blast cell's geometry; source-specific altitude limits are exclusive.
pub(super) struct BlastCell {
    pub coordinate: BattleHexCoordinate,
    pub origin: BattlePoint,
    pub tile: BattleHex,
    pub lower: i32,
    pub upper: i32,
}

/// Live target facts sampled immediately before its packets, after earlier targets' effects.
pub(super) struct BlastTarget {
    pub arc: BattleHitArc,
    pub character: bool,
    pub rules: BattleFallRules,
}

impl BlastCell {
    /// Select a live occupant and capture its facing and pilot protection without consuming dice.
    pub fn target(
        &self,
        world: &World,
        id: ObjectId,
        mut rules: BattleFallRules,
    ) -> Result<Option<BlastTarget>> {
        let Some(object) = world
            .objects
            .get(&id)
            .filter(|object| !object.flags.contains(Flag::Going))
        else {
            return Ok(None);
        };
        let unit =
            super::scanner::scanner_unit(world, id).context("Blast target is unavailable")?;
        if !unit.position.is_some_and(|position| {
            i32::from(position.x) == self.coordinate.x && i32::from(position.y) == self.coordinate.y
        }) {
            return Ok(None);
        }
        let (elevation, motion, pilot) = if let Some(unit) = world.btech.vehicles().get(&id) {
            (unit.elevation_level(self.tile), unit.motion(), unit.pilot())
        } else {
            let unit = &world.btech.constructed_units()[&id];
            (unit.elevation_level(self.tile), unit.motion(), unit.pilot())
        };
        if elevation <= self.lower || elevation >= self.upper {
            return Ok(None);
        }
        let motion = motion.context("Blast target is not placed")?;
        let bearing = motion.point.bearing(self.origin)?.unwrap_or(180.0);
        rules.toughness = pilot
            .and_then(|pilot| world.btech.character_values().get(&pilot))
            .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
        rules.vehicle_impact.criticals.toughness |= rules.toughness;
        Ok(Some(BlastTarget {
            arc: blast_arc(bearing, motion.heading),
            character: object.flags.contains(Flag::InCharacter),
            rules,
        }))
    }
}

/// Geometry and admission belong to the enclosing blast source.
pub(super) struct BlastDamage {
    pub damage: u16,
    pub packet_size: u8,
    pub table: BattleHitTable,
    pub arc: BattleHitArc,
    pub heat: i32,
    pub character: bool,
}

/// Ordered material and thermal effects from one admitted occupant.
pub(super) struct BlastEffects {
    pub impacts: Vec<BattleBlastImpact>,
    pub vehicle_heat: Option<BattleVehicleHeatExposure>,
    pub burn_seconds: i64,
    pub notices: Vec<BattleNotice>,
    /// Packet-local private messages offset into this occupant's notice stream.
    pub pilot_notices: Vec<BattlePilotNotice>,
}

/// Finish every packet before heat, preserving the blast cell's rear-armor selector.
pub(super) fn resolve(
    world: &mut World,
    id: ObjectId,
    request: BlastDamage,
    rear: &mut bool,
    rules: BattleFallRules,
) -> Result<BlastEffects> {
    ensure!(
        request.packet_size > 0,
        "Blast packet size must be positive"
    );
    let vehicle = world.btech.vehicles().contains_key(&id);
    let mut effects = BlastEffects {
        impacts: Vec::new(),
        vehicle_heat: None,
        burn_seconds: 0,
        notices: Vec::new(),
        pilot_notices: Vec::new(),
    };
    let mut remaining = request.damage;
    while remaining > 0 {
        let amount = remaining.min(u16::from(request.packet_size));
        remaining -= amount;
        let (impact, notices) = resolve_packet(
            world,
            id,
            MaterialPacket {
                amount,
                table: request.table,
                arc: request.arc,
                character: request.character,
                attacker: None,
            },
            *rear,
            rules,
        )?;
        impact.append_feedback(&mut effects.pilot_notices, effects.notices.len());
        effects.notices.extend(notices);
        effects.impacts.push(impact);
    }
    if vehicle {
        let before = world.clone();
        let heat = resolve_vehicle_heat_exposure(world, id, request.heat, rules.vehicle_impact)?;
        effects.burn_seconds = heat.burn_seconds;
        super::piloting::append_feedback(
            &mut effects.pilot_notices,
            heat.pilot_notices.iter().cloned(),
            effects.notices.len(),
        );
        effects.notices.extend(heat.notices.clone());
        append_broadcasts(&before, &mut effects.notices, &heat.broadcasts);
        effects.vehicle_heat = Some(heat);
    } else if request.heat != 0 {
        effects.burn_seconds = i64::from(request.heat) * 6;
        super::inferno::adjust_burn(world, id, effects.burn_seconds)?;
    }
    Ok(effects)
}

/// One admitted material packet; its owner supplies geometry, attribution and crew policy.
pub(super) struct MaterialPacket {
    pub amount: u16,
    pub table: BattleHitTable,
    pub arc: BattleHitArc,
    pub character: bool,
    pub attacker: Option<ObjectId>,
}

/// Shared location and impact resolution for blast and wizard packets, including post-destruction rolls.
pub(super) fn resolve_packet(
    world: &mut World,
    id: ObjectId,
    request: MaterialPacket,
    rear: bool,
    rules: BattleFallRules,
) -> Result<(BattleBlastImpact, Vec<BattleNotice>)> {
    let vehicle = world.btech.vehicles().contains_key(&id);
    if vehicle {
        let before = world.clone();
        let impact = super::vehicle_impact::resolve_followup_in_candidate(
            world,
            id,
            request.arc,
            super::vehicle_impact::ImpactRequest {
                amount: u32::from(request.amount),
                armor_piercing: None,
                rear,
                attacker: request.attacker,
            },
            rules.vehicle_impact,
        )?;
        let mut notices = impact.notices.clone();
        append_broadcasts(&before, &mut notices, &impact.broadcasts);
        return Ok((BattleBlastImpact::Vehicle(impact), notices));
    }
    let unit = &world.btech.constructed_units()[&id];
    let mut dice = unit.dice.clone();
    let location = if request.table == BattleHitTable::Weapon {
        let roll = dice.generic_roll();
        let mut location = rules.hit.resolve(unit, request.arc, roll, &mut dice)?;
        location.rear_armor = rear;
        location
    } else {
        BattleHit {
            section: request
                .table
                .location(unit.chassis(), request.arc, dice.d6())?,
            rear_armor: rear,
            through_armor_critical: false,
            crew_stun: false,
        }
    };
    world.btech.constructed.get_mut(&id).unwrap().dice = dice;
    // Unplaced scenario units have material damage but no battlefield fall or flooding effects.
    let environment = world.btech.constructed_units()[&id]
        .position()
        .is_some()
        .then_some(rules);
    let impact = super::impact::resolve_attack_in_candidate(
        world,
        id,
        location,
        request.amount,
        environment,
        super::impact::AttackImpact {
            attacker: request.attacker,
            weapon_effect: None,
            character: request.character,
            followup: true,
        },
    )?;
    let notices = impact.notices.clone();
    Ok((BattleBlastImpact::Mech(impact), notices))
}

/// Resolve visibility against the state immediately before each material or thermal effect.
fn append_broadcasts(world: &World, notices: &mut Vec<BattleNotice>, broadcasts: &[BattleNotice]) {
    for notice in broadcasts {
        notices.extend(super::broadcast::observer_notices(
            world,
            notice.unit,
            &notice.text,
        ));
    }
}

/// Area blast arcs use strict 60/120-degree boundaries independent of weapon arc configuration.
fn blast_arc(bearing: f64, heading: f64) -> BattleHitArc {
    let angle = (bearing - heading).rem_euclid(360.0);
    if angle > 120.0 && angle < 240.0 {
        return BattleHitArc::Rear;
    }
    if angle > 300.0 || angle < 60.0 {
        return BattleHitArc::Front;
    }
    if angle > 180.0 {
        return BattleHitArc::Left;
    }
    BattleHitArc::Right
}

/// Neighboring blast cells ignite undecorated woods using the map's durable random stream.
pub(super) fn ignite_forest(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
) -> Result<bool> {
    let record = &world.btech.maps()[&map];
    let tile = record.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    if !matches!(tile.terrain, Terrain::LightForest | Terrain::HeavyForest)
        || record.decoration(coordinate)?.is_some()
    {
        return Ok(false);
    }
    let dice = world
        .btech
        .maps
        .get_mut(&map)
        .unwrap()
        .fire_dice
        .as_mut()
        .context("Map fire random stream is unavailable")?;
    let remaining = 59 + dice.die(121)?;
    set_map_decoration(
        world,
        map,
        coordinate,
        Some(BattleDecoration::new(
            BattleDecorationKind::Fire,
            i64::from(remaining),
            None,
        )),
    )?;
    Ok(true)
}
