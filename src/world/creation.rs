//! Object allocation and creation policy.

use super::{Kind, Object, ObjectId, World};
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

/// Policy source for allocation; foundational objects do not require a creator.
#[derive(Clone, Copy, Debug)]
pub enum CreationContext {
    Bootstrap,
    Player,
    Object {
        creator: ObjectId,
        zone: Option<ObjectId>,
    },
}

impl World {
    /// Resolve creation policy before allocating a dbref.
    pub fn create_with(
        &mut self,
        config: &Config,
        name: String,
        kind: Kind,
        context: CreationContext,
    ) -> Result<ObjectId> {
        let zone = match context {
            CreationContext::Bootstrap => None,
            CreationContext::Player => {
                ensure!(kind == Kind::Player, "Player creation requires player type");
                (config.mux.player_zone > 0).then_some(ObjectId(config.mux.player_zone))
            }
            CreationContext::Object { creator, zone } => {
                ensure!(
                    matches!(kind, Kind::Room | Kind::Thing | Kind::Exit),
                    "Invalid created object type"
                );
                let creator = self
                    .objects
                    .get(&creator)
                    .filter(|object| {
                        object.kind != Kind::Garbage
                            && !object.flags.contains(crate::flags::Flag::Going)
                    })
                    .context("Invalid object creator")?;
                zone.or(creator.zone)
            }
        };
        if let Some(zone) = zone {
            self.validate_zone(zone)?;
        }
        let id = self.create(config, name, kind);
        self.objects.get_mut(&id).unwrap().zone = zone;
        Ok(id)
    }

    /// Foundational allocation without creator policy.
    pub fn create(&mut self, config: &Config, name: String, kind: Kind) -> ObjectId {
        let id = ObjectId(self.next_id);
        self.next_id += 1;
        let (flags, parent) = match kind {
            Kind::Player => (
                &config.mux.default_player_flags,
                &config.mux.default_player_lua_parent,
            ),
            Kind::Room => (
                &config.mux.default_room_flags,
                &config.mux.default_room_lua_parent,
            ),
            Kind::Exit => (
                &config.mux.default_exit_flags,
                &config.mux.default_exit_lua_parent,
            ),
            _ => (
                &config.mux.default_thing_flags,
                &config.mux.default_thing_lua_parent,
            ),
        };
        let flags = flags
            .iter()
            .copied()
            .filter(|flag| *flag != crate::flags::Flag::Connected)
            .collect();
        self.objects.insert(
            id,
            Object {
                generation: Default::default(),
                pending_destroyer: None,
                id,
                name,
                kind,
                location: None,
                zone: None,
                home: None,
                affiliation: None,
                destination: None,
                dropto: None,
                description: None,
                internal_description: None,
                lua_parent: parent.clone(),
                flags,
                powers: Default::default(),
                state: BTreeMap::new(),
            },
        );
        if kind == Kind::Player {
            for &number in &config.mux.default_player_macros {
                let Some(set) = self.macros.sets.get(number) else {
                    config.log(
                        crate::logging::LogLevel::Warn,
                        &[crate::logging::Category::Problems], "MAC", "WARN",
                        format!("Player #{} ({}) created without default macro set {number}: set not found (default_player_macros)", id.0, self.objects[&id].name),
                    );
                    continue;
                };
                let identity = set.id;
                // Default attachments are trusted creation policy, independent of set sharing modes.
                if let Err(error) = crate::macros::service::attach(self, id, identity) {
                    config.log(
                        crate::logging::LogLevel::Warn,
                        &[crate::logging::Category::Problems],
                        "MAC",
                        "WARN",
                        format!(
                            "Player #{}: could not attach default macro set {number}: {}",
                            id.0, error.1
                        ),
                    );
                }
            }
        }
        id
    }
}
