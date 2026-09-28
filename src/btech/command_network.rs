//! Command-network membership with shared identities, family-specific capacity, and transactional controls.
use super::network_unit::{set_link, unit as network_unit, units as network_units};
use super::{BattleNotice, BattlePower};
use crate::{Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

/// Independent command-computer network families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BattleCommandNetwork {
    C3,
    C3i,
}

impl BattleCommandNetwork {
    /// Player-facing network name.
    pub fn name(self) -> &'static str {
        match self {
            Self::C3 => "C3",
            Self::C3i => "C3i",
        }
    }
    /// Absolute network size, including the requesting unit.
    fn maximum(self) -> usize {
        match self {
            Self::C3 => 12,
            Self::C3i => 6,
        }
    }
    /// Read the identity in this family's independent namespace.
    pub(super) fn link(self, unit: &super::network_unit::NetworkUnit<'_>) -> Option<u64> {
        match self {
            Self::C3 => unit.c3_network,
            Self::C3i => unit.c3i_network,
        }
    }
    /// Hardware remains independent of power and interference.
    fn operational(self, unit: &super::network_unit::NetworkUnit<'_>) -> Result<bool> {
        match self {
            Self::C3 => unit.c3_operational(),
            Self::C3i => Ok(unit.c3_hardware()?.c3i_operational),
        }
    }
    /// Physical installation can survive functional loss.
    fn installed(self, unit: &super::network_unit::NetworkUnit<'_>) -> Result<bool> {
        let h = unit.c3_hardware()?;
        Ok(match self {
            Self::C3 => h.masters > 0 || h.slave_installed,
            Self::C3i => h.c3i_installed,
        })
    }
    /// A master supplies three peer slots, capped at eleven, plus the network's first unit.
    fn capacity(self, world: &World, members: &[ObjectId]) -> Result<usize> {
        if self == Self::C3i {
            return Ok(6);
        }
        let masters = members
            .iter()
            .try_fold(0usize, |sum, id| -> Result<usize> {
                Ok(sum + network_unit(world, *id)?.c3_hardware()?.working_masters)
            })?;
        Ok((1 + masters * 3).min(self.maximum()))
    }
}

/// Current physical eligibility; reactor shutdown and interference retain membership.
fn eligible(world: &World, id: ObjectId, kind: BattleCommandNetwork) -> bool {
    world
        .objects
        .get(&id)
        .is_some_and(|o| !o.flags.contains(Flag::Going))
        && network_unit(world, id)
            .is_ok_and(|unit| unit.position().is_some() && kind.operational(&unit).unwrap_or(false))
}

/// Inspect all eligible members, including the caller, or an empty list when disconnected.
/// Shutdown and ECM do not remove a member; targeting applies its own availability checks.
pub fn members_for(
    world: &World,
    id: ObjectId,
    kind: BattleCommandNetwork,
) -> Result<Vec<ObjectId>> {
    let unit = network_unit(world, id)?;
    let Some(network) = kind.link(&unit) else {
        return Ok(vec![]);
    };
    if !eligible(world, id, kind) {
        return Ok(vec![]);
    }
    let mut result = Vec::new();
    for (other, peer) in network_units(world)? {
        if kind.link(&peer) == Some(network)
            && eligible(world, other, kind)
            && peer.position().map(|p| p.map) == unit.position().map(|p| p.map)
            && peer.signature().team == unit.signature().team
        {
            result.push(other);
        }
    }
    trim(world, id, kind, result, false)
}

/// Reapply working-master capacity to an already filtered membership set.
fn trim(
    world: &World,
    id: ObjectId,
    kind: BattleCommandNetwork,
    mut result: Vec<ObjectId>,
    retain_requester: bool,
) -> Result<Vec<ObjectId>> {
    if kind == BattleCommandNetwork::C3 {
        let capacity = kind.capacity(world, &result)?;
        if result.len() > capacity {
            let mut ranked = result
                .iter()
                .map(|id| {
                    Ok((
                        *id,
                        network_unit(world, *id)?.c3_hardware()?.working_masters == 0,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            ranked.sort_by_key(|(member, slave)| {
                (!retain_requester || *member != id, *slave, *member)
            });
            result = ranked
                .into_iter()
                .take(capacity)
                .map(|(id, _)| id)
                .collect();
            result.sort_unstable();
        }
    }
    if result.len() < 2 || !result.contains(&id) {
        result.clear();
    }
    Ok(result)
}

/// Remove dead membership and singleton identities before allocating or changing a network.
fn normalize(world: &mut World, kind: BattleCommandNetwork) -> Result<()> {
    let disconnected = network_units(world)?
        .keys()
        .copied()
        .map(|id| Ok((id, members_for(world, id, kind)?.is_empty())))
        .collect::<Result<Vec<_>>>()?;
    for (id, clear) in disconnected {
        if clear {
            set_link(world, id, kind, None);
        }
    }
    Ok(())
}

/// Reject over-capacity or conflicting shared identities on database load.
pub(super) fn validate(world: &World) -> Result<()> {
    for kind in [BattleCommandNetwork::C3, BattleCommandNetwork::C3i] {
        let mut groups = BTreeMap::<_, Vec<_>>::new();
        for (_, unit) in network_units(world)? {
            if let Some(network) = kind.link(&unit) {
                groups.entry(network).or_default().push(unit);
            }
        }
        for group in groups.values() {
            ensure!(
                group.len() <= kind.maximum(),
                "{} network exceeds capacity",
                kind.name()
            );
            let first = &group[0];
            ensure!(
                group.iter().all(|unit| unit.position().map(|p| p.map)
                    == first.position().map(|p| p.map)
                    && unit.signature().team == first.signature().team),
                "{} network crosses maps or teams",
                kind.name()
            );
        }
    }
    Ok(())
}

/// Pilot intent for one network control command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleNetworkRequest {
    /// Join the named unit's network; automatic management is left as it was.
    Join(ObjectId),
    /// Disconnect and hold the unit out of automatic management.
    Leave,
    /// Resume automatic management and link immediately where possible.
    Automatic,
}

/// Apply a pilot's request: manual join, held disconnect, or return to automatic linking.
/// All admission, capacity checks, and membership edits succeed or roll back together.
pub fn request_for(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    request: BattleNetworkRequest,
    kind: BattleCommandNetwork,
) -> Result<Vec<BattleNotice>> {
    match request {
        BattleNetworkRequest::Join(target) => join_leave_for(world, id, pilot, Some(target), kind),
        BattleNetworkRequest::Leave => {
            let mut notices = join_leave_for(world, id, pilot, None, kind)?;
            super::network_unit::set_automation(world, id, kind, false);
            let name = kind.name();
            notices.push(BattleNotice {
                unit: id,
                text: format!(
                    "Automatic {name} linking is off until you run {} +.",
                    name.to_ascii_lowercase()
                ),
            });
            Ok(notices)
        }
        BattleNetworkRequest::Automatic => {
            ready_for(world, id, pilot, kind)?;
            let mut candidate = world.clone();
            super::network_unit::set_automation(&mut candidate, id, kind, true);
            let mut notices = super::network_topology::reconcile_for(&mut candidate, kind)?;
            if kind.link(&network_unit(&candidate, id)?).is_none() {
                notices.push(BattleNotice {
                    unit: id,
                    text: format!(
                        "Automatic {} linking resumed; no network is available yet.",
                        kind.name()
                    ),
                });
            }
            *world = candidate;
            Ok(notices)
        }
    }
}

/// Join a visible friendly unit's network, or disconnect with None, without publishing notices.
/// All admission, capacity checks, and membership edits succeed or roll back together.
pub fn join_leave_for(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
    kind: BattleCommandNetwork,
) -> Result<Vec<BattleNotice>> {
    ready_for(world, id, pilot, kind)?;
    let name = kind.name();
    let unit = network_unit(world, id)?;
    let mut candidate = world.clone();
    normalize(&mut candidate, kind)?;
    let Some(target) = target else {
        let connected = kind.link(&network_unit(&candidate, id)?).is_some();
        set_link(&mut candidate, id, kind, None);
        normalize(&mut candidate, kind)?;
        *world = candidate;
        return Ok(vec![BattleNotice {
            unit: id,
            text: if connected {
                format!("You disconnect from the {name} network.")
            } else {
                format!("You are not connected to a {name} network!")
            },
        }]);
    };
    ensure!(
        kind.link(&network_unit(&candidate, id)?).is_none(),
        "You are already in a {name} network!"
    );
    ensure!(target != id, "You can't connect to yourself!");
    super::visible_contact(&candidate, id, target)?
        .context("That is not a valid targetID. Try again.")?;
    let peer = network_unit(&candidate, target)?;
    ensure!(
        peer.signature().team == unit.signature().team,
        "You can't use the {name} network of unfriendly units!"
    );
    ensure!(
        peer.power() == BattlePower::Running,
        "That unit is not started!"
    );
    ensure!(
        eligible(&candidate, target, kind),
        "That unit does not appear to be equipped with working {name}!"
    );
    let mut group = members_for(&candidate, target, kind)?;
    if group.is_empty() {
        group.push(target);
    }
    let mut proposed = group.clone();
    proposed.push(id);
    ensure!(
        proposed.len() <= kind.capacity(&candidate, &proposed)?,
        "That unit's {name} network is operating at maximum capacity!"
    );
    let network = match kind.link(&peer) {
        Some(network) => network,
        None => network_units(&candidate)?
            .values()
            .filter_map(|u| kind.link(u))
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .with_context(|| format!("{name} network identities exhausted"))?,
    };
    let mut notices = vec![BattleNotice {
        unit: id,
        text: format!(
            "You connect to {}'s {name} network.",
            display_id(&candidate, id, target)?
        ),
    }];
    for &recipient in &group {
        let peer = network_unit(&candidate, recipient)?;
        if recipient == target
            || (peer.power() == BattlePower::Running
                && !super::electronic_field(&candidate, recipient)?.blocks_outgoing_guidance()
                && !peer
                    .pilot()
                    .is_some_and(|pilot| candidate.btech.unconscious(pilot)))
        {
            notices.push(BattleNotice {
                unit: recipient,
                text: format!(
                    "{} connects to your {name} network.",
                    display_id(&candidate, recipient, id)?
                ),
            });
        }
    }
    group.push(id);
    for member in group {
        set_link(&mut candidate, member, kind, Some(network));
    }
    validate(&candidate)?;
    *world = candidate;
    Ok(notices)
}

/// Resolve native/Lua target labels and publish membership changes under one checkpoint.
pub fn control(
    scripts: &Scripts,
    id: ObjectId,
    pilot: ObjectId,
    arguments: &str,
    kind: BattleCommandNetwork,
) -> Result<Vec<BattleNotice>> {
    let words: Vec<_> = arguments.split_whitespace().collect();
    ensure!(words.len() == 1, "Invalid number of arguments to function!");
    let request = match words[0] {
        "-" => BattleNetworkRequest::Leave,
        "+" => BattleNetworkRequest::Automatic,
        label => {
            let world = scripts.world();
            BattleNetworkRequest::Join(super::radio_targeted::target(&world, id, label)?)
        }
    };
    scripts.atomic(|_| {
        let notices = request_for(&mut scripts.world_mut(), id, pilot, request, kind)?;
        for notice in &notices {
            super::notify_unit(scripts, notice.clone())?;
        }
        Ok(notices)
    })
}

/// Native network join/leave commands use the invoking player's cockpit.
fn command_for(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
    kind: BattleCommandNetwork,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        control(ctx.scripts, id, ctx.player, &input.args, kind)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Network notifications disclose names only with a clear terrain sightline.
pub(super) fn display_id(world: &World, observer: ObjectId, subject: ObjectId) -> Result<String> {
    let unit = network_unit(world, subject)?;
    let clear = super::visibility::unit_unblocked(world, observer, subject)?;
    let mut label = unit
        .battlefield_id()
        .context("Unit has no battlefield ID")?;
    if clear && unit.signature().team == network_unit(world, observer)?.signature().team {
        label.make_ascii_lowercase();
    }
    Ok(format!(
        "{} [{}]",
        if clear { unit.name() } else { "something" },
        label
    ))
}

/// Shared admission for pilot-operated command-network controls.
pub(super) fn ready_for(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    kind: BattleCommandNetwork,
) -> Result<()> {
    if world.btech.vehicles().contains_key(&id) {
        super::vehicle_power::controlled(world, id, pilot)?;
    } else {
        super::power::controlled_unit(world, id, pilot)?;
    }
    let unit = network_unit(world, id)?;
    ensure!(unit.power() == BattlePower::Running, "Start the unit first");
    ensure!(
        kind.installed(&unit)?,
        "This unit is not equipped with {}!",
        kind.name()
    );
    ensure!(
        eligible(world, id, kind),
        "Your {} system is destroyed!",
        kind.name()
    );
    ensure!(
        !super::electronic_field(world, id)?.blocks_outgoing_guidance(),
        "Your {} system is not currently operational!",
        kind.name()
    );
    Ok(())
}

/// Inspect C3i membership independently of classic C3.
pub fn members(world: &World, id: ObjectId) -> Result<Vec<ObjectId>> {
    members_for(world, id, BattleCommandNetwork::C3i)
}
/// Inspect classic C3 membership with working-master capacity.
pub fn c3_members(world: &World, id: ObjectId) -> Result<Vec<ObjectId>> {
    members_for(world, id, BattleCommandNetwork::C3)
}
/// Join or leave C3i through the shared membership transaction.
pub fn join_leave(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
) -> Result<Vec<BattleNotice>> {
    join_leave_for(world, id, pilot, target, BattleCommandNetwork::C3i)
}
/// Join or leave classic C3 through the shared membership transaction.
pub fn join_leave_c3(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
) -> Result<Vec<BattleNotice>> {
    join_leave_for(world, id, pilot, target, BattleCommandNetwork::C3)
}
/// C3i cockpit control and notification.
pub fn c3i(
    scripts: &Scripts,
    id: ObjectId,
    pilot: ObjectId,
    arguments: &str,
) -> Result<Vec<BattleNotice>> {
    control(scripts, id, pilot, arguments, BattleCommandNetwork::C3i)
}
/// Classic C3 cockpit control and notification.
pub fn c3(
    scripts: &Scripts,
    id: ObjectId,
    pilot: ObjectId,
    arguments: &str,
) -> Result<Vec<BattleNotice>> {
    control(scripts, id, pilot, arguments, BattleCommandNetwork::C3)
}
/// C3i command dispatch.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, BattleCommandNetwork::C3i)
}
/// Classic C3 command dispatch.
pub(crate) fn c3_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, BattleCommandNetwork::C3)
}
/// Members currently able to exchange data, with classic capacity reduced by unavailable masters.
/// This temporary view leaves durable membership unchanged; consciousness is optional for messages.
pub(super) fn active_members(
    world: &World,
    id: ObjectId,
    kind: BattleCommandNetwork,
    check_consciousness: bool,
) -> Result<Vec<ObjectId>> {
    let mut active = Vec::new();
    for member in members_for(world, id, kind)? {
        let unit = network_unit(world, member)?;
        if unit.power() != BattlePower::Running
            || super::electronic_field(world, member)?.blocks_outgoing_guidance()
            || (check_consciousness
                && unit
                    .pilot()
                    .is_some_and(|pilot| world.btech.unconscious(pilot)))
        {
            continue;
        }
        active.push(member);
    }
    trim(world, id, kind, active, true)
}
