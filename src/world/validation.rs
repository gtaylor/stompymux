//! Whole-world and configuration-reference validation.

use super::{Kind, ObjectId, World};
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;

impl World {
    /// Validate an available zone before configuration or allocation.
    pub fn validate_zone(&self, zone: ObjectId) -> Result<()> {
        ensure!(
            self.objects.get(&zone).is_some_and(|object| {
                matches!(object.kind, Kind::Room | Kind::Thing)
                    && !object.flags.contains(crate::flags::Flag::Going)
            }),
            "Invalid or unavailable zone #{}",
            zone.0
        );
        Ok(())
    }

    /// Validate a configured positive player-zone default.
    pub fn validate_player_zone(&self, config: &Config) -> Result<()> {
        if config.mux.player_zone > 0 {
            self.validate_zone(ObjectId(config.mux.player_zone))
                .map_err(|error| anyhow::anyhow!("mux.player_zone: {error}"))?;
        }
        Ok(())
    }

    /// Validate that account identities correspond to uniquely named players.
    pub fn validate_accounts(&self) -> Result<()> {
        let mut names = BTreeSet::new();
        for (id, account) in &self.accounts {
            let object = self
                .objects
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("account object missing"))?;
            ensure!(object.kind == Kind::Player, "account is not a player");
            ensure!(
                names.insert(
                    crate::text::plain_with(&self.palette, &object.name).to_ascii_lowercase()
                ),
                "duplicate player name"
            );
            if let Some(alias) = &account.alias
                && !alias.is_empty()
                && !alias
                    .eq_ignore_ascii_case(&crate::text::plain_with(&self.palette, &object.name))
            {
                ensure!(
                    names.insert(alias.to_ascii_lowercase()),
                    "duplicate player alias"
                );
            }
        }
        Ok(())
    }

    /// Validate the complete transactional world projection.
    pub fn validate(&self, config: &Config) -> Result<()> {
        self.btech.validate(self)?;
        self.macros.validate(self)?;
        for id in [config.start(), config.home()] {
            ensure!(
                self.objects
                    .get(&ObjectId(id))
                    .is_some_and(|object| object.kind == Kind::Room),
                "starting room/home #{id} missing or not a room"
            );
        }
        self.validate_accounts()?;
        for object in self
            .objects
            .values()
            .filter(|object| object.kind != Kind::Garbage)
        {
            if !config.retains_state(object) {
                crate::state::validate(&object.state, config)
                    .with_context(|| format!("object #{} state", object.id.0))?;
            }
            let chain = self.containment_chain(object.location)?;
            ensure!(
                !chain.contains(&object.id),
                "Containment cycle at #{}.",
                object.id.0
            );
            ensure!(
                object.kind != Kind::Room || object.location.is_none(),
                "Room #{} has an invalid location.",
                object.id.0
            );
            for id in [
                object.location,
                object.home,
                object.destination,
                object.zone,
                object.affiliation,
                object.dropto,
            ]
            .into_iter()
            .flatten()
            {
                ensure!(
                    self.objects
                        .get(&id)
                        .is_some_and(|target| target.kind != Kind::Garbage),
                    "{} references missing #{}",
                    object.name,
                    id.0
                );
            }
        }
        let mut channel_names = BTreeSet::new();
        for channel in self.channels.values() {
            ensure!(
                channel_names.insert(channel.name.to_ascii_lowercase()),
                "duplicate channel {}",
                channel.name
            );
            ensure!(
                channel.messages >= 0,
                "channel {} has a negative message count",
                channel.name
            );
            let mut members = BTreeSet::new();
            for member in &channel.users {
                ensure!(
                    members.insert(member.who),
                    "channel {} has duplicate member #{}",
                    channel.name,
                    member.who.0
                );
                ensure!(
                    self.objects
                        .get(&member.who)
                        .is_some_and(|object| object.kind != Kind::Garbage),
                    "channel {} has invalid member #{}",
                    channel.name,
                    member.who.0
                );
            }
            if let Some(id) = channel.object {
                ensure!(
                    self.objects
                        .get(&id)
                        .is_some_and(|object| object.kind != Kind::Garbage),
                    "channel {} references invalid #{}",
                    channel.name,
                    id.0
                );
            }
        }
        for (who, aliases) in &self.channel_aliases {
            ensure!(
                self.objects
                    .get(who)
                    .is_some_and(|object| object.kind != Kind::Garbage),
                "channel aliases have invalid owner #{}",
                who.0
            );
            let mut names = BTreeSet::new();
            for alias in aliases {
                ensure!(
                    names.insert(alias.alias.to_ascii_lowercase()),
                    "duplicate channel alias {} for #{}",
                    alias.alias,
                    who.0
                );
            }
        }
        for (who, recipients) in &self.last_pages {
            ensure!(
                self.accounts.contains_key(who),
                "last-page owner #{} is not a player",
                who.0
            );
            for recipient in recipients {
                ensure!(
                    self.accounts.contains_key(recipient),
                    "last-page recipient #{} is not a player",
                    recipient.0
                );
            }
        }
        Ok(())
    }
}
