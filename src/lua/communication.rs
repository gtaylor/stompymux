//! Lua-backed host callbacks used by the communication domain.

use super::{Scripts, sessions, transactions};
use crate::{
    communication::{HostCallbacks, LockOutcome},
    runtime::SharedWorld,
    world::ObjectId,
};

impl HostCallbacks for mlua::Lua {
    fn lock_passes(
        &self,
        object: ObjectId,
        who: ObjectId,
        lock: crate::LockType,
    ) -> anyhow::Result<LockOutcome> {
        let function: mlua::Function = self
            .load("return function(o,p,k,c) return mux.world.lock_passes({object=mux.world.object(o),enactor=p,subject=p,cause=c,lock=k}) end")
            .eval()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        match function.call((
            object.0,
            who.0,
            lock,
            transactions::cause(self).unwrap_or(who).0,
        )) {
            Ok(decision) => Ok(LockOutcome::Decision(decision)),
            Err(error) => Ok(LockOutcome::CallbackFailed(anyhow::anyhow!(
                error.to_string()
            ))),
        }
    }

    fn terminal_width(&self, session: Option<u64>) -> Option<usize> {
        sessions::players(self)
            .ok()?
            .sequence_values::<mlua::Table>()
            .filter_map(|row| row.ok())
            .find(|row| row.get::<u64>("session").ok() == session)?
            .get("terminal_width")
            .ok()
    }

    fn idle(&self, who: ObjectId) -> u64 {
        sessions::players(self)
            .ok()
            .map(|players| {
                players
                    .sequence_values::<mlua::Table>()
                    .filter_map(|row| row.ok())
                    .filter(|row| row.get::<i64>("dbref").ok() == Some(who.0))
                    .filter_map(|row| row.get::<u64>("idle_for").ok())
                    .min()
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }

    fn channel_leave(
        &self,
        world: &SharedWorld,
        object: ObjectId,
        who: ObjectId,
    ) -> anyhow::Result<()> {
        let scripts =
            Scripts::services(self).map_err(|error| anyhow::anyhow!(error.to_string()))?;
        transactions::run(self, world, || {
            scripts
                .channel_leave_event(object, who)
                .map_err(mlua::Error::external)
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))
    }
}
