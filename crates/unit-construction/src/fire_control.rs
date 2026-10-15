//! Fire-control links: each Artemis IV, Artemis V or MRM "Apollo" fire-control system names
//! the launcher it guides by the one-based slot of that launcher's first critical in the
//! controller's own location. Construction refuses a controller without a link, one linked to
//! anything but an applicable launcher there, two controllers sharing a launcher, and a unit
//! that guides some applicable launchers but not all.
use super::{CriticalLocation, ResolvedLoadout, SectionDefinition, System, Weapon};
use anyhow::{Result, bail, ensure};
use std::collections::BTreeMap;

/// Which fire-control systems a check covers and the launchers they guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireControl {
    /// Artemis IV and V guide standard LRM, SRM and MML launchers (TechManual p. 206).
    Artemis,
    /// The MRM "Apollo" system guides MRM launchers (Tactical Operations: Advanced Units &
    /// Equipment p. 142).
    Apollo,
}

impl FireControl {
    /// Both fire-control families.
    pub const ALL: [Self; 2] = [Self::Artemis, Self::Apollo];

    /// Whether `system` is one of these controllers.
    pub fn controls(self, system: System) -> bool {
        match self {
            Self::Artemis => matches!(system, System::ArtemisIv | System::ArtemisV),
            Self::Apollo => system == System::ApolloFcs,
        }
    }

    /// Whether `weapon` is a launcher these controllers guide.
    pub fn serves(self, weapon: Weapon) -> bool {
        match self {
            Self::Artemis => weapon.is_artemis_launcher(),
            Self::Apollo => weapon.is_mrm(),
        }
    }

    /// The launchers these controllers guide, for messages.
    fn launchers(self) -> &'static str {
        match self {
            Self::Artemis => "LRM, SRM or MML launcher",
            Self::Apollo => "MRM launcher",
        }
    }

    /// The rule these controllers follow, for messages.
    fn citation(self) -> &'static str {
        match self {
            Self::Artemis => "TechManual p. 206",
            Self::Apollo => "Tactical Operations: Advanced Units & Equipment p. 142",
        }
    }
}

/// Whether `system` is any fire-control system that links a launcher.
pub fn is_fire_control(system: System) -> bool {
    FireControl::ALL.iter().any(|kind| kind.controls(system))
}

/// The one-based launcher slot a fire-control critical's stored data names. A controller
/// without a link, or with link zero, guides nothing and is refused.
pub fn fire_control_link(data: &str) -> Result<u8> {
    ensure!(data != "-", "has no launcher link");
    let link: u8 = data
        .parse()
        .map_err(|_| anyhow::anyhow!("has an invalid launcher link {data:?}"))?;
    ensure!(link > 0, "has no launcher link");
    Ok(link)
}

/// A controller's display name.
fn controller_name(system: System) -> &'static str {
    match system {
        System::ArtemisIv => "Artemis IV",
        System::ArtemisV => "Artemis V",
        _ => "Apollo fire control system",
    }
}

/// Check every fire-control link of a resolved loadout against its template sections.
/// `artemis_v_slots` is how many critical entries one Artemis V installation fills: two on a
/// Mech (Tactical Operations: Advanced Units & Equipment p. 217), one on a vehicle, where
/// each entry links its launcher.
pub(crate) fn check_fire_control_links<S: Copy + Ord>(
    loadout: &ResolvedLoadout<CriticalLocation<S>>,
    sections: &BTreeMap<S, SectionDefinition>,
    section_name: impl Fn(S) -> &'static str,
    artemis_v_slots: usize,
) -> Result<()> {
    for kind in FireControl::ALL {
        check_kind(kind, loadout, sections, &section_name, artemis_v_slots)?;
    }
    Ok(())
}

/// Check the links of one fire-control family.
fn check_kind<S: Copy + Ord>(
    kind: FireControl,
    loadout: &ResolvedLoadout<CriticalLocation<S>>,
    sections: &BTreeMap<S, SectionDefinition>,
    section_name: &impl Fn(S) -> &'static str,
    artemis_v_slots: usize,
) -> Result<()> {
    // Controller entries grouped by the index of the launcher they link.
    let mut guided: BTreeMap<usize, Vec<&super::SystemCritical<CriticalLocation<S>>>> =
        BTreeMap::new();
    for part in loadout
        .systems
        .iter()
        .filter(|part| kind.controls(part.system))
    {
        let location = part.location;
        let name = section_name(location.section);
        let here = format!(
            "The {} in {name} slot {}",
            controller_name(part.system),
            location.slot + 1
        );
        let data = sections
            .get(&location.section)
            .and_then(|section| section.criticals.get(&location.slot))
            .map_or("-", |critical| critical.data.as_str());
        let link = match fire_control_link(data) {
            Ok(link) => link,
            Err(error) => bail!(
                "{here} {error}; it must link the first slot of the {} it guides in {name} ({})",
                kind.launchers(),
                kind.citation()
            ),
        };
        let target = CriticalLocation {
            section: location.section,
            slot: link - 1,
        };
        let Some(index) = loadout
            .weapons
            .iter()
            .position(|mount| mount.criticals[0] == target && kind.serves(mount.weapon))
        else {
            bail!(
                "{here} links slot {link}, which is not the first slot of an {} in {name} ({})",
                kind.launchers(),
                kind.citation()
            );
        };
        guided.entry(index).or_default().push(part);
    }
    if guided.is_empty() {
        return Ok(());
    }
    for (index, mount) in loadout.weapons.iter().enumerate() {
        if !kind.serves(mount.weapon) {
            continue;
        }
        let location = mount.criticals[0];
        let name = section_name(location.section);
        let launcher = format!(
            "the {} in {name} slot {}",
            mount.weapon.name(),
            location.slot + 1
        );
        let Some(parts) = guided.get(&index) else {
            bail!(
                "Fire control must guide every {} or none: no controller links {launcher} ({})",
                kind.launchers(),
                kind.citation()
            );
        };
        let system = parts[0].system;
        let expected = if system == System::ArtemisV {
            artemis_v_slots
        } else {
            1
        };
        ensure!(
            parts.iter().all(|part| part.system == system) && parts.len() == expected,
            "{} fire-control entries link {launcher}; one {} guides each launcher{} ({})",
            parts.len(),
            controller_name(system),
            if expected > 1 {
                format!(" and fills {expected} slots")
            } else {
                String::new()
            },
            kind.citation()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stored link data parses to a positive slot or is refused.
    #[test]
    fn links_must_name_a_slot() {
        assert_eq!(fire_control_link("3").unwrap(), 3);
        for data in ["-", "0", "bad", "-1", "256"] {
            assert!(fire_control_link(data).is_err(), "{data}");
        }
    }

    /// Each family recognizes only its own controllers and launchers.
    #[test]
    fn families_match_their_launchers() {
        assert!(FireControl::Artemis.controls(System::ArtemisV));
        assert!(!FireControl::Artemis.controls(System::ApolloFcs));
        assert!(FireControl::Artemis.serves(Weapon::Lrm10));
        assert!(!FireControl::Artemis.serves(Weapon::Mrm10));
        assert!(FireControl::Apollo.serves(Weapon::Mrm10));
        assert!(is_fire_control(System::ApolloFcs));
        assert!(!is_fire_control(System::HeatSink));
    }
}
