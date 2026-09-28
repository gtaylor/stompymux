//! Peer sightings folded into the ordinary contact display and the autopilot's tactical
//! picture: what the active command network sees, and the shared aiming range it supplies.
use super::contacts::{BattleContactFacts, ContactReader};
use super::network_unit::unit as network_unit;
use super::{BattleNetworkRange, BattlePower};
use crate::{BattleCommandNetwork, ObjectId, World};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};

/// One observer's usable networks for the current display: every family that can
/// currently supply sightings, in the priority order aiming uses for shared range.
pub(super) struct NetworkSightings<'w> {
    world: &'w World,
    observer: ObjectId,
    /// Active peers per family; the first entry decides the shared aiming distance.
    families: Vec<(BattleCommandNetwork, Vec<ObjectId>)>,
    readers: BTreeMap<ObjectId, ContactReader<'w>>,
}

impl<'w> NetworkSightings<'w> {
    /// The observer's active networks, or None when no network can currently supply data
    /// (unlinked, shut down, or jammed).
    pub(super) fn new(world: &'w World, observer: ObjectId) -> Result<Option<Self>> {
        let unit = network_unit(world, observer)?;
        if (unit.c3_network.is_none() && unit.c3i_network.is_none())
            || unit.power() != BattlePower::Running
            || super::electronic_field(world, observer)?.blocks_outgoing_guidance()
        {
            return Ok(None);
        }
        let mut families = Vec::new();
        for kind in [BattleCommandNetwork::C3, BattleCommandNetwork::C3i] {
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
            readers: BTreeMap::new(),
        }))
    }

    /// Whether any peer in any family currently sees `target`, and whether one identifies it.
    pub(super) fn identified(&mut self, target: ObjectId) -> Result<Option<bool>> {
        let mut seen = None;
        for &peer in self.families.iter().flat_map(|(_, peers)| peers) {
            if peer == target {
                continue;
            }
            let reader = match self.readers.entry(peer) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(ContactReader::new(self.world, peer)?)
                }
            };
            if let Some(facts) = reader.facts(target)? {
                seen = Some(seen.unwrap_or(false) || facts.identified);
            }
        }
        Ok(seen)
    }

    /// Every active peer across all families, without duplicates.
    fn peers(&self) -> BTreeSet<ObjectId> {
        self.families
            .iter()
            .flat_map(|(_, peers)| peers.iter().copied())
            .collect()
    }

    /// Shared aiming distance for `target` from the priority family, given the observer's
    /// own physical distance.
    pub(super) fn range(&self, target: ObjectId, physical: f64) -> Result<BattleNetworkRange> {
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
    pub(crate) facts: BattleContactFacts,
    /// Held only by network peers; the observer cannot lock or fire on it directly.
    pub(crate) relayed: bool,
    /// Shared aiming distance, when the observer has an active network.
    pub(crate) network_range: Option<f64>,
}

/// The observer's own acquired contacts plus every target an active network peer has acquired,
/// each carrying the shared aiming distance. Read-only: nothing is acquired or rolled.
pub(crate) fn networked_contact_facts(
    world: &World,
    observer: ObjectId,
) -> Result<Vec<NetworkedContactFacts>> {
    let mut contacts: Vec<_> = super::contacts::acquired_contact_facts(world, observer)?
        .into_iter()
        .map(|facts| NetworkedContactFacts {
            facts,
            relayed: false,
            network_range: None,
        })
        .collect();
    let Some(mut network) = NetworkSightings::new(world, observer)? else {
        return Ok(contacts);
    };
    let team = super::scanner::scanner_unit(world, observer)
        .context("Unit is unavailable")?
        .signature
        .team;
    let direct: BTreeSet<_> = contacts.iter().map(|c| c.facts.target).collect();
    // Only a peer's own acquired contacts can be relayed, so they bound the search.
    let candidates: BTreeSet<_> = network
        .peers()
        .into_iter()
        .filter_map(|peer| super::scanner::scanner_unit(world, peer))
        .flat_map(|peer| peer.contacts.keys().copied())
        .filter(|target| *target != observer && !direct.contains(target))
        .collect();
    for target in candidates {
        if let Some(identified) = network.identified(target)? {
            contacts.push(NetworkedContactFacts {
                facts: super::contacts::relayed_facts(world, observer, team, target, identified)?,
                relayed: true,
                network_range: None,
            });
        }
    }
    for contact in &mut contacts {
        contact.network_range = Some(
            network
                .range(contact.facts.target, contact.facts.range.spatial)?
                .distance,
        );
    }
    Ok(contacts)
}
