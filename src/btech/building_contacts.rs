//! Structure contact disclosure through live terrain visibility and silent identification locks.
use super::fire_target::TargetSource;
use crate::{Flag, LockInvocation, LockType, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// One identified or restricted visible entrance, without mine or character-skill disclosure.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleBuildingContact {
    /// Whether the sensor band or sight reaches the entrance hex.
    pub detection: Option<super::BattleDetectionChannel>,
    /// Plain compact building row after lock evaluation.
    pub short_text: String,
    /// General observer torso direction toward the entrance.
    pub weapon_arc: super::BattleContactArc,
    pub interior: ObjectId,
    pub coordinate: super::BattleHexCoordinate,
    pub elevation: i32,
    pub name: String,
    pub range: f64,
    pub bearing: u16,
    pub integrity: i64,
    pub maximum_integrity: i64,
    pub identified: bool,
    pub hidden: bool,
    pub status: char,
}

impl BattleBuildingContact {
    /// Restricted identification highlights the row without interpreting its name as markup.
    pub fn styled_text(&self) -> String {
        super::contacts::styled_row(&self.short_text, (!self.identified).then_some("yellow"))
    }

    /// Plain contact text for the native combined list; the caller owns final text escaping.
    pub fn text(&self) -> String {
        let name: String = crate::text::plain(&self.name).chars().take(23).collect();
        format!(
            "{}{}{} {:<23} x:{:>3} y:{:>3} z:{:>2} r:{:>4.1} b:{:>3} CF:{:>4} /{:>4} S:{}{}",
            super::contacts::detection_code(self.detection, true),
            ' ',
            self.weapon_arc.symbol(),
            name,
            self.coordinate.x,
            self.coordinate.y,
            self.elevation,
            self.range,
            self.bearing,
            self.integrity,
            self.maximum_integrity,
            self.status,
            if self.hidden { 'H' } else { ' ' }
        )
    }
}

/// List visible entrances in saved order, rolling back lock side effects if any evaluation fails.
/// Invisible or unavailable buildings never invoke their lock; locks may deliberately change state.
pub fn building_contacts(
    scripts: &Scripts,
    observer: ObjectId,
    pilot: ObjectId,
) -> Result<Vec<BattleBuildingContact>> {
    scripts.atomic(|_| {
        let source = super::brief::display_source(&scripts.world.borrow(), observer, pilot)?;
        let map = admission(&scripts.world.borrow(), source, pilot)?;
        let entries: Vec<_> = scripts.world.borrow().btech.maps()[&map]
            .building_entrances()
            .iter()
            .map(|(&slot, &entrance)| (slot, entrance))
            .collect();
        let mut contacts = Vec::new();
        for (slot, entrance) in entries {
            if candidate(&scripts.world.borrow(), source, pilot, map, slot, entrance)?.is_none() {
                continue;
            }
            let outcome = scripts.evaluate_lock(LockInvocation {
                kind: LockType::IdentifyBuilding,
                object: entrance.interior,
                enactor: pilot,
                subject: pilot,
                cause: source.unit,
                descriptor: None,
                silent: true,
            })?;
            // A callback can remove or change an entrance, the structure, or observer visibility.
            let Some(mut contact) =
                candidate(&scripts.world.borrow(), source, pilot, map, slot, entrance)?
            else {
                continue;
            };
            if contact.hidden && !outcome.passes {
                continue;
            }
            let building = scripts.world.borrow().btech.maps()[&entrance.interior].building;
            contact.identified = outcome.passes;
            contact.status =
                if building.is_safe() || (!outcome.passes && building.is_command_center()) {
                    'X'
                } else if !outcome.passes {
                    'x'
                } else if building.is_command_center() {
                    'C'
                } else {
                    ' '
                };
            contact.short_text = contact.text();
            if contacts.len() < 250 {
                contacts.push(contact);
            }
        }
        Ok(contacts)
    })
}

/// Revalidate the original operator after identification callbacks.
fn admission(world: &World, source: TargetSource, pilot: ObjectId) -> Result<ObjectId> {
    super::brief::display_source(world, source.unit, pilot)?;
    let observer = source.unit;
    let unit = super::scanner::scanner_unit(world, observer).context("Scanner is unavailable")?;
    ensure!(
        unit.power == super::BattlePower::Running && !unit.destroyed,
        "Start the unit first"
    );
    Ok(unit.position.context("Unit is not on a battlefield")?.map)
}

/// Resolve only current visible data, both before and after the identification callback.
fn candidate(
    world: &World,
    source: TargetSource,
    pilot: ObjectId,
    map: ObjectId,
    slot: u32,
    entrance: super::BattleBuildingEntrance,
) -> Result<Option<BattleBuildingContact>> {
    if admission(world, source, pilot)? != map {
        return Ok(None);
    }
    let observer = source.unit;
    let field = &world.btech.maps()[&map];
    if field.building_entrances().get(&slot) != Some(&entrance) {
        return Ok(None);
    }
    let Some(interior) = world.btech.maps().get(&entrance.interior) else {
        return Ok(None);
    };
    let Some(object) = world
        .objects
        .get(&entrance.interior)
        .filter(|o| !o.flags.contains(Flag::Going))
    else {
        return Ok(None);
    };
    if interior.building.is_invisible() {
        return Ok(None);
    }
    let detection = super::hex_detection(world, observer, entrance.coordinate)?;
    if detection.is_none()
        || !super::visibility::hex_unblocked(world, observer, entrance.coordinate)?
    {
        return Ok(None);
    }
    let unit = super::scanner::scanner_unit(world, observer).context("Scanner is unavailable")?;
    let point = unit.point.context("Unit has no motion state")?;
    let center = entrance.coordinate.center();
    let elevation = i32::from(
        field
            .base_hex(
                i64::from(entrance.coordinate.x),
                i64::from(entrance.coordinate.y),
            )?
            .elevation,
    ) + 1;
    let altitude = super::unit_elevation(world, observer)?.context("Unit has no elevation")?;
    let heading = unit.heading.context("Unit has no heading")?;
    let bearing = point.bearing(center)?.unwrap_or(180.0);
    Ok(Some(BattleBuildingContact {
        detection,
        short_text: String::new(),
        weapon_arc: unit.facing.contact_arc(
            if unit.vehicle {
                heading.trunc()
            } else {
                heading
            },
            if unit.vehicle {
                bearing.round()
            } else {
                bearing
            },
        )?,
        interior: entrance.interior,
        coordinate: entrance.coordinate,
        elevation,
        name: crate::text::plain(&object.name),
        range: point
            .range(center)?
            .hypot(f64::from(elevation - altitude) / 5.0),
        bearing: bearing.round().rem_euclid(360.0) as u16,
        integrity: interior.building.integrity,
        maximum_integrity: interior.building.maximum_integrity,
        identified: false,
        hidden: interior.building.is_hidden(),
        status: ' ',
    }))
}
