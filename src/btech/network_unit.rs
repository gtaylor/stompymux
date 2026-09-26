//! Borrowed command-network inputs keep membership and reporting independent of unit anatomy.
use super::*;
use crate::{BattleCommandNetwork, ObjectId, World};
use anyhow::{Context, Result};

/// Common computer and tactical facts for one network participant or contact.
pub(super) struct NetworkUnit<'a> {
    pub c3_network: Option<u64>,
    pub c3i_network: Option<u64>,
    scanner: super::scanner::ScannerUnit<'a>,
    pilot: Option<ObjectId>,
    motion: Option<BattleMotion>,
    computer: ComputerSource<'a>,
    pub protection: (u32, u32, u32, u32),
}

/// Borrow equipment only when a network rule needs hardware, avoiding eager inventory scans.
enum ComputerSource<'a> {
    Mech(&'a BattleUnit),
    Vehicle(&'a BattleVehicle),
}

impl NetworkUnit<'_> {
    /// Current map placement.
    pub fn position(&self) -> Option<BattlePosition> {
        self.scanner.position
    }
    /// Reactor state remains separate from installed hardware.
    pub fn power(&self) -> BattlePower {
        self.scanner.power
    }
    /// Team and optical signature.
    pub fn signature(&self) -> BattleUnitSignature {
        self.scanner.signature
    }
    /// Current operator for message reception.
    pub fn pilot(&self) -> Option<ObjectId> {
        self.pilot
    }
    /// Common movement snapshot.
    pub fn motion(&self) -> Option<BattleMotion> {
        self.motion
    }
    /// Physical and working computer counts.
    pub fn c3_hardware(&self) -> Result<BattleC3Hardware> {
        match self.computer {
            ComputerSource::Mech(unit) => unit.c3_hardware(),
            ComputerSource::Vehicle(unit) => unit.c3_hardware(),
        }
    }
    /// Classic computer eligibility before power and interference checks.
    pub fn c3_operational(&self) -> Result<bool> {
        match self.computer {
            ComputerSource::Mech(unit) => unit.c3_operational(),
            ComputerSource::Vehicle(unit) => unit.c3_operational(),
        }
    }
    /// Tactical label independent of the construction store.
    pub fn battlefield_id(&self) -> Option<String> {
        self.scanner.label()
    }
    /// Resolved presentation name shared with identified contacts.
    pub fn name(&self) -> &str {
        self.scanner.name
    }
    /// Contact arcs use the same facing convention as other tactical displays.
    pub fn facing(&self) -> BattleFacing {
        self.scanner.facing
    }
    /// Whole-unit destruction state.
    pub fn is_destroyed(&self) -> bool {
        self.scanner.destroyed
    }
    /// Selected unit target, if any.
    pub fn selected(&self) -> Option<ObjectId> {
        self.scanner.selected
    }
}

/// Read a participant without borrowing either construction type into network rules.
pub(super) fn unit(world: &World, id: ObjectId) -> Result<NetworkUnit<'_>> {
    let scanner = super::scanner::scanner_unit(world, id).context("Unit not found")?;
    let (c3_network, c3i_network, pilot, motion, computer, protection) =
        if let Some(unit) = world.btech.vehicles().get(&id) {
            (
                unit.c3_network,
                unit.c3i_network,
                unit.pilot(),
                unit.motion(),
                ComputerSource::Vehicle(unit),
                totals(
                    unit.sections().values(),
                    unit.definition().sections.values(),
                ),
            )
        } else {
            let unit = &world.btech.constructed_units()[&id];
            (
                unit.c3_network,
                unit.c3i_network,
                unit.pilot(),
                unit.motion(),
                ComputerSource::Mech(unit),
                totals(
                    unit.sections().values(),
                    unit.definition().sections.values(),
                ),
            )
        };
    Ok(NetworkUnit {
        c3_network,
        c3i_network,
        pilot,
        motion,
        computer,
        protection,
        scanner,
    })
}

/// Read both stores in object order for deterministic network identities and capacity trimming.
pub(super) fn units(
    world: &World,
) -> Result<std::collections::BTreeMap<ObjectId, NetworkUnit<'_>>> {
    world
        .btech
        .constructed_units()
        .keys()
        .chain(world.btech.vehicles().keys())
        .map(|&id| Ok((id, unit(world, id)?)))
        .collect()
}

/// Change only the requested family's durable identity in the owning construction store.
pub(super) fn set_link(
    world: &mut World,
    id: ObjectId,
    kind: BattleCommandNetwork,
    value: Option<u64>,
) {
    let (classic, improved) = if world.btech.vehicles().contains_key(&id) {
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        (&mut unit.c3_network, &mut unit.c3i_network)
    } else {
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        (&mut unit.c3_network, &mut unit.c3i_network)
    };
    *match kind {
        BattleCommandNetwork::C3 => classic,
        BattleCommandNetwork::C3i => improved,
    } = value;
}

/// Sum remaining and original protection, including rear armor where the chassis has it.
fn totals<'a>(
    live: impl Iterator<Item = &'a BattleSectionState>,
    original: impl Iterator<Item = &'a SectionDefinition>,
) -> (u32, u32, u32, u32) {
    let sum = |(a, i), (armor, rear, internal)| {
        (
            a + u32::from(armor) + u32::from(rear),
            i + u32::from(internal),
        )
    };
    let (armor, internal) = live
        .map(|s| (s.armor, s.rear, s.internal))
        .fold((0, 0), sum);
    let (original_armor, original_internal) = original
        .map(|s| (s.armor, s.rear, s.internal))
        .fold((0, 0), sum);
    (armor, original_armor, internal, original_internal)
}
