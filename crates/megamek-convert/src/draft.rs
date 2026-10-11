//! A converted unit before stompymux validation: the facts read from a MegaMek file, written
//! out in the template document syntax with every slot spelled out. The game's own template
//! parser then checks the draft and renders it in canonical, compressed form.
use std::fmt::Write as _;

/// One item in one slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub item: String,
    pub rounds: Option<u16>,
    /// One-based slot of the launcher an Artemis system guides.
    pub link: Option<u8>,
    pub modes: Vec<&'static str>,
}

/// One armored location and its twelve slots.
#[derive(Debug, Clone)]
pub struct Section {
    /// Template heading such as `left_arm` or `front_side`.
    pub heading: &'static str,
    pub armor: u16,
    pub rear: u16,
    /// Mech sections list their actuators, engine, gyro and cockpit themselves.
    pub explicit: bool,
    pub slots: [Option<Slot>; 12],
}

impl Section {
    /// An empty section.
    pub fn new(heading: &'static str, armor: u16, rear: u16, explicit: bool) -> Self {
        Self {
            heading,
            armor,
            rear,
            explicit,
            slots: Default::default(),
        }
    }
}

/// A weapon whose slots continue from its primary section into an adjacent one.
#[derive(Debug, Clone)]
pub struct SplitMount {
    pub item: String,
    pub modes: Vec<&'static str>,
    /// Primary then extension placement: heading and inclusive zero-based slots.
    pub placements: [(&'static str, usize, usize); 2],
}

/// A whole converted unit.
#[derive(Debug, Clone)]
pub struct Draft {
    pub name: String,
    /// Template class: `mech`, `vehicle` or `vtol`.
    pub class: &'static str,
    /// Template movement: `biped`, `quad`, `track`, `wheel`, `hover` or `vtol`.
    pub movement: &'static str,
    pub tons: u16,
    /// Maximum (running or flank) movement points.
    pub max_mp: u32,
    pub jump_mp: u32,
    /// Total heat dissipation, when the unit states one.
    pub heat_sinks: Option<u32>,
    /// `[construction]` keys and document spellings for every non-default choice.
    pub construction: Vec<(&'static str, &'static str)>,
    pub specials: Vec<&'static str>,
    pub sections: Vec<Section>,
    pub split_mounts: Vec<SplitMount>,
}

impl Draft {
    /// Write the draft as a template document.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "name = {}", quote(&self.name));
        let _ = writeln!(out, "class = {}", quote(self.class));
        let _ = writeln!(out, "movement = {}", quote(self.movement));
        let _ = writeln!(out, "tons = {}", self.tons);
        let _ = writeln!(out, "walk_mp = {}", self.max_mp);
        if self.jump_mp > 0 {
            let _ = writeln!(out, "jump_mp = {}", self.jump_mp);
        }
        if let Some(heat_sinks) = self.heat_sinks {
            let _ = writeln!(out, "heat_sinks = {heat_sinks}");
        }
        if !self.specials.is_empty() {
            let _ = writeln!(out, "specials = {}", list(&self.specials));
        }
        if !self.construction.is_empty() {
            out.push_str("\n[construction]\n");
            for (key, value) in &self.construction {
                let _ = writeln!(out, "{key} = {}", quote(value));
            }
        }
        for section in &self.sections {
            let _ = writeln!(out, "\n[sections.{}]", section.heading);
            let _ = writeln!(out, "armor = {}", section.armor);
            if section.rear > 0 {
                let _ = writeln!(out, "rear = {}", section.rear);
            }
            if section.explicit {
                out.push_str("explicit = true\n");
            }
            out.push_str("slots = [\n");
            for (index, slot) in section.slots.iter().enumerate() {
                let Some(slot) = slot else {
                    continue;
                };
                let _ = write!(
                    out,
                    "    {{ at = {}, item = {}",
                    index + 1,
                    quote(&slot.item)
                );
                if let Some(rounds) = slot.rounds {
                    let _ = write!(out, ", rounds = {rounds}");
                }
                if let Some(link) = slot.link {
                    let _ = write!(out, ", link = {link}");
                }
                if !slot.modes.is_empty() {
                    let _ = write!(out, ", modes = {}", list(&slot.modes));
                }
                out.push_str(" },\n");
            }
            out.push_str("]\n");
        }
        for mount in &self.split_mounts {
            out.push_str("\n[[split_mounts]]\n");
            let _ = writeln!(out, "item = {}", quote(&mount.item));
            if !mount.modes.is_empty() {
                let _ = writeln!(out, "modes = {}", list(&mount.modes));
            }
            out.push_str("placements = [\n");
            for (heading, first, last) in mount.placements {
                let _ = writeln!(
                    out,
                    "    {{ section = {}, at = \"{}-{}\" }},",
                    quote(heading),
                    first + 1,
                    last + 1
                );
            }
            out.push_str("]\n");
        }
        out
    }
}

/// Quote a TOML basic string.
fn quote(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

/// Render strings as a one-line TOML array.
fn list(values: &[&str]) -> String {
    let values: Vec<_> = values.iter().map(|value| quote(value)).collect();
    format!("[{}]", values.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_spell_out_every_slot() {
        let mut section = Section::new("front_side", 10, 0, false);
        section.slots[0] = Some(Slot {
            item: "IS.LRM-5".into(),
            rounds: None,
            link: None,
            modes: vec![],
        });
        section.slots[1] = Some(Slot {
            item: "ArtemisIV".into(),
            rounds: None,
            link: Some(1),
            modes: vec![],
        });
        section.slots[2] = Some(Slot {
            item: "Ammo_IS.LRM-5".into(),
            rounds: Some(24),
            link: None,
            modes: vec!["Artemis/Mine"],
        });
        let draft = Draft {
            name: "Truck".into(),
            class: "vehicle",
            movement: "wheel",
            tons: 20,
            max_mp: 6,
            jump_mp: 0,
            heat_sinks: None,
            construction: vec![("engine", "ice")],
            specials: vec![],
            sections: vec![section],
            split_mounts: vec![],
        };
        assert_eq!(
            draft.render(),
            "name = \"Truck\"\nclass = \"vehicle\"\nmovement = \"wheel\"\ntons = 20\nwalk_mp = 6\n\n\
             [construction]\nengine = \"ice\"\n\n[sections.front_side]\narmor = 10\nslots = [\n    \
             { at = 1, item = \"IS.LRM-5\" },\n    { at = 2, item = \"ArtemisIV\", link = 1 },\n    \
             { at = 3, item = \"Ammo_IS.LRM-5\", rounds = 24, modes = [\"Artemis/Mine\"] },\n]\n"
        );
    }
}
