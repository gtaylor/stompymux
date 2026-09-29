//! Automatic command-network formation: eligible units are linked into same-map, same-team
//! networks each tick, so pilots and autopilots never manage membership by hand.
use super::command_network::BattleCommandNetwork;
use super::network_unit::{set_link, units as network_units};
use super::{BattleNotice, BattlePower};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Per-unit opt-out from automatic linking, one flag per network family.
/// Both default to enabled; `c3 -` or `c3i -` disables a family until `c3 +` or `c3i +`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleNetworkAutomation {
    pub c3: bool,
    pub c3i: bool,
}

impl Default for BattleNetworkAutomation {
    fn default() -> Self {
        Self {
            c3: true,
            c3i: true,
        }
    }
}

impl BattleNetworkAutomation {
    /// Whether the given family may be managed automatically.
    pub fn allows(self, kind: BattleCommandNetwork) -> bool {
        match kind {
            BattleCommandNetwork::C3 => self.c3,
            BattleCommandNetwork::C3i => self.c3i,
        }
    }
}

/// Facts about one unit for one family, gathered once per pass; hardware scans are the
/// expensive part, so both families read the same scan.
struct Candidate {
    id: ObjectId,
    map: ObjectId,
    team: i32,
    link: Option<u64>,
    /// Physically able to participate: placed, not going, computer operational.
    eligible: bool,
    /// May be linked by the server, given a running reactor and enabled automation.
    joinable: bool,
    working_masters: usize,
}

/// One network under construction during a pass.
struct Group {
    link: u64,
    members: Vec<ObjectId>,
    working_masters: usize,
    /// Groups created by this pass exist only once they hold two members.
    fresh: bool,
}

impl Group {
    /// Seats the family allows given the masters present.
    fn limit(kind: BattleCommandNetwork, working_masters: usize) -> usize {
        match kind {
            BattleCommandNetwork::C3 => (1 + 3 * working_masters).min(12),
            BattleCommandNetwork::C3i => 6,
        }
    }

    /// Whether `candidate` fits under the family rules after joining.
    fn admits(&self, kind: BattleCommandNetwork, candidate: &Candidate) -> bool {
        self.members.len() < Self::limit(kind, self.working_masters + candidate.working_masters)
    }

    /// Spare seats, used to fill the roomiest network first.
    fn room(&self, kind: BattleCommandNetwork) -> usize {
        Self::limit(kind, self.working_masters).saturating_sub(self.members.len())
    }
}

/// Bring both families up to date: drop dead or singleton links, then link every joinable unit.
/// Existing memberships are never rearranged, so manual joins survive the pass.
pub fn reconcile(world: &mut World) -> Result<Vec<BattleNotice>> {
    let mut notices = Vec::new();
    for kind in [BattleCommandNetwork::C3, BattleCommandNetwork::C3i] {
        notices.extend(reconcile_for(world, kind)?);
    }
    Ok(notices)
}

/// Reconcile one family; see [`reconcile`].
pub(super) fn reconcile_for(
    world: &mut World,
    kind: BattleCommandNetwork,
) -> Result<Vec<BattleNotice>> {
    let mut candidates = candidates(world, kind)?;
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    for id in stale_links(&candidates, kind) {
        set_link(world, id, kind, None);
        candidates.iter_mut().find(|c| c.id == id).unwrap().link = None;
    }
    let mut next_link = candidates
        .iter()
        .filter_map(|c| c.link)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .with_context(|| format!("{} network identities exhausted", kind.name()))?;
    let mut notices = Vec::new();
    for pool in pools(&candidates).values() {
        let mut groups = BTreeMap::<u64, Group>::new();
        for candidate in pool {
            let Some(link) = candidate.link else {
                continue;
            };
            let group = groups.entry(link).or_insert_with(|| Group {
                link,
                members: Vec::new(),
                working_masters: 0,
                fresh: false,
            });
            group.members.push(candidate.id);
            group.working_masters += candidate.working_masters;
        }
        // Masters first so they seed classic networks before slaves look for one.
        let mut unlinked: Vec<_> = pool
            .iter()
            .filter(|c| c.link.is_none() && c.joinable)
            .collect();
        unlinked.sort_by_key(|c| (c.working_masters == 0, c.id));
        let mut joined = Vec::new();
        for candidate in unlinked {
            let target = groups
                .values()
                .filter(|group| group.admits(kind, candidate))
                .max_by_key(|group| (group.room(kind), std::cmp::Reverse(group.link)))
                .map(|group| group.link);
            let link = match target {
                Some(link) => link,
                // A lone unit only seeds a network when the family lets it carry peers.
                None if kind == BattleCommandNetwork::C3 && candidate.working_masters == 0 => {
                    continue;
                }
                None => {
                    let link = next_link;
                    next_link = next_link
                        .checked_add(1)
                        .with_context(|| format!("{} network identities exhausted", kind.name()))?;
                    groups.insert(
                        link,
                        Group {
                            link,
                            members: Vec::new(),
                            working_masters: 0,
                            fresh: true,
                        },
                    );
                    link
                }
            };
            let group = groups.get_mut(&link).unwrap();
            group.members.push(candidate.id);
            group.working_masters += candidate.working_masters;
            joined.push((candidate.id, link));
        }
        for group in groups.values() {
            let newcomers: Vec<_> = joined
                .iter()
                .filter(|(_, link)| *link == group.link)
                .map(|(id, _)| *id)
                .collect();
            if group.members.len() < 2 || newcomers.is_empty() {
                continue;
            }
            for &id in &newcomers {
                set_link(world, id, kind, Some(group.link));
            }
            notices.extend(announce(world, kind, group, &newcomers)?);
        }
    }
    super::command_network::validate(world)?;
    Ok(notices)
}

/// Group eligible units by map and team; every pair is an independent pool.
fn pools(candidates: &[Candidate]) -> BTreeMap<(ObjectId, i32), Vec<&Candidate>> {
    let mut pools = BTreeMap::<_, Vec<_>>::new();
    for candidate in candidates.iter().filter(|c| c.eligible) {
        pools
            .entry((candidate.map, candidate.team))
            .or_default()
            .push(candidate);
    }
    pools
}

/// Links that no longer connect their unit to anyone: ineligible units, networks left with
/// one member, and classic members beyond what the surviving masters can carry.
fn stale_links(candidates: &[Candidate], kind: BattleCommandNetwork) -> Vec<ObjectId> {
    let mut stale: Vec<_> = candidates
        .iter()
        .filter(|c| c.link.is_some() && !c.eligible)
        .map(|c| c.id)
        .collect();
    let mut groups = BTreeMap::<(ObjectId, i32, u64), Vec<&Candidate>>::new();
    for candidate in candidates.iter().filter(|c| c.eligible) {
        if let Some(link) = candidate.link {
            groups
                .entry((candidate.map, candidate.team, link))
                .or_default()
                .push(candidate);
        }
    }
    for mut group in groups.into_values() {
        let masters = group.iter().map(|c| c.working_masters).sum();
        let limit = Group::limit(kind, masters);
        // Over capacity, masters keep their seats ahead of slaves, then lower ids win.
        group.sort_by_key(|c| (c.working_masters == 0, c.id));
        let kept = if limit >= 2 { limit } else { 0 };
        stale.extend(group.iter().skip(kept).map(|c| c.id));
        if group.len().min(kept) < 2 {
            stale.extend(group.iter().take(kept).map(|c| c.id));
        }
    }
    stale.sort_unstable();
    stale.dedup();
    stale
}

/// Describe a network change to its members: newcomers learn the size, veterans learn who joined.
fn announce(
    world: &World,
    kind: BattleCommandNetwork,
    group: &Group,
    newcomers: &[ObjectId],
) -> Result<Vec<BattleNotice>> {
    let name = kind.name();
    let size = group.members.len();
    let mut notices = Vec::new();
    for &member in &group.members {
        if group.fresh || newcomers.contains(&member) {
            notices.push(BattleNotice {
                unit: member,
                text: format!("{name} network established with {size} units."),
            });
            continue;
        }
        for &newcomer in newcomers {
            notices.push(BattleNotice {
                unit: member,
                text: format!(
                    "{} connects to your {name} network.",
                    super::command_network::display_id(world, member, newcomer)?
                ),
            });
        }
    }
    Ok(notices)
}

/// Gather facts for every placed unit carrying this family's hardware.
fn candidates(world: &World, kind: BattleCommandNetwork) -> Result<Vec<Candidate>> {
    let mut result = Vec::new();
    for (id, unit) in network_units(world)? {
        let Some(position) = unit.position() else {
            continue;
        };
        let hardware = unit.c3_hardware()?;
        let (installed, operational) = match kind {
            BattleCommandNetwork::C3 => (
                hardware.masters > 0 || hardware.slave_installed,
                unit.c3_operational()?,
            ),
            BattleCommandNetwork::C3i => (hardware.c3i_installed, hardware.c3i_operational),
        };
        if !installed {
            continue;
        }
        let going = world
            .objects
            .get(&id)
            .is_none_or(|object| object.flags.contains(Flag::Going));
        let eligible = !going && operational;
        result.push(Candidate {
            id,
            map: position.map,
            team: unit.signature().team,
            link: kind.link(&unit),
            eligible,
            joinable: eligible
                && unit.power() == BattlePower::Running
                && unit.automation.allows(kind),
            working_masters: hardware.working_masters,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: i64, link: Option<u64>, masters: usize, eligible: bool) -> Candidate {
        Candidate {
            id: ObjectId(id),
            map: ObjectId(1),
            team: 0,
            link,
            eligible,
            joinable: eligible,
            working_masters: masters,
        }
    }

    #[test]
    fn stale_links_clear_singletons_ineligible_units_and_classic_overflow() {
        let candidates = vec![
            candidate(1, Some(7), 0, true),
            candidate(2, Some(8), 1, true),
            candidate(3, Some(8), 0, true),
            candidate(4, Some(8), 0, true),
            candidate(5, Some(8), 0, true),
            candidate(6, Some(8), 0, true),
            candidate(9, Some(8), 0, false),
        ];
        // One master carries three slaves: the lone unit, the dead one and the fifth slave go.
        assert_eq!(
            stale_links(&candidates, BattleCommandNetwork::C3),
            vec![ObjectId(1), ObjectId(6), ObjectId(9)]
        );
        // C3i ignores masters and only drops the singleton and the dead unit.
        assert_eq!(
            stale_links(&candidates, BattleCommandNetwork::C3i),
            vec![ObjectId(1), ObjectId(9)]
        );
    }

    #[test]
    fn slaves_without_a_master_lose_their_classic_link() {
        let candidates = vec![
            candidate(1, Some(3), 0, true),
            candidate(2, Some(3), 0, true),
        ];
        assert_eq!(
            stale_links(&candidates, BattleCommandNetwork::C3),
            vec![ObjectId(1), ObjectId(2)]
        );
        assert!(stale_links(&candidates, BattleCommandNetwork::C3i).is_empty());
    }

    #[test]
    fn groups_admit_by_family_capacity() {
        let group = Group {
            link: 1,
            members: vec![ObjectId(1); 4],
            working_masters: 1,
            fresh: false,
        };
        let slave = candidate(5, None, 0, true);
        let master = candidate(6, None, 1, true);
        assert!(!group.admits(BattleCommandNetwork::C3, &slave));
        assert!(group.admits(BattleCommandNetwork::C3, &master));
        assert!(group.admits(BattleCommandNetwork::C3i, &slave));
        assert_eq!(group.room(BattleCommandNetwork::C3), 0);
        assert_eq!(group.room(BattleCommandNetwork::C3i), 2);
    }
}
