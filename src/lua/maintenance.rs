//! Immediate semantic repair with effects retained until the outer world commit.
use super::{Scripts, packages::error::failure, transactions};
use crate::world::{Kind, ObjectId};

/// Apply C-visible repair and callbacks now; SQL and session effects remain staged.
pub(crate) fn check(lua: &mlua::Lua) -> mlua::Result<()> {
    transactions::require(lua)?;
    let s = Scripts::services(lua)?;
    transactions::run(lua, &s.world, &s.outbox, || {
        let config = super::configuration(lua);
        let before = s.world.borrow().clone();
        let links = crate::dbck::rebuild_links(&before, &before.links);
        let (repaired, mut report) =
            crate::dbck::plan(&before, &links, &config).map_err(|e| failure("mux.runtime", e))?;
        *s.world.borrow_mut() = repaired;
        for relocation in &report.plan.relocations {
            let movement = crate::movement::Move {
                actor: ObjectId(1),
                object: relocation.object,
                source: relocation.source,
                destination: relocation.destination,
                session: None,
            };
            if let Some(source) = relocation.source.filter(|id| {
                s.world
                    .borrow()
                    .objects
                    .get(id)
                    .is_some_and(|o| matches!(o.kind, Kind::Room | Kind::Thing | Kind::Player))
            }) {
                s.world
                    .borrow_mut()
                    .objects
                    .get_mut(&relocation.object)
                    .ok_or_else(|| {
                        failure("mux.object.invalid", "callback removed repaired object")
                    })?
                    .location = Some(source);
                s.movement_event("on_exit", source, &movement)
                    .map_err(|e| failure("mux.runtime", e))?;
            }
            s.world
                .borrow_mut()
                .objects
                .get_mut(&relocation.object)
                .ok_or_else(|| failure("mux.object.invalid", "callback removed repaired object"))?
                .location = Some(relocation.destination);
            s.movement_event("on_enter", relocation.destination, &movement)
                .map_err(|e| failure("mux.runtime", e))?;
        }
        s.world
            .borrow()
            .validate(&config)
            .map_err(|e| failure("mux.runtime", e))?;
        report.plan.links = crate::dbck::rebuild_links(&s.world.borrow(), &links);
        s.world.borrow_mut().links = report.plan.links.clone();
        s.flows.stage_maintenance(report);
        Ok(())
    })
}
