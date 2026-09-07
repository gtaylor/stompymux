//! Online paging with C recipient resolution, poses and persistent last-recipient lists.
use super::*;

impl Service<'_> {
    /// Resolve a whole player name first, preserving ordered partial deliveries.
    pub fn page(&self, sender: ObjectId, args: &str) -> Result<()> {
        let (targets, message) = args.split_once('=').unwrap_or((args, ""));
        let saved = message.is_empty();
        if !saved && targets.trim().is_empty() {
            return self.notify(sender, "I don't recognize \"\".");
        }
        let (recipients, group_format) = if saved {
            let recipients = self
                .world
                .borrow()
                .last_pages
                .get(&sender)
                .cloned()
                .unwrap_or_default();
            let group = recipients.len() > 1;
            (recipients, group)
        } else {
            let w = self.world.borrow();
            if let Some(id) = w.find_player(targets.trim()) {
                (vec![id], false)
            } else {
                let names: Vec<_> = targets.split_whitespace().collect();
                let group = names.len() > 1;
                let mut found = Vec::new();
                for name in names {
                    if let Some(id) = w.find_player(name) {
                        found.push(id);
                    } else {
                        self.notify(
                            sender,
                            format!("I don't recognize \"{}\".", crate::text::escape(name)),
                        )?;
                    }
                }
                (found, group)
            }
        };
        if saved && targets.is_empty() {
            if recipients.is_empty() {
                self.notify(sender, "You have not paged anyone.")?;
            }
            for id in recipients {
                if let Some(o) = self.world.borrow().objects.get(&id) {
                    self.notify(sender, format!("You last paged {}.", o.name))?;
                }
            }
            return Ok(());
        }
        if saved && recipients.is_empty() {
            return self.notify(sender, "You have not paged anyone.");
        }
        let message = if saved { targets } else { message };
        let (name, alias, message, names) = {
            let w = self.world.borrow();
            let alias = w
                .accounts
                .get(&sender)
                .and_then(|a| a.alias.as_ref())
                .filter(|s| !s.is_empty())
                .map(|a| format!(" ({})", crate::text::escape(a)))
                .unwrap_or_default();
            (
                w.objects[&sender].name.clone(),
                alias,
                crate::text::escape(&crate::text::plain_with(&w.palette, message)),
                recipients
                    .iter()
                    .filter_map(|id| w.objects.get(id).map(|o| o.name.clone()))
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        };
        let multiple = recipients.len() > 1;
        let pose = message
            .strip_prefix(':')
            .map(|m| (" ", m))
            .or_else(|| message.strip_prefix(';').map(|m| ("", m)));
        let mut delivered = Vec::new();
        for target in recipients {
            let error = {
                let w = self.world.borrow();
                let Some(o) = w.objects.get(&target).filter(|o| o.kind == Kind::Player) else {
                    continue;
                };
                if in_character(&w, &self.config, sender)
                    && !wizard(&w, sender)
                    && !wizard(&w, target)
                {
                    Some("Permission denied.".into())
                } else if !o.flags.contains(Flag::Connected) {
                    Some(format!("Sorry, {} is not connected.", o.name))
                } else if !wizard(&w, sender)
                    && !wizard(&w, target)
                    && in_character(&w, &self.config, target)
                {
                    Some(format!("Sorry, {} is not accepting pages.", o.name))
                } else {
                    None
                }
            };
            if let Some(error) = error {
                self.notify(sender, error)?;
                continue;
            }
            let output = if let Some((separator, body)) = pose {
                if group_format {
                    format!("From afar, to ({names}):{alias} {name}{separator}{body}")
                } else {
                    format!("From afar,{alias} {name}{separator}{body}")
                }
            } else {
                let body = message.strip_prefix('"').unwrap_or(&message);
                if group_format {
                    format!("To ({names}), {name}{alias} pages you: {body}")
                } else {
                    format!("{name}{alias} pages: {body}")
                }
            };
            self.notify(target, output.clone())?;
            let has_exit = self.world.borrow().objects.values().any(|object| {
                object.kind == Kind::Exit
                    && object.location == Some(target)
                    && object.flags.contains(Flag::Audible)
                    && object.destination.is_some_and(|to| to != target)
            });
            if has_exit {
                crate::notification::send(
                    &self.world.borrow(),
                    self.outbox,
                    &self.config,
                    crate::notification::Request {
                        target,
                        sender,
                        document: output.into(),
                        policy: crate::notification::Policy::AUDIBLE_EXITS,
                        exclusions: None,
                    },
                )?;
            }
            delivered.push(target);
        }
        if !delivered.is_empty() {
            let display = if multiple {
                format!("({names})")
            } else {
                names
            };
            let confirmation = if let Some((separator, body)) = pose {
                format!("Long distance to {display}: {name}{separator}{body}")
            } else {
                format!("You paged {display} with '{message}'.")
            };
            self.world.borrow_mut().last_pages.insert(sender, delivered);
            self.notify(sender, confirmation)?;
        }
        Ok(())
    }
}
