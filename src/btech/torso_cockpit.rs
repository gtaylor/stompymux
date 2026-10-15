//! Cockpit placement and sensor damage: a torso-mounted cockpit sits in the center torso, so
//! the Mech outlives its head, and its third sensor package lets it fight on through a second
//! sensor hit (Tactical Operations: Advanced Units & Equipment p. 113).
use super::{Mech, MechSection, System, Technology};

/// To-hit modifier that stands for weapons the sensors can no longer aim.
pub(super) const SENSORS_BLIND: u8 = 75;

impl Mech {
    /// Whether the Mech has a torso-mounted cockpit.
    pub fn torso_mounted_cockpit(&self) -> bool {
        self.definition()
            .has_technology(Technology::TorsoMountedCockpit)
    }

    /// The location holding the cockpit, whose loss kills the MechWarrior: the head, or the
    /// center torso for a torso-mounted cockpit.
    pub fn cockpit_section(&self) -> MechSection {
        if self.torso_mounted_cockpit() {
            MechSection::CenterTorso
        } else {
            MechSection::Head
        }
    }

    /// Whether a hit on the head must be rolled again: a torso-mounted cockpit's Mech survives
    /// its head, and later strikes there are re-rolled (Tactical Operations: Advanced Units &
    /// Equipment p. 113).
    pub fn rerolls_head_hits(&self) -> bool {
        self.torso_mounted_cockpit() && self.sections()[&MechSection::Head].internal == 0
    }

    /// Sensor criticals lost in the head, counting those of a destroyed head.
    fn head_sensor_hits(&self) -> usize {
        self.definition().sections[&MechSection::Head]
            .criticals
            .iter()
            .filter(|(slot, part)| {
                System::named(&part.equipment) == Some(System::Sensors)
                    && self.critical_destroyed(super::CriticalLocation {
                        section: MechSection::Head,
                        slot: **slot,
                    })
            })
            .count()
    }

    /// Weapon to-hit modifier for sensor damage. One hit adds 2 and a second stops weapon
    /// attacks (Total Warfare p. 124); behind a torso-mounted cockpit the second hit adds 4
    /// and only a third stops them (Tactical Operations: Advanced Units & Equipment p. 113).
    pub fn sensor_aim_modifier(&self) -> u8 {
        match (
            self.system_hits(System::Sensors),
            self.torso_mounted_cockpit(),
        ) {
            (0, _) => 0,
            (1, _) => 2,
            (2, true) => 4,
            _ => SENSORS_BLIND,
        }
    }

    /// Whether sensor damage has left the Mech unable to aim weapons.
    pub fn sensors_failed(&self) -> bool {
        self.sensor_aim_modifier() >= SENSORS_BLIND
    }

    /// Modifier to piloting rolls and physical attacks from a blinded torso-mounted cockpit:
    /// +4 once three sensors are lost, or once both head sensors are (Tactical Operations:
    /// Advanced Units & Equipment p. 113). Standard cockpits take none.
    pub fn blind_cockpit_modifier(&self) -> u8 {
        if !self.torso_mounted_cockpit() {
            return 0;
        }
        if self.system_hits(System::Sensors) >= 3 || self.head_sensor_hits() >= 2 {
            4
        } else {
            0
        }
    }
}
