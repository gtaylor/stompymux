//! Turns one location's resolved MegaMek equipment into template slots, linking each Artemis
//! or MRM Apollo fire-control system to the missile launcher it guides and each machine gun
//! array to the machine guns of its size class in the location.
use crate::draft::Slot;
use crate::equipment::Critical;
use anyhow::{Result, bail};
use stompymux_unit_construction::{SECTION_SLOTS, VEHICLE_SECTION_SLOT_LIMIT};

/// How many slots each weapon instance spans in this kind of unit, and so how many slots a
/// section offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSpan {
    /// Mech weapons occupy their catalogue critical slot count.
    Catalogue,
    /// Vehicle weapons occupy one slot each.
    Single,
}

impl WeaponSpan {
    /// Slots one section holds: twelve for a Mech, and no per-location limit for a vehicle,
    /// whose item-slot budget covers the whole vehicle (TechManual p. 98).
    fn section_slots(self) -> usize {
        match self {
            Self::Catalogue => usize::from(SECTION_SLOTS),
            Self::Single => usize::from(VEHICLE_SECTION_SLOT_LIMIT),
        }
    }
}

/// Build a section's slots from its criticals, which must already be in slot order. The
/// result lists at least twelve slots, empty or not.
pub fn slots(criticals: &[Critical], span: WeaponSpan) -> Result<Vec<Option<Slot>>> {
    let limit = span.section_slots();
    if criticals.len() > limit {
        bail!("{} items do not fit in {limit} slots", criticals.len());
    }
    if span == WeaponSpan::Catalogue {
        check_communications_runs(criticals)?;
    }
    let mut launchers = launcher_starts(criticals, span, |_| true);
    let mut mrm_launchers = launcher_starts(criticals, span, |weapon| weapon.is_mrm());
    let mut guns = gun_starts(criticals);
    let mut slots = vec![None; criticals.len().max(usize::from(SECTION_SLOTS))];
    // The launcher the first slot of a two-slot Mech Artemis V installation claimed.
    let mut artemis_v: Option<usize> = None;
    for (index, critical) in criticals.iter().enumerate() {
        let Some(item) = critical.item() else {
            artemis_v = None;
            continue;
        };
        let link = match critical {
            Critical::Artemis(_) => Some(claim_launcher(&mut launchers, index)? + 1),
            Critical::Apollo => Some(claim_launcher(&mut mrm_launchers, index)? + 1),
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
        let links = match critical {
            Critical::MachineGunArray(_, class) => claim_guns(&mut guns, index, *class),
            _ => Vec::new(),
        };
        slots[index] = Some(Slot {
            item,
            rounds: match critical {
                Critical::Ammo { rounds, .. } => Some(*rounds),
                _ => None,
            },
            link: link.map(|slot| slot as u8),
            links,
            // A vehicle's communications equipment is one item stating its tons; a Mech's
            // fills a slot per ton (TechManual p. 212).
            tons: match critical {
                Critical::Communications(tons) if span == WeaponSpan::Single => Some(*tons),
                _ => None,
            },
            modes: critical.modes(),
        });
    }
    Ok(slots)
}

/// A Mech's communications equipment fills a slot per ton (TechManual p. 212), so each run
/// of it is as long as the tons its `SIZE` states.
fn check_communications_runs(criticals: &[Critical]) -> Result<()> {
    let mut index = 0;
    while index < criticals.len() {
        let Critical::Communications(tons) = criticals[index] else {
            index += 1;
            continue;
        };
        let run = criticals[index..]
            .iter()
            .take_while(|critical| **critical == Critical::Communications(tons))
            .count();
        if run != usize::from(tons) {
            bail!("{tons} tons of communications equipment fill {run} slots, not one per ton");
        }
        index += run;
    }
    Ok(())
}

/// Zero-based first slots of every missile launcher instance in a section that `accept`s.
fn launcher_starts(
    criticals: &[Critical],
    span: WeaponSpan,
    accept: impl Fn(stompymux_unit_construction::Weapon) -> bool,
) -> Vec<usize> {
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
        if weapon.profile().missiles > 0 && accept(*weapon) {
            starts.push(index);
        }
        index += run;
    }
    starts
}

/// Zero-based slots of every machine gun in a section, with its size class.
fn gun_starts(
    criticals: &[Critical],
) -> Vec<(usize, stompymux_unit_construction::MachineGunClass)> {
    criticals
        .iter()
        .enumerate()
        .filter_map(|(index, critical)| match critical {
            Critical::Weapon { weapon, .. } => Some((index, weapon.machine_gun_class()?)),
            _ => None,
        })
        .collect()
}

/// Take up to four unclaimed machine guns of an array's size class in its section, nearest
/// the array first, as the one-based slots its `links` list in slot order (TechManual
/// p. 228). Construction refuses an array left with fewer than two.
fn claim_guns(
    guns: &mut Vec<(usize, stompymux_unit_construction::MachineGunClass)>,
    array: usize,
    class: stompymux_unit_construction::MachineGunClass,
) -> Vec<u8> {
    let mut matching: Vec<_> = guns
        .iter()
        .filter(|(_, gun)| *gun == class)
        .map(|(slot, _)| *slot)
        .collect();
    matching.sort_by_key(|slot| (slot.abs_diff(array), *slot));
    matching.truncate(*stompymux_unit_construction::ARRAY_GUNS.end());
    guns.retain(|(slot, _)| !matching.contains(slot));
    matching.sort_unstable();
    matching.into_iter().map(|slot| slot as u8 + 1).collect()
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

    /// A vehicle location has no slot limit, while a Mech location holds twelve.
    #[test]
    fn vehicle_locations_take_more_than_twelve_items() {
        let mut criticals: Vec<_> = std::iter::repeat_n("ISMediumLaser", 11)
            .chain(["SRM 4", "ISArtemisIV"])
            .map(|name| parse(name).unwrap())
            .collect();
        let slots = slots(&criticals, WeaponSpan::Single).unwrap();
        assert_eq!(slots.len(), 13);
        assert_eq!(slots[12].as_ref().unwrap().link, Some(12));
        criticals.truncate(12);
        criticals.push(parse("ISMediumLaser").unwrap());
        assert!(super::slots(&criticals, WeaponSpan::Catalogue).is_err());
        assert_eq!(super::slots(&[], WeaponSpan::Single).unwrap().len(), 12);
    }

    /// A machine gun array links up to four machine guns of its own size class in its
    /// section, nearest first, wherever they sit, even past a vehicle location's twelfth item
    /// (TechManual p. 228).
    #[test]
    fn machine_gun_arrays_link_guns_of_their_class() {
        let mut names = vec!["ISMediumLaser"; 12];
        names.extend([
            "Machine Gun",
            "Light Machine Gun",
            "Machine Gun",
            "ISMGA",
            "Machine Gun",
            "Machine Gun",
            "Machine Gun",
            "ISLMGA",
            "Light Machine Gun",
        ]);
        let criticals: Vec<_> = names.iter().map(|name| parse(name).unwrap()).collect();
        let slots = slots(&criticals, WeaponSpan::Single).unwrap();
        let standard = slots[15].as_ref().unwrap();
        assert_eq!(standard.item, "IS.MGA");
        assert_eq!(standard.links, [13, 15, 17, 18]);
        let light = slots[19].as_ref().unwrap();
        assert_eq!(light.links, [14, 21]);
    }

    /// An Apollo system links to the MRM launcher in its section, passing over other
    /// launchers.
    #[test]
    fn apollo_links_to_an_mrm_launcher() {
        let criticals: Vec<_> = ["MRM 10", "MRM 10", "LRM 5", "ISApollo"]
            .into_iter()
            .map(|name| parse(name).unwrap())
            .collect();
        let slots = slots(&criticals, WeaponSpan::Catalogue).unwrap();
        let apollo = slots[3].as_ref().unwrap();
        assert_eq!((apollo.item.as_str(), apollo.link), ("ApolloFCS", Some(1)));
    }

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
