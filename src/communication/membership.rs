//! Persistent channel aliases, listening preferences and visibility-filtered member queries.
use super::*;

impl Service<'_> {
    /// Create an alias and join atomically under the caller transaction.
    pub fn add(
        &self,
        who: ObjectId,
        channel: &str,
        alias: &str,
        quiet: bool,
        trusted: bool,
    ) -> Result<()> {
        ensure!(!alias.is_empty(), "You need to specify an alias.");
        ensure!(
            alias.len() <= ALIAS_LIMIT && alias.bytes().all(|b| (33..=126).contains(&b)),
            "Channel aliases must be 1-5 printable ASCII characters without spaces."
        );
        let name = self.name(channel)?;
        let identity = self.world.borrow().channels[&name].id;
        ensure!(
            !self
                .world
                .borrow()
                .channel_aliases
                .get(&who)
                .is_some_and(|a| a.iter().any(|a| a.alias.eq_ignore_ascii_case(alias))),
            "That alias is already in use."
        );
        ensure!(
            trusted || self.allowed(who, &name, Access::Join)?,
            "You are not allowed to join that channel."
        );
        ensure!(
            self.by_id(identity)? == name,
            "channel replaced by callback"
        );
        {
            let mut w = self.world.borrow_mut();
            let aliases = w.channel_aliases.entry(who).or_default();
            aliases.push(ChannelAlias {
                alias: alias.into(),
                channel: name.clone(),
            });
            aliases.sort_by_key(|a| a.alias.to_ascii_lowercase());
        }
        self.join(who, &name, quiet)
    }

    /// Enable membership, growing capacity in the C ten-entry increments.
    pub fn join(&self, who: ObjectId, channel: &str, quiet: bool) -> Result<()> {
        let name = self.name(channel)?;
        let already = self.world.borrow().channels[&name]
            .users
            .iter()
            .any(|u| u.who == who && u.listening);
        if already {
            return self.notify(who, format!("You are already on channel {name}."));
        }
        let (player, dark) = {
            let mut w = self.world.borrow_mut();
            let o = w
                .objects
                .get(&who)
                .ok_or_else(|| anyhow::anyhow!("object does not exist"))?;
            let details = (o.name.clone(), o.flags.contains(Flag::Dark));
            let c = w.channels.get_mut(&name).unwrap();
            if let Some(u) = c.users.iter_mut().find(|u| u.who == who) {
                u.listening = true;
            } else {
                c.users.push(Membership {
                    who,
                    listening: true,
                });
                c.users.sort_by_key(|u| u.who);
                if c.users.len() >= c.max_users {
                    c.max_users += 10;
                }
            }
            details
        };
        self.notify(who, format!("You have joined channel {name}."))?;
        if !quiet && !dark {
            self.emit(&name, &format!("{player} has joined this channel."), false)?;
        }
        Ok(())
    }

    /// Announce departure and run channel-object callbacks before turning off.
    pub fn leave(&self, who: ObjectId, channel: &str) -> Result<()> {
        let name = self.name(channel)?;
        let (player, dark, on) = {
            let w = self.world.borrow();
            let o = &w.objects[&who];
            (
                o.name.clone(),
                o.flags.contains(Flag::Dark),
                w.channels[&name]
                    .users
                    .iter()
                    .any(|u| u.who == who && u.listening),
            )
        };
        let identity = self.world.borrow().channels[&name].id;
        self.leave_callbacks(who, &name)?;
        ensure!(
            self.by_id(identity)? == name,
            "channel replaced by callback"
        );
        self.notify(who, format!("You have left channel {name}."))?;
        if on && !dark {
            self.emit(&name, &format!("{player} has left this channel."), false)?;
        }
        self.world
            .borrow_mut()
            .channels
            .get_mut(&name)
            .ok_or_else(|| anyhow::anyhow!("channel removed by callback"))?
            .users
            .iter_mut()
            .find(|u| u.who == who)
            .ok_or_else(|| anyhow::anyhow!("membership removed by callback"))?
            .listening = false;
        Ok(())
    }

    /// Remove membership only when the last alias to that channel disappears.
    pub fn remove_alias(&self, who: ObjectId, alias: &str) -> Result<()> {
        let name = {
            let mut w = self.world.borrow_mut();
            let a = w
                .channel_aliases
                .get_mut(&who)
                .ok_or_else(|| anyhow::anyhow!("Unknown channel alias."))?;
            let i = a
                .iter()
                .position(|a| a.alias.eq_ignore_ascii_case(alias))
                .ok_or_else(|| anyhow::anyhow!("Unknown channel alias."))?;
            a.remove(i).channel
        };
        if !self.world.borrow().channel_aliases[&who]
            .iter()
            .any(|a| a.channel.eq_ignore_ascii_case(&name))
            && let Ok(name) = self.name(&name)
        {
            if self.world.borrow().channels[&name]
                .users
                .iter()
                .any(|u| u.who == who && u.listening)
            {
                self.leave(who, &name)?;
            }
            self.world
                .borrow_mut()
                .channels
                .get_mut(&name)
                .ok_or_else(|| anyhow::anyhow!("channel removed by callback"))?
                .users
                .retain(|u| u.who != who);
        }
        self.notify(who, format!("Alias {alias} deleted."))
    }

    /// Use the shared leave/alias cleanup path for administrative removal.
    pub fn boot(&self, actor: ObjectId, who: ObjectId, channel: &str) -> Result<()> {
        let name = self.name(channel)?;
        ensure!(
            self.world.borrow().channels[&name]
                .users
                .iter()
                .any(|u| u.who == who),
            "object is not a member of this channel"
        );
        let (a, b) = {
            let w = self.world.borrow();
            (
                format!("{}(#{})", w.objects[&actor].name, actor.0),
                format!("{}(#{})", w.objects[&who].name, who.0),
            )
        };
        self.emit(&name, &format!("{a} boots {b} off the channel."), false)?;
        let aliases = self
            .world
            .borrow()
            .channel_aliases
            .get(&who)
            .cloned()
            .unwrap_or_default();
        for alias in aliases
            .into_iter()
            .filter(|a| a.channel.eq_ignore_ascii_case(&name))
        {
            self.remove_alias(who, &alias.alias)?;
        }
        self.world
            .borrow_mut()
            .channels
            .get_mut(&name)
            .ok_or_else(|| anyhow::anyhow!("channel removed by callback"))?
            .users
            .retain(|u| u.who != who);
        Ok(())
    }

    /// Return valid active or all memberships for native and Lua queries.
    pub fn members(&self, channel: &str, all: bool) -> Result<Vec<Membership>> {
        let name = self.name(channel)?;
        let w = self.world.borrow();
        Ok(w.channels[&name]
            .users
            .iter()
            .filter(|u| {
                w.objects.get(&u.who).is_some_and(|o| {
                    o.kind != Kind::Garbage
                        && (all || o.kind != Kind::Player || o.flags.contains(Flag::Connected))
                })
            })
            .cloned()
            .collect())
    }

    /// Render C player/object groups and Wizard-level identity details.
    pub fn who(&self, viewer: ObjectId, channel: &str, all: bool, admin: bool) -> Result<()> {
        let name = self.name(channel)?;
        let members = self.members(&name, all)?;
        if admin {
            self.notify(viewer, format!("-- {name} --"))?;
        }
        if admin {
            self.notify(viewer, "Name                          Status Player")?;
        } else {
            self.notify(viewer, "-- Players --")?;
        }
        for kind in [true, false] {
            if !kind && !admin {
                self.notify(viewer, "-- Objects --")?;
            }
            for u in &members {
                let output = {
                    let w = self.world.borrow();
                    let o = &w.objects[&u.who];
                    if (o.kind == Kind::Player) != kind || (!admin && !u.listening) {
                        continue;
                    }
                    if (kind || admin) && o.flags.contains(Flag::Dark) && !wizard(&w, viewer) {
                        continue;
                    }
                    if !admin && kind && in_character(&w, self.config, u.who) && !wizard(&w, u.who)
                    {
                        continue;
                    }
                    if !admin && !kind && o.flags.contains(Flag::Going) {
                        continue;
                    }
                    let display = if wizard(&w, viewer) {
                        format!("{}{}", o.name, crate::find::suffix(o))
                    } else {
                        o.name.clone()
                    };
                    if admin {
                        format!(
                            "{} {:6} {}",
                            column(&crate::text::plain_with(&w.palette, &display), 29),
                            if u.listening { "on" } else { "off" },
                            if kind { "yes" } else { "no" }
                        )
                    } else {
                        let idle = if kind { self.idle(u.who) } else { 0 };
                        if idle > 30 {
                            format!("{display} [[idle {}]", duration(idle))
                        } else {
                            display
                        }
                    }
                };
                self.notify(viewer, output)?;
            }
        }
        self.notify(viewer, format!("-- {name} --"))?;
        Ok(())
    }
}

impl Service<'_> {
    /// Channel things receive C's leave event, with the departing member as cause.
    fn leave_callbacks(&self, who: ObjectId, channel: &str) -> Result<()> {
        let objects = {
            let w = self.world.borrow();
            w.channels[channel]
                .users
                .iter()
                .rev()
                .filter_map(|u| w.objects.get(&u.who))
                .filter(|o| o.kind == Kind::Thing && !o.flags.contains(Flag::Halted))
                .map(|o| (o.id, o.lua_parent.clone()))
                .collect::<Vec<_>>()
        };
        let invoke: mlua::Function = self.lua.load(r#"return function(parent, object, who)
            local module = _parents[parent]
            local callback = module and module.events and module.events.on_leave
            if callback then callback({object=object, subject=who, enactor=who, cause=who, scope='object'}) end
        end"#).eval().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for (object, parent) in objects {
            invoke
                .call::<()>((parent, object.0, who.0))
                .map_err(|e| anyhow::anyhow!("channel {channel} leave callback: {e}"))?;
        }
        Ok(())
    }
}

/// Use live session snapshots; multiple sessions use the shortest idle interval.
impl Service<'_> {
    fn idle(&self, who: ObjectId) -> u64 {
        self.lua
            .globals()
            .get::<Option<mlua::Table>>("_connected_players")
            .ok()
            .flatten()
            .map(|list| {
                list.sequence_values::<mlua::Table>()
                    .filter_map(|row| row.ok())
                    .filter(|row| row.get::<i64>("dbref").ok() == Some(who.0))
                    .filter_map(|row| row.get::<u64>("idle_for").ok())
                    .min()
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }
}

/// Display-column clipping never splits a Unicode grapheme.
pub(super) fn column(s: &str, width: usize) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    let mut result = String::new();
    let mut used = 0;
    for g in s.graphemes(true) {
        let columns = unicode_width::UnicodeWidthStr::width(g);
        if used + columns > width {
            break;
        }
        result.push_str(g);
        used += columns;
    }
    result.push_str(&" ".repeat(width - used));
    result
}

/// C's verbose idle duration uses thirty-day months and twelve-month years.
fn duration(mut seconds: u64) -> String {
    let mut parts = Vec::new();
    for (size, name) in [
        (31_104_000, "year"),
        (2_592_000, "month"),
        (86_400, "day"),
        (3_600, "hour"),
        (60, "minute"),
        (1, "second"),
    ] {
        let count = seconds / size;
        seconds %= size;
        if count > 0 {
            parts.push(format!(
                "{count} {name}{}",
                if count == 1 { "" } else { "s" }
            ));
        }
    }
    if parts.len() > 1 {
        let last = parts.pop().unwrap();
        format!("{} and {last}", parts.join(", "))
    } else {
        parts.join("")
    }
}
