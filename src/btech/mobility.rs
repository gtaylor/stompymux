//! Derive BattleMech mobility from persistent section and actuator damage.
use super::{CriticalLocation, Mech, MechChassis, SpeedEnvelope, System};
use serde::Serialize;

/// Damage-only movement limits; heat, cargo, terrain and pilot advantages are applied separately.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Mobility {
    /// Walking ceiling in km/h after leg and actuator damage.
    pub walking_speed: f64,
    /// Running ceiling in km/h: the damaged walking MP x 1.5 rounded up (Total Warfare p. 125).
    pub maximum_speed: f64,
    pub piloting_modifier: u8,
}

impl Mobility {
    /// The damage-only walking and running ceilings as one envelope.
    pub fn envelope(self) -> SpeedEnvelope {
        SpeedEnvelope {
            walking: self.walking_speed,
            running: self.maximum_speed,
        }
    }
}

impl Mech {
    /// Inherent chassis control bonus: a quad's -2 to Piloting Skill Rolls lasts only while
    /// none of its four legs is destroyed (Total Warfare p. 59, p. 123).
    pub fn chassis_piloting_modifier(&self) -> i16 {
        if self.unavailable_legs() > 0 {
            return 0;
        }
        self.chassis().piloting_modifier()
    }

    /// Recompute mobility from facts instead of accumulating mutable penalties after each hit.
    ///
    /// Walking MP is the template's own figure. A destroyed hip halves it, rounding up,
    /// before each other damaged leg actuator takes one more point (Total Warfare p. 125);
    /// running is then refigured as walking x 1.5 rounded up. On a biped, a destroyed hip
    /// supersedes the actuator hits in its own leg, which then cost neither MP nor piloting
    /// modifiers (p. 127); a quad counts every leg critical (p. 125). A destroyed leg adds
    /// +5 to a biped's rolls and leaves it walking at 1 MP without running (TW pp. 60, 122). A
    /// quad missing one leg loses one walking MP and its -2 bonus but takes no leg modifier
    /// of its own; missing two it is restricted like a one-legged biped, and three like a
    /// legless one (p. 123). Hardened armor costs one running MP throughout.
    pub fn mobility(&self) -> Mobility {
        let baseline =
            SpeedEnvelope::from_running(self.propulsion.baseline(self.definition().max_speed));
        let chassis = self.chassis();
        let legs = chassis.legs();
        let missing = self.unavailable_legs();
        let gyro = self.gyro_damage();
        let (mut envelope, leg_penalty, can_run) = match (chassis, missing) {
            (_, 0) => (baseline, 0, true),
            (MechChassis::Quad, 1) => (baseline.with_walking_mp_delta(-1.0), 0, true),
            (MechChassis::Quad, 2) => (SpeedEnvelope::from_walking_mp(1.0), 5, false),
            (MechChassis::Quad, _) => (SpeedEnvelope::STOPPED, 10, false),
            (MechChassis::Biped, 1) => (SpeedEnvelope::from_walking_mp(1.0), 5, false),
            (MechChassis::Biped, _) => (SpeedEnvelope::STOPPED, 10, false),
        };
        let mut piloting_modifier = leg_penalty + self.gyro_piloting_modifier();
        let mut hips = 0;
        let mut actuators = 0;
        for &section in legs {
            if self.leg_unavailable(section) {
                continue;
            }
            let damaged: Vec<_> = self.definition().sections[&section]
                .criticals
                .iter()
                .filter(|(slot, _)| {
                    self.critical_destroyed(CriticalLocation {
                        section,
                        slot: **slot,
                    })
                })
                .filter_map(|(_, part)| System::named(&part.equipment))
                .collect();
            let hip = damaged.contains(&System::ShoulderOrHip);
            if hip {
                hips += 1;
                piloting_modifier += 2;
            }
            // A biped's locked hip works as a crutch: the leg's other actuator hits no longer
            // cost MP or add to rolls (Total Warfare p. 127). A quad's leg damage is
            // cumulative (p. 125).
            if hip && chassis == MechChassis::Biped {
                continue;
            }
            let hit = damaged
                .iter()
                .filter(|system| {
                    matches!(
                        system,
                        System::UpperActuator | System::LowerActuator | System::HandOrFootActuator
                    )
                })
                .count() as u8;
            actuators += hit;
            piloting_modifier += hit;
        }
        // The hip halving applies before any actuator reduction (Total Warfare p. 125).
        for _ in 0..hips {
            let halved = (envelope.walking_mp() / 2.0).ceil();
            envelope = envelope.with_walking_mp_delta(halved - envelope.walking_mp());
        }
        if actuators > 0 {
            envelope = envelope.with_walking_mp_delta(-f64::from(actuators));
        }
        if !can_run {
            envelope.running = envelope.walking;
        }
        let hardened = self.hardened_speed_penalty();
        if hardened > 0.0 {
            envelope.running = (envelope.running - hardened).max(envelope.walking);
        }
        // An administrative ceiling replaces the derived running speed; walking follows it.
        if envelope.running != self.propulsion.maximum(envelope.running) {
            envelope = SpeedEnvelope::from_running(self.propulsion.maximum(envelope.running));
        }
        // Total Warfare p. 127: a biped's second or a quad's fourth hip critical leaves no MP.
        if hips == legs.len()
            || missing >= if chassis == MechChassis::Quad { 3 } else { 2 }
            || gyro >= 2
            || self.is_destroyed()
        {
            envelope = SpeedEnvelope::STOPPED;
        }
        Mobility {
            walking_speed: envelope.walking.max(0.0),
            maximum_speed: envelope.running.max(0.0),
            piloting_modifier,
        }
    }
}
