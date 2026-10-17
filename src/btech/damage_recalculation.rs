//! Rebuild Mech system baselines after a damage-field critical replacement.
use super::{BeaconKind, MP_KPH, Mech, SpeedEnvelope};
use anyhow::Result;

/// The construction envelope a recalculation adopts: the template's own walking MP and the
/// running MP derived from it (Total Warfare p. 53), or the envelope an administrative
/// template-speed edit states in km/h.
fn template_envelope(unit: &Mech) -> SpeedEnvelope {
    let derived = SpeedEnvelope::from_walking_mp(f64::from(unit.definition().walk_mp));
    let speed = unit.template_speed();
    if speed == derived.running {
        return derived;
    }
    SpeedEnvelope::from_running(speed)
}

/// Adopt owned construction and surviving equipment without combat rolls or template-file reloads.
/// Standard jump jets give no more Jumping MP than the walking MP, improved ones no more than
/// the running MP (TechManual p. 51).
pub(super) fn recalculate(unit: &mut Mech, gyro_protection_used: bool) -> Result<()> {
    let loadout = unit.loadout()?;
    unit.reconstructed_cooling = Some(super::engine_sink_capacity::reconstructed_capacity(unit)?);
    unit.heat_cutoff.clamp(unit.working_heat_sinks());
    let improved = unit.definition().has_special("ImprovedJJ_Tech");
    let installed = loadout.jump_jet_groups(improved)?.len();
    let envelope = template_envelope(unit);
    let maximum = if improved {
        envelope.running / MP_KPH
    } else {
        envelope.walking_mp()
    }
    .floor();
    let jump = (installed as f64).min(maximum) * MP_KPH;
    unit.propulsion.reconstruct(unit.template_speed(), jump);
    unit.reconstruct_gyro(gyro_protection_used);
    unit.hardware.tactical = None;
    unit.hardware.long_range = None;
    unit.hardware.scan = None;
    // Thermal production and cooling are derived from installed equipment at the next sample.
    // Recalculation does not advance that sample or replace stored heat.
    unit.beacons.retain(|_, kinds| {
        kinds.retain(|kind| *kind == BeaconKind::Narc);
        !kinds.is_empty()
    });
    let available: std::collections::BTreeSet<_> = loadout
        .weapons
        .iter()
        .enumerate()
        .filter(|(_, mount)| {
            mount
                .criticals
                .iter()
                .all(|location| !unit.critical_unavailable(*location))
        })
        .map(|(index, _)| index)
        .collect();
    unit.spent_launchers
        .retain(|index| !available.contains(index));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Jenner's template states 7 walking MP, so recalculation caps standard jump jets at
    /// 7 and improved ones at its 11 running MP; an administrative template-speed edit states
    /// the envelope in km/h instead.
    #[test]
    fn template_envelope_reads_the_template_walking_mp() {
        let template =
            super::super::MechTemplate::parse("JR7-D", include_str!("../../game/units/JR7-D.toml"))
                .unwrap();
        let mut unit = Mech::from_template(template).unwrap();
        let envelope = template_envelope(&unit);
        assert_eq!(envelope, SpeedEnvelope::from_walking_mp(7.0));
        assert_eq!(
            (envelope.walking_mp(), envelope.running / MP_KPH),
            (7.0, 11.0)
        );
        unit.set_template_speed(64.5);
        assert_eq!(
            template_envelope(&unit),
            SpeedEnvelope::from_walking_mp(4.0)
        );
    }
}
