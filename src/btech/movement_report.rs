//! Movement consequences retained until the host publishes character feedback and casualties.

/// Ground and airborne notices with their applied fall and collision reports.
#[derive(Default)]
pub(super) struct MovementReport {
    /// Direct crew feedback captured before impacts can move or kill the pilot.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub boundaries: Vec<BoundaryCrossing>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub dfas: Vec<super::DfaReport>,
    pub charges: Vec<super::ChargeReport>,
    pub notices: Vec<super::Notice>,
    pub character_injuries: Vec<super::CharacterPilotInjury>,
    pub mines: Vec<super::MineEventReport>,
    pub falls: Vec<super::MechFallReport>,
    pub vehicle_falls: Vec<super::VehicleFallReport>,
    pub stacking: super::stacking::StackingEffects,
}

/// An attempted unlinked edge retains controls for a host building-exit attempt.
#[derive(Clone)]
pub(super) struct BoundaryCrossing {
    pub unit: crate::ObjectId,
    pub map: crate::ObjectId,
    pub motion: super::Motion,
    pub notice: super::Notice,
}

impl BoundaryCrossing {
    /// Retain the exact fallback notice so successful exits can replace it.
    pub fn new(
        unit: crate::ObjectId,
        map: crate::ObjectId,
        motion: super::Motion,
        text: &str,
    ) -> Self {
        Self {
            unit,
            map,
            motion,
            notice: super::Notice {
                unit,
                text: text.into(),
            },
        }
    }
}

impl MovementReport {
    /// Combine independently resolved movement phases without losing nested consequences.
    pub fn extend(&mut self, other: Self) {
        super::piloting::append_feedback(
            &mut self.pilot_notices,
            other.pilot_notices,
            self.notices.len(),
        );
        self.boundaries.extend(other.boundaries);
        self.experience_messages.extend(other.experience_messages);
        self.dfas.extend(other.dfas);
        self.charges.extend(other.charges);
        self.notices.extend(other.notices);
        self.character_injuries.extend(other.character_injuries);
        self.mines.extend(other.mines);
        self.falls.extend(other.falls);
        self.vehicle_falls.extend(other.vehicle_falls);
        self.stacking
            .experience_messages
            .extend(other.stacking.experience_messages);
        self.stacking.impacts.extend(other.stacking.impacts);
        self.stacking.falls.extend(other.stacking.falls);
    }
}
