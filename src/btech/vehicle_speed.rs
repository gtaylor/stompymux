//! A combat vehicle's cruising and flanking ceilings in km/h. Cruising MP is the template's
//! own `walk_mp` figure (Total Warfare p. 53; TechManual p. 100), motive damage, rotor hits
//! and an engine hit take cruising MP away (TW pp. 193, 195, 197), and flank is always the
//! cruising MP left x 1.5 rounded up, so every cruise gate reads the same figure.
use super::{Hex, MP_KPH, SpeedEnvelope, Vehicle, VehicleMovement};

/// Actual-speed bounds for a throttle `envelope`: pavement adds a point of movement to a
/// ground vehicle travelling in either direction (Total Warfare p. 61).
pub(super) fn paved_headroom(envelope: SpeedEnvelope) -> SpeedEnvelope {
    SpeedEnvelope {
        walking: envelope.walking + MP_KPH,
        running: envelope.running + MP_KPH,
    }
}

/// Actual-speed bounds for a vehicle of `movement` on `tile` whose throttle is held inside
/// `envelope`. A ground vehicle on pavement, a road or a bridge deck keeps the point of
/// movement the surface adds (Total Warfare p. 61), the same surfaces on which the motion
/// engine grants it; a VTOL, a vehicle off pavement and a stopped vehicle keep the envelope.
pub(super) fn actual_speed_bounds(
    tile: Hex,
    movement: VehicleMovement,
    envelope: SpeedEnvelope,
) -> SpeedEnvelope {
    if envelope.is_stopped() || !tile.is_paved() || movement == VehicleMovement::Vtol {
        return envelope;
    }
    paved_headroom(envelope)
}

/// The share by which the paved movement bonus speeds a ground vehicle's travel: one point of
/// movement over its flanking ceiling, so a flanking vehicle gains exactly one MP (Total
/// Warfare p. 61). A stopped envelope gains nothing.
pub(super) fn paved_bonus(envelope: SpeedEnvelope) -> f64 {
    if envelope.running <= 0.0 {
        return 1.0;
    }
    (envelope.running + MP_KPH) / envelope.running
}

/// The fastest actual speed at which a vehicle of `movement` on `tile` is still cruising. On
/// pavement a ground vehicle keeps the movement bonus whether it cruises or flanks (Total
/// Warfare p. 61), so its cruising ceiling rises by the same share its travel does.
pub(super) fn cruising_ceiling(
    tile: Hex,
    movement: VehicleMovement,
    envelope: SpeedEnvelope,
) -> f64 {
    if !tile.is_paved() || movement == VehicleMovement::Vtol {
        return envelope.walking;
    }
    envelope.walking * paved_bonus(envelope)
}

impl Vehicle {
    /// Cruising MP left: the template's cruising MP less what motive damage, rotor hits and
    /// an engine hit have taken (Total Warfare pp. 193, 195, 197).
    pub fn cruising_mp(&self) -> u32 {
        self.definition()
            .walk_mp
            .saturating_sub(self.motive_mp_loss())
    }

    /// Current cruising and flanking ceilings before towed load, extreme temperature and
    /// map gravity. Flank is the cruising MP left x 1.5 rounded up (Total Warfare p. 53). An
    /// immobilized, rotorless, destroyed or stationary vehicle has no movement at all, and an
    /// administrative maximum speed replaces the derived flank speed, cruising following it
    /// as it does for a 'Mech's running ceiling.
    pub fn speed_envelope(&self) -> SpeedEnvelope {
        if self.immobilized()
            || self.rotor_destroyed()
            || self.is_destroyed()
            || self.definition().movement == VehicleMovement::Stationary
        {
            return SpeedEnvelope::STOPPED;
        }
        let derived = SpeedEnvelope::from_walking_mp(f64::from(self.cruising_mp()));
        let maximum = self.propulsion.maximum(derived.running);
        if maximum != derived.running {
            return SpeedEnvelope::from_running(maximum);
        }
        derived
    }

    /// Current cruising ceiling: the threshold above which the vehicle is flanking, the
    /// fastest it may reverse, and the cap a stunned crew or tail-rotor hit leaves.
    pub fn cruising_speed(&self) -> f64 {
        self.speed_envelope().walking
    }

    /// Current flanking ceiling after motive damage, independently of load and map.
    pub fn maximum_speed(&self) -> f64 {
        self.speed_envelope().running
    }

    /// The widest envelope saved controls may retain: the current ceilings under the
    /// lightest gravity the rules allow, since gravity applies after every other movement
    /// modifier (Tactical Operations: Advanced Rules p. 53).
    pub(super) fn retained_envelope(&self) -> SpeedEnvelope {
        let envelope = self.speed_envelope();
        SpeedEnvelope {
            walking: super::speed_bonus::saved_limit(envelope.walking),
            running: super::speed_bonus::saved_limit(envelope.running),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 5/8 Flatbed Truck as built.
    fn flatbed() -> Vehicle {
        let template = super::super::VehicleTemplate::parse(
            "Flatbed_Truck",
            include_str!("../../game/units/Flatbed_Truck.toml"),
        )
        .unwrap();
        Vehicle::new(template).unwrap()
    }

    /// Cruising MP is the template's own 5 MP, 53.75 km/h, not two thirds of the 86 km/h
    /// flank speed (57.33 km/h); motive damage takes cruising MP and flank follows it as
    /// cruising x 1.5 rounded up (Total Warfare pp. 53, 193), and an engine hit or
    /// immobilization stops it.
    #[test]
    fn cruising_mp_comes_from_the_template_and_motive_damage() {
        use super::super::VehicleMotiveHit;
        let mut truck = flatbed();
        assert_eq!(truck.cruising_mp(), 5);
        assert_eq!(
            truck.speed_envelope(),
            SpeedEnvelope {
                walking: 53.75,
                running: 86.0
            }
        );
        truck.apply_motive_hit(VehicleMotiveHit::SpeedLoss { movement_points: 1 });
        assert_eq!((truck.motive_mp_loss(), truck.cruising_mp()), (1, 4));
        assert_eq!(
            (truck.cruising_speed(), truck.maximum_speed()),
            (43.0, 64.5)
        );
        truck.apply_motive_hit(VehicleMotiveHit::HalveSpeed);
        assert_eq!((truck.motive_mp_loss(), truck.cruising_mp()), (3, 2));
        assert_eq!(
            (truck.cruising_speed(), truck.maximum_speed()),
            (21.5, 32.25)
        );
        assert_eq!(truck.retained_envelope().walking, 43.0);
        truck.apply_motive_hit(VehicleMotiveHit::Immobilize);
        assert_eq!(truck.speed_envelope(), SpeedEnvelope::STOPPED);
        let mut wrecked = flatbed();
        wrecked.disable_engine();
        assert_eq!(wrecked.motive_mp_loss(), 5);
        assert_eq!(wrecked.speed_envelope(), SpeedEnvelope::STOPPED);
    }

    /// An administrative maximum replaces the derived flank speed and cruising follows it;
    /// motive damage then lowers it by the flank speed the derived figure loses.
    #[test]
    fn administrative_maximum_replaces_flank_and_loses_motive_damage() {
        use super::super::VehicleMotiveHit;
        let mut truck = flatbed();
        truck.propulsion.set(96.75);
        assert_eq!(truck.speed_envelope(), SpeedEnvelope::from_running(96.75));
        assert_eq!(truck.cruising_speed(), 64.5);
        truck.apply_motive_hit(VehicleMotiveHit::SpeedLoss { movement_points: 1 });
        assert_eq!(truck.maximum_speed(), 96.75 - 21.5);
    }

    /// Pavement headroom adds one movement point to both directions.
    #[test]
    fn paved_headroom_adds_one_point_each_way() {
        let headroom = paved_headroom(SpeedEnvelope::from_walking_mp(5.0));
        assert_eq!(headroom.walking, 64.5);
        assert_eq!(headroom.running, 96.75);
    }

    /// Load reconciliation bounds actual speed on the surfaces where the motion engine grants
    /// the paved point: bare pavement as well as roads and bridge decks (Total Warfare p. 61).
    /// Open ground, a VTOL and a stopped vehicle keep the throttle envelope.
    #[test]
    fn actual_speed_bounds_keep_the_paved_point_wherever_motion_grants_it() {
        use super::super::Terrain;
        let envelope = SpeedEnvelope::from_walking_mp(5.0);
        let headroom = paved_headroom(envelope);
        let pavement = Hex::new(Terrain::Pavement, 0);
        assert!(pavement.is_paved() && !pavement.is_road() && !pavement.has_bridge());
        let road = Hex::new(Terrain::Road, 0);
        let bridge = Hex::new(Terrain::Bridge, 1);
        let clear = Hex::new(Terrain::Clear, 0);
        for movement in [
            VehicleMovement::Wheeled,
            VehicleMovement::Tracked,
            VehicleMovement::Hover,
        ] {
            for paved in [pavement, road, bridge] {
                assert_eq!(actual_speed_bounds(paved, movement, envelope), headroom);
            }
            assert_eq!(actual_speed_bounds(clear, movement, envelope), envelope);
            assert_eq!(
                actual_speed_bounds(pavement, movement, SpeedEnvelope::STOPPED),
                SpeedEnvelope::STOPPED
            );
        }
        assert_eq!(
            actual_speed_bounds(pavement, VehicleMovement::Vtol, envelope),
            envelope
        );
    }

    /// A 5/8 vehicle cruises at its 53.75 km/h template figure off pavement; on pavement the
    /// movement bonus speeds a flanking vehicle by exactly one MP, and the cruising ceiling
    /// rises by the same share (Total Warfare pp. 53, 61). VTOLs never gain it.
    #[test]
    fn cruising_ceiling_keeps_the_paved_bonus() {
        use super::super::Terrain;
        let envelope = SpeedEnvelope::from_walking_mp(5.0);
        let clear = Hex::new(Terrain::Clear, 0);
        let paved = Hex::new(Terrain::Pavement, 0);
        assert!((paved_bonus(envelope) * envelope.running - 96.75).abs() < 1e-9);
        assert_eq!(paved_bonus(SpeedEnvelope::STOPPED), 1.0);
        for movement in [VehicleMovement::Wheeled, VehicleMovement::Hover] {
            assert_eq!(cruising_ceiling(clear, movement, envelope), 53.75);
            assert!(
                (cruising_ceiling(paved, movement, envelope) - 53.75 * 96.75 / 86.0).abs() < 1e-9
            );
        }
        assert_eq!(
            cruising_ceiling(paved, VehicleMovement::Vtol, envelope),
            53.75
        );
    }
}
