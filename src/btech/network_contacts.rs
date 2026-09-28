//! Peer sightings folded into the ordinary contact display: what the active command network
//! sees, and the shared aiming range it supplies for every listed contact.
use super::contacts::ContactReader;
use super::network_unit::unit as network_unit;
use super::{BattleNetworkRange, BattlePower};
use crate::{BattleCommandNetwork, ObjectId, World};
use anyhow::Result;
use std::collections::BTreeMap;

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
