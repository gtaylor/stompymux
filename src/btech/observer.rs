//! Trusted observer mode and radio identification, using saved battlefield membership identities.
use super::{Mech, RadioChannel};
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};

impl Mech {
    /// Administrator-controlled observer mode; cockpit pilots cannot enable it through radio settings.
    pub fn is_observer(&self) -> bool {
        self.observer
    }

    /// Scenario identity or a stable base-36 label derived from the saved membership slot.
    pub fn battlefield_id(&self) -> Option<String> {
        self.map_slot().map(|slot| {
            self.battlefield_label
                .clone()
                .unwrap_or_else(|| battlefield_label(slot))
        })
    }
}

/// Trusted scenario edit; the caller owns administrative authority and the commit boundary.
pub fn set_observer(world: &mut World, id: ObjectId, enabled: bool) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|o| o.kind == Kind::Thing && !o.flags.contains(Flag::Going)),
        "Unit must be a live thing"
    );
    super::with_unit_mut!(
        world
            .btech
            .unit_mut(id)
            .context("Unit is not constructed")?,
        |unit| {
            unit.observer = enabled;
            Ok(())
        }
    )
}

/// Observer radio text identifies affiliation and sender, preserving the clear original payload.
pub(super) fn radio_text(
    world: &World,
    sender: ObjectId,
    channel: usize,
    bearing: u16,
    selected: &RadioChannel,
    message: &str,
) -> Result<String> {
    let source = super::radio::unit(world, sender)?;
    let identity = source
        .battlefield_id()
        .context("Sender has no battlefield identity")?;
    let affiliation = world.objects[&sender]
        .affiliation
        .and_then(|id| world.objects.get(&id))
        .filter(|o| !o.flags.contains(Flag::Going))
        .map_or("", |o| o.name.as_str());
    let (open, close) = if selected.mode.digital {
        ('[', ']')
    } else {
        ('(', ')')
    };
    Ok(format!(
        "{}{open}{}:{bearing}{close} <{affiliation}:{identity}:{}> <{}> {message}[reset]",
        team_color(source.signature().team),
        char::from(b'A' + channel as u8),
        selected.frequency,
        selected.title
    ))
}

/// Observer colors follow the sender's team rather than the selected receiving-channel color.
fn team_color(team: i32) -> &'static str {
    match if team > 15 { team % 15 } else { team } {
        1 => "[fg=white]",
        2 => "[fg=cyan]",
        3 => "[fg=magenta]",
        4 => "[fg=blue]",
        5 => "[fg=yellow]",
        6 => "[fg=green]",
        7 => "[fg=red]",
        8 => "[fg=black bold]",
        9 => "[fg=white bold]",
        10 => "[fg=cyan bold]",
        11 => "[fg=magenta bold]",
        12 => "[fg=blue bold]",
        13 => "[fg=yellow bold]",
        14 => "[fg=green bold]",
        15 => "[fg=red bold]",
        _ => "",
    }
}

impl super::Vehicle {
    /// Administrator-controlled observer role shared with the scanner and radio projections.
    pub fn is_observer(&self) -> bool {
        self.observer
    }

    /// Scenario identity or the same membership-derived label used by Mechs.
    pub fn battlefield_id(&self) -> Option<String> {
        self.map_slot().map(|slot| {
            self.battlefield_label
                .clone()
                .unwrap_or_else(|| battlefield_label(slot))
        })
    }
}

/// Base-36 battlefield labels share the same alphabet and minimum width for all units.
pub(super) fn battlefield_label(mut slot: u32) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut letters = Vec::new();
    loop {
        letters.push(char::from(alphabet[(slot % 36) as usize]));
        slot /= 36;
        if slot == 0 {
            break;
        }
    }
    while letters.len() < 2 {
        letters.push('A');
    }
    letters.into_iter().rev().collect()
}

/// Inspect the common scenario role without depending on unit anatomy.
pub fn unit_observer(world: &World, id: ObjectId) -> Result<bool> {
    Ok(super::scanner::scanner_unit(world, id)
        .context("Unit is not constructed")?
        .observer)
}
