//! A converted unit before stompymux validation: the facts read from a MegaMek file, written
//! out in the template document syntax with every slot spelled out. The game's own template
//! parser then checks the draft and renders it in canonical, compressed form.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// One item in one slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub item: String,
    pub rounds: Option<u16>,
    /// One-based slot of the launcher an Artemis system guides.
    pub link: Option<u8>,
    /// One-based first slots of the machine guns a machine gun array links.
    pub links: Vec<u8>,
    pub modes: Vec<&'static str>,
}

/// One armored location and its slots: twelve for a Mech, as many as a vehicle needs.
#[derive(Debug, Clone)]
pub struct Section {
    /// Template heading such as `left_arm` or `front_side`.
    pub heading: &'static str,
    pub armor: u16,
    pub rear: u16,
    /// Mech sections list their actuators, engine, gyro and cockpit themselves.
    pub explicit: bool,
    pub slots: Vec<Option<Slot>>,
}

impl Section {
    /// An empty section of twelve slots.
    pub fn new(heading: &'static str, armor: u16, rear: u16, explicit: bool) -> Self {
        Self {
            heading,
            armor,
            rear,
            explicit,
            slots: vec![None; 12],
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
    /// Walking (Mech) or cruising (vehicle) movement points, as the source states them.
    pub walk_mp: u32,
    pub jump_mp: u32,
    /// Installed heat sinks, as the source counts them, when they differ from the default.
    pub heat_sinks: Option<u32>,
    /// The engine rating the source declares: the `.mtf` engine line's rating, or a vehicle's
    /// Final Engine Rating from its tonnage, cruising MP and motive type.
    pub engine_rating: u32,
    /// `[construction]` keys and document spellings for every non-default choice.
    pub construction: Vec<(&'static str, &'static str)>,
    pub specials: Vec<&'static str>,
    /// Standard cargo bay capacity in tons, written as `cargo_tons` (TechManual p. 239).
    pub cargo_tons: Option<String>,
    /// `[transport_bays]` keys with their TOML values: tons for a variable bay, a count for a
    /// fixed one.
    pub transport_bays: Vec<(&'static str, String)>,
    pub sections: Vec<Section>,
    pub split_mounts: Vec<SplitMount>,
}

/// The ammunition slot mode marking a half-ton lot, which names no munition.
const HALF_TON: &str = "Halfton";

impl Draft {
    /// Load every launcher the way MegaMek does, from one of its own bins. A stompymux weapon
    /// fires only the bins of the munition it has selected, so a weapon none of whose bins
    /// holds standard rounds (only Artemis-capable missiles, say, or MML long-range missiles)
    /// starts with the munition of its first bin in template order instead of with none.
    pub fn load_launchers(&mut self) {
        let mut first: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
        let mut standard = BTreeSet::new();
        for slot in self
            .sections
            .iter()
            .flat_map(|section| section.slots.iter().flatten())
        {
            let Some(weapon) = slot.item.strip_prefix("Ammo_") else {
                continue;
            };
            let munition: Vec<_> = slot
                .modes
                .iter()
                .copied()
                .filter(|mode| *mode != HALF_TON)
                .collect();
            if munition.is_empty() {
                standard.insert(weapon.to_owned());
            }
            first.entry(weapon.to_owned()).or_insert(munition);
        }
        let load = |item: &str, modes: &mut Vec<&'static str>| {
            if standard.contains(item) {
                return;
            }
            for mode in first.get(item).into_iter().flatten() {
                if !modes.contains(mode) {
                    modes.push(mode);
                }
            }
        };
        for slot in self
            .sections
            .iter_mut()
            .flat_map(|section| section.slots.iter_mut().flatten())
            .filter(|slot| !slot.item.starts_with("Ammo_"))
        {
            load(&slot.item, &mut slot.modes);
        }
        for mount in &mut self.split_mounts {
            load(&mount.item, &mut mount.modes);
        }
    }

    /// Write the draft as a template document.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "name = {}", quote(&self.name));
        let _ = writeln!(out, "class = {}", quote(self.class));
        let _ = writeln!(out, "movement = {}", quote(self.movement));
        let _ = writeln!(out, "tons = {}", self.tons);
        let _ = writeln!(out, "walk_mp = {}", self.walk_mp);
        if self.jump_mp > 0 {
            let _ = writeln!(out, "jump_mp = {}", self.jump_mp);
        }
        if let Some(heat_sinks) = self.heat_sinks {
            let _ = writeln!(out, "heat_sinks = {heat_sinks}");
        }
        if let Some(tons) = &self.cargo_tons {
            let _ = writeln!(out, "cargo_tons = {tons}");
        }
        if !self.specials.is_empty() {
            let _ = writeln!(out, "specials = {}", list(&self.specials));
        }
        out.push_str("\n[construction]\n");
        // The rating follows the technology base and engine family, ahead of the other choices.
        let mut rating = Some(self.engine_rating);
        for (key, value) in &self.construction {
            if !matches!(*key, "tech_base" | "mixed_tech" | "engine")
                && let Some(rating) = rating.take()
            {
                let _ = writeln!(out, "engine_rating = {rating}");
            }
            // `mixed_tech` is the one boolean construction key.
            if *key == "mixed_tech" {
                let _ = writeln!(out, "{key} = {value}");
                continue;
            }
            let _ = writeln!(out, "{key} = {}", quote(value));
        }
        if let Some(rating) = rating {
            let _ = writeln!(out, "engine_rating = {rating}");
        }
        if !self.transport_bays.is_empty() {
            out.push_str("\n[transport_bays]\n");
            for (key, value) in &self.transport_bays {
                let _ = writeln!(out, "{key} = {value}");
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
                if !slot.links.is_empty() {
                    let links: Vec<_> = slot.links.iter().map(u8::to_string).collect();
                    let _ = write!(out, ", links = [{}]", links.join(", "));
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

    /// A slot holding `item` with `modes` and no rounds or links.
    fn slot(item: &str, modes: &[&'static str]) -> Option<Slot> {
        Some(Slot {
            item: item.into(),
            rounds: None,
            link: None,
            links: vec![],
            modes: modes.to_vec(),
        })
    }

    /// A launcher whose bins are all of one special munition starts loaded with it; one with
    /// a standard bin, or with no bins, keeps the standard selection.
    #[test]
    fn launchers_start_with_the_munition_of_their_bins() {
        let mut front = Section::new("front_side", 10, 0, false);
        front.slots[0] = slot("IS.LRM-10", &[]);
        front.slots[1] = slot("Ammo_IS.LRM-10", &["Artemis/Mine"]);
        front.slots[2] = slot("IS.SRM-4", &[]);
        front.slots[3] = slot("Ammo_IS.SRM-4", &["Artemis/Mine"]);
        front.slots[4] = slot("IS.MML-5", &[]);
        front.slots[5] = slot("Ammo_IS.MML-5", &["MML_LRM", "Halfton"]);
        let mut rear = Section::new("aft_side", 10, 0, false);
        rear.slots[0] = slot("Ammo_IS.SRM-4", &[]);
        rear.slots[1] = slot("IS.MediumLaser", &[]);
        rear.slots[2] = slot("IS.LRM-10", &["RearMount"]);
        let mut draft = Draft {
            name: "Carrier".into(),
            class: "vehicle",
            movement: "track",
            tons: 50,
            walk_mp: 3,
            jump_mp: 0,
            heat_sinks: None,
            engine_rating: 100,
            construction: vec![],
            specials: vec![],
            cargo_tons: None,
            transport_bays: vec![],
            sections: vec![front, rear],
            split_mounts: vec![SplitMount {
                item: "IS.LRM-10".into(),
                modes: vec![],
                placements: [("left_torso", 0, 1), ("left_arm", 0, 2)],
            }],
        };
        draft.load_launchers();
        let modes = |section: usize, index: usize| {
            draft.sections[section].slots[index]
                .as_ref()
                .unwrap()
                .modes
                .clone()
        };
        assert_eq!(modes(0, 0), ["Artemis/Mine"]);
        // The SRM-4 has a standard bin in the rear as well.
        assert!(modes(0, 2).is_empty());
        // The half-ton lot names no munition.
        assert_eq!(modes(0, 4), ["MML_LRM"]);
        assert!(modes(1, 1).is_empty());
        assert_eq!(modes(1, 2), ["RearMount", "Artemis/Mine"]);
        // Bins keep their own modes.
        assert_eq!(modes(0, 5), ["MML_LRM", "Halfton"]);
        assert_eq!(draft.split_mounts[0].modes, ["Artemis/Mine"]);
    }

    #[test]
    fn drafts_spell_out_every_slot() {
        let mut section = Section::new("front_side", 10, 0, false);
        section.slots[0] = Some(Slot {
            item: "IS.LRM-5".into(),
            rounds: None,
            link: None,
            links: vec![],
            modes: vec![],
        });
        section.slots[1] = Some(Slot {
            item: "IS.ArtemisIV".into(),
            rounds: None,
            link: Some(1),
            links: vec![],
            modes: vec![],
        });
        section.slots[2] = Some(Slot {
            item: "Ammo_IS.LRM-5".into(),
            rounds: Some(24),
            link: None,
            links: vec![],
            modes: vec!["Artemis/Mine"],
        });
        let draft = Draft {
            name: "Truck".into(),
            class: "vehicle",
            movement: "wheel",
            tons: 20,
            walk_mp: 4,
            jump_mp: 0,
            heat_sinks: None,
            engine_rating: 60,
            construction: vec![("engine", "ice")],
            specials: vec![],
            cargo_tons: None,
            transport_bays: vec![],
            sections: vec![section],
            split_mounts: vec![],
        };
        assert_eq!(
            draft.render(),
            "name = \"Truck\"\nclass = \"vehicle\"\nmovement = \"wheel\"\ntons = 20\nwalk_mp = 4\n\n\
             [construction]\nengine = \"ice\"\nengine_rating = 60\n\n[sections.front_side]\narmor = 10\nslots = [\n    \
             { at = 1, item = \"IS.LRM-5\" },\n    { at = 2, item = \"IS.ArtemisIV\", link = 1 },\n    \
             { at = 3, item = \"Ammo_IS.LRM-5\", rounds = 24, modes = [\"Artemis/Mine\"] },\n]\n"
        );
    }
}
