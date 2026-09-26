//! Map identity retirement without deleting its world container or other maps' route markers.
use crate::{BtechState, Config, Kind, ObjectId, Scripts, World};
use anyhow::{Result, ensure};
use std::{collections::BTreeSet, sync::Arc};

/// Remove map-owned state and authored configuration references, retaining world-object references.
pub(crate) fn forget(state: &mut BtechState, ids: &BTreeSet<ObjectId>) {
    state.maps.retain(|id, _| !ids.contains(id));
    Arc::make_mut(&mut state.registrations).retain(|id, _| !ids.contains(id));
    for map in state.maps.values_mut() {
        if map
            .authored_link
            .is_some_and(|link| ids.contains(&link.parent))
        {
            map.authored_link = None;
        }
    }
}

/// Identify map-role removals separately from ordinary object purges and verify detached membership.
pub(crate) fn removed(before: &World, after: &World) -> Result<BTreeSet<ObjectId>> {
    let ids: BTreeSet<_> = before
        .btech
        .maps()
        .keys()
        .copied()
        .filter(|id| {
            !after.btech.maps().contains_key(id)
                && after
                    .objects
                    .get(id)
                    .is_some_and(|object| object.kind != Kind::Garbage)
        })
        .collect();
    for id in &ids {
        ensure!(
            after.btech.registrations().get(id).map(String::as_str) != Some("MAP"),
            "Removed map retains registration"
        );
        ensure!(
            !after
                .btech
                .units()
                .values()
                .any(|unit| unit.map == Some(*id)),
            "Removed map retains units"
        );
    }
    Ok(ids)
}

/// Shut down tactical occupants, then retire map ownership in one rollback checkpoint.
pub(super) fn unregister(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
) -> Result<()> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(&before, actor)
                && crate::authority::controls(&before, actor, map),
            "permission denied."
        );
        super::map_clear::teardown(scripts, config, actor, map)?;
        forget(&mut scripts.world_mut().btech, &BTreeSet::from([map]));
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(())
    })
}
