//! Communication service dependencies and shared channel operations.

use super::{Access, CHANNEL_NAME_LIMIT, Channel, ChannelId};
use crate::{
    config::Config,
    runtime::{Effects, Outbox, SharedWorld},
    world::{Kind, ObjectId},
};
use anyhow::{Result, ensure};

/// Host callbacks needed by communication policy, independent of a scripting engine.
pub trait HostCallbacks {
    /// Evaluate an attached channel-object lock.
    fn lock_passes(
        &self,
        object: ObjectId,
        who: ObjectId,
        lock: crate::LockType,
    ) -> Result<LockOutcome>;
    /// Return a connection's negotiated terminal width.
    fn terminal_width(&self, session: Option<u64>) -> Option<usize>;
    /// Return the shortest idle interval across an object's sessions.
    fn idle(&self, who: ObjectId) -> u64;
    /// Run a channel leave event inside the host transaction.
    fn channel_leave(&self, world: &SharedWorld, object: ObjectId, who: ObjectId) -> Result<()>;
}

/// Result of invoking a prepared channel lock callback.
pub enum LockOutcome {
    /// The callback completed and returned its access decision.
    Decision(bool),
    /// The prepared callback failed while executing game code.
    CallbackFailed(anyhow::Error),
}

/// Shared transaction dependencies for channel and paging behavior.
pub struct Service<'a> {
    /// Serialized candidate world shared by native and scripted operations.
    pub world: &'a SharedWorld,
    /// Object-directed output staged until commit.
    pub outbox: &'a Outbox,
    /// Runtime-owned transaction effects and typed savepoints.
    pub effects: &'a Effects,
    /// Effective communication policy and output limits.
    pub config: Config,
    /// Narrow host adapter for locks, sessions, and game callbacks.
    pub host: &'a dyn HostCallbacks,
}

impl Service<'_> {
    /// Resolve the canonical channel name without creating it.
    pub fn name(&self, name: &str) -> Result<String> {
        self.world
            .borrow()
            .channels
            .keys()
            .find(|candidate| candidate.eq_ignore_ascii_case(name))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Unknown channel {name}."))
    }

    /// Resolve a generation-sensitive handle; removed identities never revive.
    pub fn by_id(&self, id: ChannelId) -> Result<String> {
        self.world
            .borrow()
            .channels
            .values()
            .find(|channel| channel.id == id)
            .map(|channel| channel.name.clone())
            .ok_or_else(|| anyhow::anyhow!("channel handle is stale"))
    }

    /// Validate the legacy name grammar and allocate a private channel.
    pub fn create(&self, name: &str) -> Result<ChannelId> {
        ensure!(!name.is_empty(), "You must specify a channel to create.");
        ensure!(
            name.len() <= CHANNEL_NAME_LIMIT && name.bytes().all(|byte| (33..=126).contains(&byte)),
            "Channel names must be printable ASCII without spaces."
        );
        ensure!(self.name(name).is_err(), "Channel already exists.");
        let channel = Channel::new(name.into());
        let id = channel.id;
        self.world
            .borrow_mut()
            .channels
            .insert(name.into(), channel);
        Ok(id)
    }

    /// Remove channel state while retaining player-owned aliases and macros.
    pub fn destroy(&self, name: &str) -> Result<()> {
        let name = self.name(name)?;
        self.world.borrow_mut().channels.remove(&name);
        Ok(())
    }

    /// Stage bounded styled output for delivery after transaction commit.
    pub fn notify(&self, player: ObjectId, message: impl Into<String>) -> Result<()> {
        let message = message.into();
        if message.is_empty() {
            return Ok(());
        }
        let mut out = self.outbox.borrow_mut();
        ensure!(
            message.len() <= self.config.runtime.output_message_limit
                && out.len() < self.config.lua.output_entry_limit
                && out
                    .iter()
                    .map(|(_, document)| document.len())
                    .sum::<usize>()
                    + message.len()
                    <= self.config.lua.output_byte_limit,
            "Communication output limit exceeded"
        );
        out.push((player, message.into()));
        Ok(())
    }

    /// Apply Wizard bypass, callback locks, and independent legacy access bits.
    pub fn allowed(&self, who: ObjectId, channel: &str, access: Access) -> Result<bool> {
        let (bits, object, player) = {
            let world = self.world.borrow();
            if super::wizard(&world, who) {
                return Ok(true);
            }
            let object = world
                .objects
                .get(&who)
                .ok_or_else(|| anyhow::anyhow!("object does not exist"))?;
            let channel = world
                .channels
                .get(channel)
                .ok_or_else(|| anyhow::anyhow!("channel removed by callback"))?;
            (channel.flags.0, channel.object, object.kind == Kind::Player)
        };
        if let Some(object) = object.filter(|id| id.0 != 0) {
            let before = self.world.borrow().clone();
            let checkpoint = self.effects.checkpoint();
            match self.host.lock_passes(object, who, access.lock())? {
                LockOutcome::Decision(true) => return Ok(true),
                LockOutcome::Decision(false) => {}
                LockOutcome::CallbackFailed(error) => {
                    *self.world.borrow_mut() = before;
                    self.effects.restore(checkpoint);
                    self.config.log(
                        &[crate::logging::Category::Bugs],
                        "LUA",
                        "ERROR",
                        format!("Channel lock {}: {error}", access.lock().key()),
                    );
                }
            }
        }
        Ok(bits & (access.bit() * if player { 1 } else { 16 }) != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{communication::ChannelFlags, world::World};
    use std::{cell::RefCell, path::Path, rc::Rc};

    struct FakeHost {
        setup_error: bool,
    }

    impl HostCallbacks for FakeHost {
        fn lock_passes(
            &self,
            _object: ObjectId,
            _who: ObjectId,
            _lock: crate::LockType,
        ) -> Result<LockOutcome> {
            if self.setup_error {
                anyhow::bail!("lock adapter unavailable");
            }
            Ok(LockOutcome::CallbackFailed(anyhow::anyhow!(
                "game callback failed"
            )))
        }

        fn terminal_width(&self, _session: Option<u64>) -> Option<usize> {
            None
        }

        fn idle(&self, _who: ObjectId) -> u64 {
            0
        }

        fn channel_leave(
            &self,
            _world: &SharedWorld,
            _object: ObjectId,
            _who: ObjectId,
        ) -> Result<()> {
            Ok(())
        }
    }

    struct Fixture {
        config: Config,
        world: SharedWorld,
        outbox: Outbox,
        effects: Effects,
        player: ObjectId,
    }

    impl Fixture {
        fn service<'a>(&'a self, host: &'a FakeHost) -> Service<'a> {
            Service {
                world: &self.world,
                outbox: &self.outbox,
                effects: &self.effects,
                config: self.config.clone(),
                host,
            }
        }
    }

    fn fixture() -> Fixture {
        let config =
            Config::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"))
                .unwrap();
        let mut world = World::default();
        let player = world.create(&config, "Player".into(), Kind::Player);
        let lock_object = world.create(&config, "Lock".into(), Kind::Thing);
        let mut channel = Channel::new("Test".into());
        channel.flags = ChannelFlags(Access::Join.bit());
        channel.object = Some(lock_object);
        world.channels.insert(channel.name.clone(), channel);
        let outbox = Outbox::default();
        Fixture {
            world: Rc::new(RefCell::new(world)),
            effects: Effects::new(&config, &outbox),
            outbox,
            config,
            player,
        }
    }

    #[test]
    fn lock_setup_errors_propagate_but_callback_failures_use_access_bits() {
        let failed = FakeHost { setup_error: false };
        let fixture = fixture();
        assert!(
            fixture
                .service(&failed)
                .allowed(fixture.player, "Test", Access::Join)
                .unwrap()
        );

        let unavailable = FakeHost { setup_error: true };
        assert!(
            fixture
                .service(&unavailable)
                .allowed(fixture.player, "Test", Access::Join)
                .is_err()
        );
    }
}
