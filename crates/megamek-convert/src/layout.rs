//! Turns one location's resolved MegaMek equipment into template slots, linking each Artemis
//! fire-control system to the missile launcher it guides.
use crate::draft::Slot;
use crate::equipment::Critical;
use anyhow::{Result, bail};

/// How many slots each weapon instance spans in this kind of unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSpan {
    /// Mech weapons occupy their catalogue critical slot count.
    Catalogue,
    /// Vehicle weapons occupy one slot each.
    Single,
}

/// Build a section's slots from its criticals, which must already be in slot order.
pub fn slots(criticals: &[Critical], span: WeaponSpan) -> Result<[Option<Slot>; 12]> {
    if criticals.len() > 12 {
        bail!("{} items do not fit in 12 slots", criticals.len());
    }
    let mut launchers = launcher_starts(criticals, span);
    let mut slots: [Option<Slot>; 12] = Default::default();
    // The launcher the first slot of a two-slot Mech Artemis V installation claimed.
    let mut artemis_v: Option<usize> = None;
    for (index, critical) in criticals.iter().enumerate() {
        let Some(item) = critical.item() else {
            artemis_v = None;
            continue;
        };
        let link = match critical {
            Critical::Artemis => Some(claim_launcher(&mut launchers, index)? + 1),
            Critical::ArtemisV => {
                if let Some(launcher) = artemis_v.take() {
                    Some(launcher + 1)
                } else {
                    let launcher = claim_launcher(&mut launchers, index)?;
                    if span == WeaponSpan::Catalogue {
                        artemis_v = Some(launcher);
                    }
                    Some(launcher + 1)
                }
            }
            _ => None,
        };
        if !matches!(critical, Critical::ArtemisV) {
            artemis_v = None;
        }
        slots[index] = Some(Slot {
            item,
            rounds: match critical {
                Critical::Ammo { rounds, .. } => Some(*rounds),
                _ => None,
            },
            link: link.map(|slot| slot as u8),
            modes: critical.modes(),
        });
    }
    Ok(slots)
}

/// Zero-based first slots of every missile launcher instance in a section.
fn launcher_starts(criticals: &[Critical], span: WeaponSpan) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut index = 0;
    while index < criticals.len() {
        let Critical::Weapon { weapon, .. } = &criticals[index] else {
            index += 1;
            continue;
        };
        let width = match span {
            WeaponSpan::Catalogue => usize::from(weapon.profile().critical_slots.max(1)),
            WeaponSpan::Single => 1,
        };
        let mut run = 1;
        while run < width && criticals.get(index + run) == Some(&criticals[index]) {
            run += 1;
        }
        if weapon.profile().missiles > 0 {
            starts.push(index);
        }
        index += run;
    }
    starts
}

/// Take the nearest launcher before an Artemis slot, as MegaMek lists Artemis after its
/// launcher, falling back to the nearest one after it.
fn claim_launcher(launchers: &mut Vec<usize>, artemis: usize) -> Result<usize> {
    let position = launchers
        .iter()
        .rposition(|&start| start < artemis)
        .or_else(|| launchers.iter().position(|&start| start > artemis));
    let Some(position) = position else {
        bail!(
            "Artemis in slot {} has no missile launcher to guide",
            artemis + 1
        );
    };
    Ok(launchers.remove(position))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::parse;

    #[test]
    fn artemis_links_to_the_launcher_it_follows() {
        let criticals: Vec<_> = [
            "LRM 10",
            "LRM 10",
            "ISArtemisIV",
            "SRM 6",
            "SRM 6",
            "ISArtemisIV",
        ]
        .into_iter()
        .map(|name| parse(name).unwrap())
        .collect();
        let slots = slots(&criticals, WeaponSpan::Catalogue).unwrap();
        assert_eq!(slots[2].as_ref().unwrap().link, Some(1));
        assert_eq!(slots[5].as_ref().unwrap().link, Some(4));
        let lonely = [parse("ISArtemisIV").unwrap()];
        assert!(super::slots(&lonely, WeaponSpan::Single).is_err());
    }

    /// A Mech's two Artemis V slots guide one launcher; each vehicle entry guides its own.
    #[test]
    fn artemis_v_pairs_guide_one_launcher() {
        let criticals: Vec<_> = [
            "CLLRM10",
            "CLArtemisV",
            "CLArtemisV",
            "CLSRM6",
            "CLArtemisV",
            "CLArtemisV",
        ]
        .into_iter()
        .map(|name| parse(name).unwrap())
        .collect();
        let slots = slots(&criticals, WeaponSpan::Catalogue).unwrap();
        let links: Vec<_> = [1, 2, 4, 5]
            .into_iter()
            .map(|index| slots[index].as_ref().unwrap().link)
            .collect();
        assert_eq!(links, [Some(1), Some(1), Some(4), Some(4)]);
        let vehicle: Vec<_> = ["CLLRM10", "CLSRM6", "CLArtemisV", "CLArtemisV"]
            .into_iter()
            .map(|name| parse(name).unwrap())
            .collect();
        let slots = super::slots(&vehicle, WeaponSpan::Single).unwrap();
        assert_eq!(slots[2].as_ref().unwrap().link, Some(2));
        assert_eq!(slots[3].as_ref().unwrap().link, Some(1));
    }
}
