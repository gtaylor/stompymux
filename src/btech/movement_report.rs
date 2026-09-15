//! Movement consequences retained until the host publishes character feedback and casualties.

/// Ground and airborne notices with their applied fall and collision reports.
#[derive(Default)]
pub(super) struct MovementReport {
    /// Direct crew feedback captured before impacts can move or kill the pilot.
    pub pilot_notices: Vec<super::BattlePilotNotice>,
    pub boundaries: Vec<BattleBoundaryCrossing>,
    pub experience_messages: Vec<super::BattleChannelMessage>,
    pub dfas: Vec<super::BattleDfaReport>,
    pub charges: Vec<super::BattleChargeReport>,
    pub notices: Vec<super::BattleNotice>,
    pub character_injuries: Vec<super::BattleCharacterPilotInjury>,
    pub mines: Vec<super::BattleMineEventReport>,
    pub falls: Vec<super::BattleFallReport>,
    pub vehicle_falls: Vec<super::BattleVehicleFallReport>,
    pub stacking: super::stacking::StackingEffects,
}

/// An attempted unlinked edge retains controls for a host building-exit attempt.
#[derive(Clone)]
pub(super) struct BattleBoundaryCrossing {
    pub unit: crate::ObjectId,
    pub map: crate::ObjectId,
    pub motion: super::BattleMotion,
    pub notice: super::BattleNotice,
}

impl BattleBoundaryCrossing {
    /// Retain the exact fallback notice so successful exits can replace it.
    pub fn new(
        unit: crate::ObjectId,
        map: crate::ObjectId,
        motion: super::BattleMotion,
        text: &str,
    ) -> Self {
        Self {
            unit,
            map,
            motion,
            notice: super::BattleNotice {
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
