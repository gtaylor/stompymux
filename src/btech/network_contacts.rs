//! Command-network additions to the contact picture: the shared aiming range an active C3 or
//! C3i network supplies for every contact the observer or its side holds.
use super::contacts::ContactFacts;
use super::network_unit::unit as network_unit;
use super::{NetworkRange, Power};
use crate::{CommandNetwork, ObjectId, World};
use anyhow::Result;

/// One observer's usable networks: every family that can currently supply range, in the
/// priority order aiming uses.
pub(super) struct NetworkSightings<'w> {
    world: &'w World,
    observer: ObjectId,
    /// Active peers per family; the first entry decides the shared aiming distance.
    families: Vec<(CommandNetwork, Vec<ObjectId>)>,
}

impl<'w> NetworkSightings<'w> {
    /// The observer's active networks, or None when no network can currently supply range
    /// (unlinked, shut down, or jammed).
    pub(super) fn new(world: &'w World, observer: ObjectId) -> Result<Option<Self>> {
        let unit = network_unit(world, observer)?;
        if (unit.c3_network.is_none() && unit.c3i_network.is_none())
            || unit.power() != Power::Running
            || super::electronic_field(world, observer)?.blocks_outgoing_guidance()
        {
            return Ok(None);
        }
        let mut families = Vec::new();
        for kind in [CommandNetwork::C3, CommandNetwork::C3i] {
            if super::command_network::members_for(world, observer, kind)?.is_empty() {
                continue;
            }
            let peers: Vec<_> =
                super::command_network::active_members(world, observer, kind, false)?
                    .into_iter()
                    .filter(|peer| *peer != observer)
                    .collect();
            families.push((kind, peers));
        }
        if families.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            world,
            observer,
            families,
        }))
    }

    /// Shared aiming distance for `target` from the priority family, given the observer's
    /// own physical distance.
    pub(super) fn range(&self, target: ObjectId, physical: f64) -> Result<NetworkRange> {
        let (kind, peers) = &self.families[0];
        super::network_range::select(
            self.world,
            self.observer,
            super::network_range::NetworkTarget::Unit(target),
            physical,
            peers,
            *kind,
        )
    }
}

/// One contact in a networked unit's tactical picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NetworkedContactFacts {
    pub(crate) facts: ContactFacts,
    /// Held only by teammates; the observer did not perceive it itself.
    pub(crate) relayed: bool,
    /// Shared aiming distance, when the observer has an active network.
    pub(crate) network_range: Option<f64>,
}

/// The observer's contacts, its own and its side's, each carrying the shared aiming distance
/// when a command network is up. Read-only: nothing is acquired or rolled.
pub(crate) fn networked_contact_facts(
    world: &World,
    observer: ObjectId,
) -> Result<Vec<NetworkedContactFacts>> {
    let reader = super::contacts::ContactReader::new(world, observer)?;
    let mut contacts: Vec<_> = reader
        .targets()
        .into_iter()
        .filter_map(|target| {
            reader
                .facts(target)
                .ok()
                .flatten()
                .map(|facts| NetworkedContactFacts {
                    facts,
                    relayed: !reader.own(target),
                    network_range: None,
                })
        })
        .collect();
    let Some(network) = NetworkSightings::new(world, observer)? else {
        return Ok(contacts);
    };
    for contact in &mut contacts {
        contact.network_range = Some(
            network
                .range(contact.facts.target, contact.facts.range.spatial)?
                .distance,
        );
    }
    Ok(contacts)
}
