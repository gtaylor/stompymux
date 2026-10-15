//! Spike clusters: they blunt physical attacks against their location and wound the limb that
//! struck them, and torso clusters sharpen a charge (Tactical Operations: Advanced Units &
//! Equipment p. 103).
use super::{CriticalLocation, FallRules, Hit, Mech, MechSection, Notice, System, TacticalImpact};
use crate::{ObjectId, World};
use anyhow::Result;

/// Points a spike cluster takes off each physical attack's damage against its location.
const ABSORBED: u16 = 4;

/// Damage the spikes deal the limbs that struck them.
const BACKLASH: u16 = 2;

/// Extra charge damage per working torso cluster.
const CHARGE_BONUS: u16 = 2;

/// Lowest 2d6 roll at which a cluster used in a charge is torn away when its location is
/// damaged.
const TORN_AWAY: u8 = 9;

impl Mech {
    /// The spike cluster in `section`, if one is installed and intact.
    pub fn working_spikes(&self, section: MechSection) -> Result<Option<CriticalLocation>> {
        Ok(self
            .loadout()?
            .systems
            .iter()
            .find(|part| part.system == System::Spikes && part.location.section == section)
            .map(|part| part.location)
            .filter(|location| !self.critical_unavailable(*location)))
    }

    /// Working spike clusters in the torso locations, which add to a charge's damage.
    pub fn torso_spike_clusters(&self) -> Result<u16> {
        let mut clusters = 0;
        for section in [
            MechSection::LeftTorso,
            MechSection::CenterTorso,
            MechSection::RightTorso,
        ] {
            clusters += u16::from(self.working_spikes(section)?.is_some());
        }
        Ok(clusters)
    }
}

/// Charge damage raised by two points per working torso cluster.
pub(super) fn charge_bonus(unit: &Mech) -> Result<u16> {
    Ok(CHARGE_BONUS * unit.torso_spike_clusters()?)
}

/// One physical damage group against a 'Mech location: the damage that lands after any spike
/// cluster there takes off four points (never below one), and whether one did.
pub(super) fn absorb(
    world: &World,
    id: ObjectId,
    section: MechSection,
    amount: u16,
) -> Result<(u16, bool)> {
    let Some(unit) = world.btech.constructed_units().get(&id) else {
        return Ok((amount, false));
    };
    if amount == 0 || unit.working_spikes(section)?.is_none() {
        return Ok((amount, false));
    }
    Ok((amount.saturating_sub(ABSORBED).max(1), true))
}

/// The two points of damage spikes deal the attacker, divided equally among the locations
/// that struck them: the center torso for a charge, the arm for a punch, the leg for a kick
/// and both legs for death from above. Clubs and physical weapons strike without the limb
/// touching the spikes and take none. The impacts' own notices are the caller's to publish.
pub(super) fn backlash(
    world: &mut World,
    attacker: ObjectId,
    sections: &[MechSection],
    rules: FallRules,
    character: bool,
    notices: &mut Vec<Notice>,
) -> Result<Vec<TacticalImpact>> {
    let mut impacts = Vec::new();
    if sections.is_empty() || world.btech.constructed_units()[&attacker].is_destroyed() {
        return Ok(impacts);
    }
    notices.push(Notice {
        unit: attacker,
        text: "You strike your target's spikes!".into(),
    });
    let each = BACKLASH / sections.len() as u16;
    for &section in sections {
        if each == 0 || world.btech.constructed_units()[&attacker].is_destroyed() {
            break;
        }
        let impact = super::impact::resolve_attack_in_candidate(
            world,
            attacker,
            Hit {
                section,
                rear_armor: false,
                through_armor_critical: false,
            },
            each,
            rules,
            super::impact::AttackImpact {
                attacker: None,
                weapon_effect: None,
                character,
                followup: true,
            },
        )?;
        impacts.push(impact);
    }
    Ok(impacts)
}

/// After a charge made with its torso spikes, roll 2d6 for each damaged location of the
/// charger that holds a cluster; on 9 or more the cluster is torn away.
pub(super) fn tear_away(
    world: &mut World,
    attacker: ObjectId,
    damaged: &[MechSection],
    notices: &mut Vec<Notice>,
) -> Result<()> {
    for &section in damaged {
        let unit = world.btech.constructed.get_mut(&attacker).unwrap();
        let Some(location) = unit.working_spikes(section)? else {
            continue;
        };
        if unit.dice.generic_roll() < TORN_AWAY {
            continue;
        }
        unit.destroy_critical(location)?;
        notices.push(Notice {
            unit: attacker,
            text: format!(
                "Your spikes in your {} are torn away!",
                unit.chassis().section_name(section).replace('_', " ")
            ),
        });
    }
    Ok(())
}
