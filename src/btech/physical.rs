//! Shared BattleMech physical targeting, damage and transactional leg, fist and hand-weapon attacks.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Seconds a physical attack ties up the attacker: one tabletop turn
/// ([`TURN_SECONDS`](super::spotter::TURN_SECONDS)). A 'Mech makes one physical attack a turn,
/// and the limbs it attacks with cannot fire their weapons in that turn (Total Warfare
/// pp. 144-149). The realtime game counts the turn from the moment of the attack.
pub(super) const PHYSICAL_RECOVERY_SECONDS: u16 = super::spotter::TURN_SECONDS as u16;

/// The leg used by a kick or trip; the other must still support the unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Leg {
    Left,
    Right,
}
impl Leg {
    /// The selected attack leg: front pair on quads.
    pub fn section(self, chassis: MechChassis) -> MechSection {
        match self {
            Self::Left if chassis == MechChassis::Quad => MechSection::LeftArm,
            Self::Right if chassis == MechChassis::Quad => MechSection::RightArm,
            _ => self.rear_section(),
        }
    }

    /// The leg on this side of a biped, or the rear leg on this side of a quad, which kicks
    /// into a quad's rear arc (Total Warfare p. 147).
    pub fn rear_section(self) -> MechSection {
        match self {
            Self::Left => MechSection::LeftLeg,
            Self::Right => MechSection::RightLeg,
        }
    }
}

/// An arm selected for a physical attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arm {
    Left,
    Right,
}

impl Arm {
    /// The attacking biped section.
    pub fn section(self) -> MechSection {
        match self {
            Self::Left => MechSection::LeftArm,
            Self::Right => MechSection::RightArm,
        }
    }
}

/// The physical attack determines reach, arc, base aim, damage and balance rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PhysicalAttack {
    Kick {
        leg: Leg,
    },
    Trip {
        leg: Leg,
    },
    Punch {
        arm: Arm,
    },
    /// A swing with the physical weapon installed in one arm; never a punch.
    Weapon {
        arm: Arm,
        weapon: ArmAttack,
    },
    Club,
}

impl PhysicalAttack {
    /// Limb that owns this attack's recovery timer for the attacking chassis.
    pub fn section(self, chassis: MechChassis) -> MechSection {
        match self {
            Self::Club => MechSection::RightArm,
            Self::Kick { leg } | Self::Trip { leg } => leg.section(chassis),
            Self::Punch { arm } | Self::Weapon { arm, .. } => arm.section(),
        }
    }

    /// Check chassis anatomy and supporting legs before geometric or timing eligibility, for an
    /// attack into the forward arc. A `Weapon` carrying `Punch` is malformed and rejected here,
    /// before any attack path uses it.
    pub fn validate_chassis_support(self, unit: &Mech) -> Result<()> {
        self.validate_anatomy(unit)?;
        self.validate_limb(unit, self.section(unit.chassis()))
    }

    /// Chassis checks that do not depend on which leg delivers a kick. Only biped 'Mechs trip
    /// (Tactical Operations: Advanced Rules p. 89); only bipeds have arms to punch, club or
    /// swing with.
    fn validate_anatomy(self, unit: &Mech) -> Result<()> {
        ensure!(
            self.hand_weapon() != Some(ArmAttack::Punch),
            "A punch is not a weapon swing"
        );
        let chassis = unit.chassis();
        if !self.uses_leg() {
            ensure!(
                chassis != MechChassis::Quad,
                "What are you going to {} with, your front right leg?",
                self.verb()
            );
            return Ok(());
        }
        ensure!(
            !self.is_trip() || chassis == MechChassis::Biped,
            "Only two-legged 'Mechs can trip."
        );
        let maximum_missing = usize::from(chassis == MechChassis::Quad);
        ensure!(
            unit.unavailable_legs() <= maximum_missing,
            "Too few supporting legs for this attack"
        );
        for &leg in chassis.legs() {
            if unit.leg_unavailable(leg) {
                continue;
            }
            ensure!(
                actuator(unit, leg, 0, System::ShoulderOrHip)?,
                "You cannot attack with a destroyed hip"
            );
        }
        Ok(())
    }

    /// The kicking or tripping leg must still be there.
    fn validate_limb(self, unit: &Mech, limb: MechSection) -> Result<()> {
        if !self.uses_leg() {
            return Ok(());
        }
        ensure!(
            !unit.leg_unavailable(limb),
            "The attacking leg is unavailable"
        );
        Ok(())
    }

    /// The limb that delivers the blow: the selected arm, or the kicking leg, which is a quad's
    /// rear leg on that side when it kicks into its rear arc (Total Warfare p. 147). Its
    /// actuators set the to-hit and damage modifiers.
    fn limb(self, chassis: MechChassis, rear_kick: bool) -> MechSection {
        match self {
            Self::Kick { leg } if rear_kick => leg.rear_section(),
            attack => attack.section(chassis),
        }
    }

    /// Sections the attack ties up for the turn. None of them may have fired a weapon this
    /// turn, and their weapons cannot fire until the turn is out: the punching or swinging arm,
    /// both arms for a club, and the kicking leg (Total Warfare pp. 145-147). A four-legged
    /// 'Mech kicking into its rear arc uses its rear legs, and may not do so after firing
    /// rear leg-mounted weapons that turn (Total Warfare p. 147).
    fn limbs(self, chassis: MechChassis, rear_kick: bool) -> Vec<MechSection> {
        match self {
            Self::Club => vec![MechSection::LeftArm, MechSection::RightArm],
            Self::Kick { .. } if rear_kick => vec![MechSection::LeftLeg, MechSection::RightLeg],
            attack => vec![attack.section(chassis)],
        }
    }

    /// Whether the attack uses leg reach and the real forward arc.
    fn uses_leg(self) -> bool {
        matches!(self, Self::Kick { .. } | Self::Trip { .. })
    }

    /// Trips force balance rather than applying direct impact damage.
    fn is_trip(self) -> bool {
        matches!(self, Self::Trip { .. })
    }

    /// Arm making a punch or weapon swing; its side arc is also reachable.
    fn arm(self) -> Option<Arm> {
        match self {
            Self::Punch { arm } | Self::Weapon { arm, .. } => Some(arm),
            _ => None,
        }
    }

    /// Installed arm weapon, including mechanical weapons that need no hand.
    fn hand_weapon(self) -> Option<ArmAttack> {
        match self {
            Self::Weapon { weapon, .. } => Some(weapon),
            _ => None,
        }
    }

    /// Player-facing attack verb.
    fn verb(self) -> &'static str {
        match self {
            Self::Kick { .. } => "kick",
            Self::Trip { .. } => "trip",
            Self::Punch { .. } => "punch",
            Self::Weapon { weapon, .. } => weapon.verb(),
            Self::Club => "club",
        }
    }
}

/// Physical arm attack selected independently of left/right/both arm selection.
/// Every variant except `Punch` is a physical weapon installed in the arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmAttack {
    Punch,
    Axe,
    Sword,
    Mace,
    Saw,
    Claw,
    RetractableBlade,
    Lance,
    Flail,
    WreckingBall,
    ChainWhip,
    SmallVibroblade,
    MediumVibroblade,
    LargeVibroblade,
}

impl ArmAttack {
    /// Hand weapons in the order an arm carrying several would swing them.
    pub const HAND_WEAPONS: [Self; 13] = [
        Self::Axe,
        Self::Sword,
        Self::Mace,
        Self::Saw,
        Self::Claw,
        Self::RetractableBlade,
        Self::Lance,
        Self::Flail,
        Self::WreckingBall,
        Self::ChainWhip,
        Self::SmallVibroblade,
        Self::MediumVibroblade,
        Self::LargeVibroblade,
    ];

    /// Bind one selected arm to the shared physical attack description.
    fn attack(self, arm: Arm) -> PhysicalAttack {
        match self {
            Self::Punch => PhysicalAttack::Punch { arm },
            weapon => PhysicalAttack::Weapon { arm, weapon },
        }
    }

    /// Hand weapon backed by an installed equipment family.
    pub fn from_system(system: System) -> Option<Self> {
        Self::HAND_WEAPONS
            .into_iter()
            .find(|weapon| weapon.system() == Some(system))
    }

    /// Equipment family backing a hand weapon; punches need none.
    pub fn system(self) -> Option<System> {
        Some(match self {
            Self::Punch => return None,
            Self::Axe => System::Axe,
            Self::Sword => System::Sword,
            Self::Mace => System::Mace,
            Self::Saw => System::DualSaw,
            Self::Claw => System::Claw,
            Self::RetractableBlade => System::RetractableBlade,
            Self::Lance => System::Lance,
            Self::Flail => System::Flail,
            Self::WreckingBall => System::WreckingBall,
            Self::ChainWhip => System::ChainWhip,
            Self::SmallVibroblade => System::SmallVibroblade,
            Self::MediumVibroblade => System::MediumVibroblade,
            Self::LargeVibroblade => System::LargeVibroblade,
        })
    }

    /// Lowercase weapon name used in cockpit and damage messages.
    pub fn name(self) -> &'static str {
        match self {
            Self::Punch => "fist",
            Self::Axe => "axe",
            Self::Sword => "sword",
            Self::Mace => "mace",
            Self::Saw => "dual saw",
            Self::Claw => "claw",
            Self::RetractableBlade => "retractable blade",
            Self::Lance => "lance",
            Self::Flail => "flail",
            Self::WreckingBall => "wrecking ball",
            Self::ChainWhip => "chain whip",
            Self::SmallVibroblade => "small vibroblade",
            Self::MediumVibroblade => "medium vibroblade",
            Self::LargeVibroblade => "large vibroblade",
        }
    }

    /// Attack verb; observers see it with a trailing "s".
    fn verb(self) -> &'static str {
        match self {
            Self::Punch => "punch",
            Self::Axe => "axe",
            Self::Sword => "chop",
            Self::Mace => "mace",
            Self::Saw => "saw",
            Self::Claw => "claw",
            Self::RetractableBlade => "cut",
            Self::Lance => "skewer",
            Self::Flail => "flail",
            Self::WreckingBall => "wreck",
            Self::ChainWhip => "whip",
            Self::SmallVibroblade | Self::MediumVibroblade | Self::LargeVibroblade => "slice",
        }
    }

    /// Critical slots a unit of this mass needs working in one arm to swing the weapon.
    pub fn minimum_slots(self, tons: u16) -> u16 {
        match self {
            Self::Punch => 0,
            Self::Axe | Self::Claw | Self::Sword => tons.div_ceil(15),
            Self::Mace => tons.div_ceil(10),
            Self::Saw => 7,
            Self::RetractableBlade => tons.div_ceil(20) + 1,
            Self::Lance => tons.div_ceil(20),
            Self::Flail => 4,
            Self::WreckingBall => 5,
            Self::ChainWhip => 2,
            Self::SmallVibroblade => 1,
            Self::MediumVibroblade => 2,
            Self::LargeVibroblade => 4,
        }
    }

    /// Whether the arm's hand actuator must work to swing this weapon. A retractable blade
    /// extends from the arm itself, so the hand does not matter (Total Warfare p. 146).
    pub fn needs_hand(self) -> bool {
        !matches!(
            self,
            Self::Saw
                | Self::Claw
                | Self::RetractableBlade
                | Self::Lance
                | Self::Flail
                | Self::WreckingBall
        )
    }

    /// Whether damaged or missing arm actuators halve this weapon's damage as they halve a
    /// punch (Total Warfare p. 146 Physical Weapon Attacks Table; TO:AUE pp. 100–103). Weapons
    /// with a fixed damage value, such as industrial saws, flails and active vibroblades, are
    /// harder to aim with a damaged arm but hit just as hard.
    pub fn actuators_reduce_damage(self) -> bool {
        matches!(
            self,
            Self::Punch
                | Self::Axe
                | Self::Sword
                | Self::Mace
                | Self::Claw
                | Self::RetractableBlade
                | Self::Lance
        )
    }

    /// Reduction applied to the pilot's piloting skill when aiming this attack.
    fn skill_bonus(self) -> i16 {
        match self {
            Self::Punch | Self::Claw => 0,
            Self::Sword
            | Self::RetractableBlade
            | Self::ChainWhip
            | Self::SmallVibroblade
            | Self::MediumVibroblade
            | Self::LargeVibroblade => 2,
            Self::Axe | Self::Mace | Self::Saw | Self::Lance | Self::Flail | Self::WreckingBall => {
                1
            }
        }
    }

    /// Aim penalty inherent to the weapon, applied after the skill bonus.
    fn weapon_modifier(self) -> u8 {
        match self {
            Self::Mace | Self::Lance | Self::WreckingBall => 2,
            Self::Saw | Self::Claw | Self::Flail => 1,
            _ => 0,
        }
    }

    /// Base damage for an attacker of this mass before myomer and actuator effects.
    pub(super) fn damage(self, tons: u16) -> u16 {
        match self {
            Self::Punch => tons.div_ceil(10),
            Self::Axe => tons / 5,
            // One point per five tons, rounded up (Tactical Operations: Advanced Units &
            // Equipment p. 101).
            Self::Lance => tons.div_ceil(5),
            Self::Sword => (tons + 5) / 10 + 1,
            Self::Mace => tons.div_ceil(4),
            Self::Saw => 7,
            Self::Claw => tons.div_ceil(7),
            Self::RetractableBlade => tons.div_ceil(10),
            Self::Flail => 9,
            Self::WreckingBall => 8,
            Self::ChainWhip => 3,
            Self::SmallVibroblade => 7,
            Self::MediumVibroblade => 10,
            Self::LargeVibroblade => 14,
        }
    }

    /// Whether active triple-strength myomer doubles this weapon's damage.
    pub(super) fn myomer_doubles(self) -> bool {
        !matches!(
            self,
            Self::Saw
                | Self::Flail
                | Self::WreckingBall
                | Self::ChainWhip
                | Self::SmallVibroblade
                | Self::MediumVibroblade
                | Self::LargeVibroblade
        )
    }

    /// Whether the weapon reaches only the forward arc. A wrecking ball swings straight ahead;
    /// every other physical weapon also reaches the side arc of its arm (Total Warfare p. 146;
    /// Tactical Operations: Advanced Rules p. 87).
    fn forward_only(self) -> bool {
        matches!(self, Self::WreckingBall)
    }

    /// Claws strike as a punch does, on the Punch Location Table (TO:AUE p. 101).
    fn strikes_as_punch(self) -> bool {
        matches!(self, Self::Punch | Self::Claw)
    }

    /// Heat added to the attacker each time the weapon is swung.
    fn heat(self) -> u8 {
        match self {
            Self::SmallVibroblade => 3,
            Self::MediumVibroblade => 5,
            Self::LargeVibroblade => 7,
            _ => 0,
        }
    }

    /// Damage the weapon deals its own wielder when the attack roll is a natural 2.
    fn fumble_damage(self) -> Option<u16> {
        match self {
            Self::Flail => Some(5),
            Self::WreckingBall => Some(4),
            _ => None,
        }
    }

    /// Whether completing this attack leaves the other arm free to attack in the same action.
    fn pairs(self) -> bool {
        matches!(self, Self::Punch | Self::Claw)
    }

    /// Hand weapon installed in an arm, whether or not enough of it still works to swing.
    pub(super) fn installed(unit: &Mech, section: MechSection) -> Result<Option<Self>> {
        let loadout = unit.loadout()?;
        Ok(Self::HAND_WEAPONS.into_iter().find(|kind| {
            loadout
                .systems
                .iter()
                .any(|part| Some(part.system) == kind.system() && part.location.section == section)
        }))
    }

    /// Count operational same-arm parts using the attack's observable minimum, not its mass divisor.
    pub(super) fn available(self, unit: &Mech, section: MechSection) -> Result<bool> {
        let Some(system) = self.system() else {
            return Ok(true);
        };
        let minimum = self.minimum_slots(unit.definition().tons);
        Ok(unit
            .loadout()?
            .systems
            .iter()
            .filter(|part| {
                part.system == system
                    && part.location.section == section
                    && !unit.critical_unavailable(part.location)
            })
            .count()
            >= usize::from(minimum))
    }
}

/// Inspect an arm attack without advancing dice or any recovery timer.
pub fn arm_attack_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    arm: Arm,
    kind: ArmAttack,
    rules: PhysicalRules,
) -> Result<PhysicalProfile> {
    attack_profile(
        world,
        attacker,
        pilot,
        target,
        kind.attack(arm),
        rules,
        None,
    )
}

/// Policy shared by physical inspection and attack resolution.
#[derive(Debug, Clone, Copy)]
pub struct PhysicalRules {
    pub fall: FallRules,
}

/// Read-only physical attack calculation. The attack roll and all consequences are separate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalProfile {
    pub attack: PhysicalAttack,
    pub target_number: i32,
    pub base: i16,
    pub actuators: u8,
    /// Aim penalty inherent to the selected physical weapon.
    pub weapon_modifier: u8,
    /// One for a four-legged 'Mech kicking into its rear arc (Total Warfare p. 147).
    pub rear_kick: u8,
    pub attacker_movement: i16,
    pub target_movement: i8,
    pub terrain: u8,
    /// Darkness on the target's map, less what searchlights offset.
    pub light: i16,
    /// Four while a torso-mounted cockpit's sensor damage leaves the attacker blind
    /// (Tactical Operations: Advanced Units & Equipment p. 113).
    pub sensors: u8,
    /// Direct material damage; trips always have zero.
    pub damage: u16,
    /// Location table for direct impacts; unused by trips and against vehicles, which take the
    /// blow on a randomly determined side (Total Warfare p. 147).
    pub hit_table: HitTable,
    /// The target is a combat vehicle rather than a 'Mech.
    pub vehicle_target: bool,
    /// The target is a VTOL, whose rotor a successful attack destroys (Total Warfare p. 198).
    pub vtol_target: bool,
    pub hit_arc: HitArc,
    /// The limb that delivers the blow, whose spikes or talons take part.
    pub limb: MechSection,
    /// Every limb the attack ties up for the turn.
    pub limbs: Vec<MechSection>,
}

/// Completed attack with already-applied damage, balance and fall consequences.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalReport {
    pub attacker: ObjectId,
    pub target: ObjectId,
    pub profile: PhysicalProfile,
    pub roll: u8,
    pub hit: bool,
    pub glancing: bool,
    /// Damage to a 'Mech target.
    pub impact: Option<TacticalImpact>,
    /// Damage to a vehicle target, on a randomly determined side.
    pub vehicle_impact: Option<VehicleImpact>,
    /// Lance armor-penetration check, made when armor remains where the lance struck.
    pub penetration: Option<LancePenetration>,
    /// The rotor a successful blow destroyed on a VTOL (Total Warfare p. 198).
    pub rotor: Option<RotorDamage>,
    /// Damage a flail or wrecking ball dealt its own wielder on a natural 2.
    pub fumble: Option<TacticalImpact>,
    /// Damage the target's spikes dealt the punching arm or kicking leg (Tactical Operations:
    /// Advanced Units & Equipment p. 103).
    pub spike_backlash: Vec<TacticalImpact>,
    /// Piloting XP applied before eligible direct damage; absent for trips and ineligible combat.
    pub experience: Option<ExperienceAward>,
    /// Accepted damage and control-check XP diagnostics captured during resolution.
    pub experience_messages: Vec<super::DiagnosticMessage>,
    /// Kick balance check, missed-mace balance, or target balance after a successful trip; absent for punches and missed trips.
    pub balance: Option<PilotingCheck>,
    pub fall: Option<MechFallReport>,
    /// Private balance feedback ordered among attack and fall notices.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Test one mechanical actuator at its canonical slot, including flooding and slot loss.
pub(super) fn actuator(
    unit: &Mech,
    section: MechSection,
    slot: u8,
    system: System,
) -> Result<bool> {
    let location = CriticalLocation { section, slot };
    Ok(!unit.critical_unavailable(location)
        && unit
            .loadout()?
            .systems
            .iter()
            .any(|part| part.location == location && part.system == system))
}

/// Inspect a kick without consuming dice or changing state.
pub fn kick_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    leg: Leg,
    rules: PhysicalRules,
) -> Result<PhysicalProfile> {
    attack_profile(
        world,
        attacker,
        pilot,
        target,
        PhysicalAttack::Kick { leg },
        rules,
        None,
    )
}

/// Inspect a trip without consuming dice or changing state. Trips have zero direct damage.
pub fn trip_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    leg: Leg,
    rules: PhysicalRules,
) -> Result<PhysicalProfile> {
    attack_profile(
        world,
        attacker,
        pilot,
        target,
        PhysicalAttack::Trip { leg },
        rules,
        None,
    )
}

/// Inspect one punch without consuming dice or changing state.
pub fn punch_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    arm: Arm,
    rules: PhysicalRules,
) -> Result<PhysicalProfile> {
    attack_profile(
        world,
        attacker,
        pilot,
        target,
        PhysicalAttack::Punch { arm },
        rules,
        None,
    )
}

/// Per-action arm recovery allowance and character casualty publication capability.
#[derive(Default)]
struct AttackContext {
    completed_arm: Option<MechSection>,
    character: bool,
}

/// Validate the attack envelope; only this action's first arm may already be recovering.
fn attack_profile(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: PhysicalAttack,
    rules: PhysicalRules,
    completed_arm: Option<MechSection>,
) -> Result<PhysicalProfile> {
    attack_profile_inner(
        world,
        attacker,
        pilot,
        target,
        attack,
        rules,
        AttackContext {
            completed_arm,
            character: false,
        },
    )
}

/// What a physical attack reads from its target before the geometry checks, whichever chassis
/// the target is. Vehicles never lie prone, rise or twist.
pub(super) struct TargetFacts {
    pub(super) vehicle: bool,
    /// A VTOL, whose rotor any successful physical attack by a 'Mech destroys (Total Warfare
    /// p. 198).
    pub(super) vtol: bool,
    /// A VTOL in flight, which the level-difference table for VTOLs governs (Total Warfare
    /// p. 198). A landed VTOL is attacked as any ground vehicle.
    pub(super) flying: bool,
    /// A four-legged 'Mech, which cannot be tripped (Tactical Operations: Advanced Rules p. 89).
    pub(super) quad: bool,
    pub(super) position: Position,
    pub(super) heading: f64,
    pub(super) team: i32,
    /// In the air beyond any physical attack's reach: a jumping or falling 'Mech, or a falling
    /// or dropping vehicle.
    pub(super) airborne: bool,
    pub(super) prone: bool,
    pub(super) rising: bool,
    pub(super) movement: i8,
}

/// Sample the target's live facts; vehicles use the shared immobility test and their speed.
pub(super) fn target_facts(world: &World, target: ObjectId) -> Result<TargetFacts> {
    if let Some(vehicle) = world.btech.vehicles().get(&target) {
        vehicle.validate()?;
        ensure!(!vehicle.is_destroyed(), "Unit is destroyed");
        let motion = vehicle.motion();
        let immobile = super::aimed_target::immobile(world, target)?;
        let phase = vehicle.vtol_flight().map(|flight| flight.phase);
        return Ok(TargetFacts {
            vehicle: true,
            vtol: vehicle.definition().is_vtol(),
            flying: phase == Some(VtolFlightPhase::Airborne),
            quad: false,
            position: vehicle
                .position()
                .context("Target is not on a battlefield")?,
            heading: motion.map_or(0.0, |motion| motion.heading),
            team: vehicle.signature().team,
            airborne: vehicle.free_fall().is_some()
                || vehicle.orbital_drop().is_some()
                || phase == Some(VtolFlightPhase::Falling),
            prone: false,
            rising: false,
            // A VTOL in the air is one harder to hit (Total Warfare p. 117).
            movement: super::aim::target_motion(motion.map_or(0.0, |motion| motion.speed))
                + i8::from(phase == Some(VtolFlightPhase::Airborne))
                - if immobile { 4 } else { 0 },
        });
    }
    let victim = world
        .btech
        .constructed_units()
        .get(&target)
        .context("Unit construction state is unavailable")?;
    victim.validate()?;
    ensure!(!victim.is_destroyed(), "Unit is destroyed");
    Ok(TargetFacts {
        vehicle: false,
        vtol: false,
        flying: false,
        quad: victim.chassis() == MechChassis::Quad,
        position: victim
            .position()
            .context("Target is not on a battlefield")?,
        heading: victim.motion().map_or(0.0, |motion| motion.heading),
        team: victim.signature().team,
        airborne: victim.airborne() || victim.free_fall().is_some(),
        prone: victim.posture() == Posture::Prone,
        rising: matches!(victim.stand_timer(), Some(StandTimer::Rising { .. })),
        movement: super::aim::ground_physical_target_modifier(world, victim),
    })
}

/// Which physical attacks reach a vehicle `difference` levels above the attacking 'Mech's own
/// level. A ground vehicle, or a landed VTOL, can be kicked, clubbed or struck with a physical
/// weapon on the 'Mech's own level, and punched, clubbed or struck from one level below it
/// (Total Warfare pp. 147, 150 and 198). A flying VTOL follows the Physical Attacks Against
/// VTOL Vehicles Table, counting from the 'Mech's hex level to the VTOL's elevation: level
/// with it everything but a punch, one or two levels up everything but a kick, three levels
/// up only clubs and physical weapons, and nothing lower or higher (Total Warfare p. 198).
fn vehicle_reach(attack: PhysicalAttack, difference: i32, flying: bool) -> Result<()> {
    let punch = matches!(attack, PhysicalAttack::Punch { .. });
    let kick = attack.uses_leg();
    let allowed = match (flying, difference) {
        (false, 0) | (true, 0) => !punch,
        (false, 1) => !kick,
        (true, 1 | 2) => !kick,
        (true, 3) => !punch && !kick,
        _ => false,
    };
    if allowed {
        return Ok(());
    }
    if !flying && difference == 1 {
        anyhow::bail!("You can only kick a vehicle on your own level.");
    }
    if !flying && difference == 0 {
        anyhow::bail!("The vehicle is too low for this arm attack.");
    }
    anyhow::bail!("You can't reach the vehicle with this attack from where you stand.")
}

/// Whether the unit's cockpit is running, whichever chassis it is.
pub(super) fn unit_running(world: &World, id: ObjectId) -> bool {
    world
        .btech
        .unit(id)
        .is_some_and(|unit| super::with_unit!(unit, |unit| unit.power() == Power::Running))
}

/// Shared targeting and equipment checks for either casualty publication mode.
fn attack_profile_inner(
    world: &World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: PhysicalAttack,
    rules: PhysicalRules,
    context: AttackContext,
) -> Result<PhysicalProfile> {
    let completed_arm = context.completed_arm;
    super::power::controlled_unit(world, attacker, pilot)?;
    ensure!(attacker != target, "You cannot attack yourself");
    for id in [attacker, target] {
        let object = world.objects.get(&id).context("Unit is unavailable")?;
        ensure!(!object.flags.contains(Flag::Going), "Unit is unavailable");
        ensure!(
            context.character || !object.flags.contains(Flag::InCharacter),
            "Physical attacks require non-character units"
        );
    }
    let source = world
        .btech
        .constructed_units()
        .get(&attacker)
        .context("Unit construction state is unavailable")?;
    source.validate()?;
    ensure!(!source.is_destroyed(), "Unit is destroyed");
    let victim = target_facts(world, target)?;
    super::building_interior::check_physical(world, attacker, target)?;
    attack.validate_anatomy(source)?;
    ensure!(source.power() == Power::Running, "Start the unit first");
    ensure!(
        !source.airborne() && source.free_fall().is_none(),
        "You can't perform physical attacks while in the air!"
    );
    ensure!(
        source.stand_timer().is_none(),
        "You are still trying to stand up!"
    );
    ensure!(
        source
            .limb_recycle
            .keys()
            .all(|section| Some(*section) == completed_arm)
            && !source
                .limb_recycle
                .contains_key(&attack.section(source.chassis())),
        "Your limbs are still recovering from another attack"
    );
    // A prone 'Mech may only punch a ground vehicle in its own hex, propping itself up on the
    // other arm (Total Warfare p. 151); every other physical attack needs it standing.
    let prone_punch = source.posture() == Posture::Prone;
    if prone_punch {
        ensure!(
            matches!(attack, PhysicalAttack::Punch { .. }) && victim.vehicle && !victim.flying,
            "You cannot attack a mech from a prone position."
        );
        ensure!(
            [MechSection::LeftArm, MechSection::RightArm]
                .iter()
                .all(|arm| source.sections()[arm].internal > 0),
            "You need both arms to punch from the ground."
        );
        ensure!(
            completed_arm.is_none(),
            "You need your other arm to prop yourself up."
        );
    }
    ensure!(
        source.stun_remaining() == 0,
        "You are still recovering from your stunning experience!"
    );
    if matches!(attack, PhysicalAttack::Punch { .. }) {
        ensure!(
            source.carried_club.map(Arm::section) != Some(attack.section(source.chassis())),
            "You are carrying a club in this arm and cannot punch with it"
        );
    }
    let club = attack == PhysicalAttack::Club;
    // Claws strike as punches, so a damaged shoulder bars them as it bars a punch (Total
    // Warfare p. 145; Tactical Operations: Advanced Units & Equipment p. 101).
    let required_sections = if club {
        vec![MechSection::LeftArm, MechSection::RightArm]
    } else if attack.uses_leg() && source.chassis() == MechChassis::Biped {
        vec![MechSection::LeftLeg, MechSection::RightLeg]
    } else {
        vec![attack.section(source.chassis())]
    };
    for section in required_sections {
        if club {
            ensure!(
                actuator(source, section, 3, System::HandOrFootActuator)?,
                "Both hands must work to club"
            );
        }
        ensure!(
            source.sections()[&section].internal > 0,
            "The required limb is destroyed"
        );
        ensure!(
            actuator(source, section, 0, System::ShoulderOrHip)?,
            "The required hip or shoulder is destroyed"
        );
    }
    if let Some(kind) = attack.hand_weapon() {
        ensure!(
            kind.available(source, attack.section(source.chassis()))?,
            "No usable {:?} installed in this arm",
            kind
        );
        ensure!(
            !kind.needs_hand()
                || actuator(
                    source,
                    attack.section(source.chassis()),
                    3,
                    System::HandOrFootActuator
                )?,
            "The required hand is destroyed or missing"
        );
    }
    ensure!(
        !victim.airborne,
        "You can't perform physical attacks on airborne units!"
    );
    if attack.is_trip() {
        ensure!(!victim.vehicle, "You cannot trip a vehicle!");
        // Only biped 'Mechs can be tripped (Tactical Operations: Advanced Rules p. 89).
        ensure!(!victim.quad, "You cannot trip a four-legged 'Mech!");
        ensure!(
            !victim.prone && !victim.rising,
            "Your target is already down!"
        );
    }
    let position = source.position().context("Unit is not on a battlefield")?;
    let target_position = victim.position;
    ensure!(
        position.map == target_position.map,
        "Target is on another battlefield"
    );
    ensure!(
        !prone_punch || (position.x, position.y) == (target_position.x, target_position.y),
        "From the ground you can only punch a vehicle in your own hex."
    );
    let map = &world.btech.maps()[&position.map];
    if source.signature().team == victim.team {
        ensure!(
            !source.friendly_fire_safety(),
            "You can't attack a teammate with FFSafeties on!"
        );
        ensure!(
            !map.blocks_friendly_fire(),
            "Friendly Fire? I don't think so..."
        );
    }
    // A physical attack reaches the attacker's own hex and the six around it, counted in
    // hexes as weapon range is (Total Warfare p. 144). Height never adds to the reach: the
    // level rules below decide which attacks a level difference allows (Total Warfare p. 150).
    let range = unit_range(world, attacker, target)?;
    ensure!(range.hexes() <= 1.0, "Target out of range!");
    ensure!(
        super::visibility::unit_unblocked(world, attacker, target)?,
        "Target is not in line of sight!"
    );
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    // A club must be picked up first, in a turn of its own (Total Warfare p. 146).
    if club {
        ensure!(
            source.carried_club.is_some(),
            "You are not carrying a club. Pick one up first."
        );
    }
    let target_tile = map.base_hex(i64::from(target_position.x), i64::from(target_position.y))?;
    let elevation = source.elevation_level(tile);
    let target_elevation = super::with_unit!(
        world.btech.unit(target).context("Unit is unavailable")?,
        |unit| unit.elevation_level(target_tile)
    );
    // A 'Mech reaches another unit only within one level of its own; a flying VTOL follows
    // its own table (Total Warfare pp. 150 and 198).
    ensure!(
        victim.flying || (elevation - target_elevation).abs() <= 1,
        "You can't attack, the elevation difference is too large."
    );
    // A 'Mech wading in water cannot reach a target under the surface, unless the whole attack
    // happens underwater, as a kick does (Total Warfare p. 145).
    ensure!(
        attack.uses_leg()
            || !(tile.immerses(elevation)
                && submerged_at(victim.prone, target_tile, target_elevation)),
        "You cannot reach a target under the water from where you stand."
    );
    if victim.vehicle {
        // Lying down puts the 'Mech a level lower than it stands (Total Warfare p. 150).
        let reach = elevation - i32::from(prone_punch);
        vehicle_reach(attack, target_elevation - reach, victim.flying)?;
    } else if attack.uses_leg() {
        ensure!(
            elevation >= target_elevation,
            "The target is too high in elevation for you to kick at."
        );
        ensure!(
            elevation == target_elevation || !victim.prone,
            "The target is too low in elevation for you to kick."
        );
    } else {
        ensure!(
            (!matches!(attack, PhysicalAttack::Punch { .. }) || elevation <= target_elevation)
                && (elevation < target_elevation || !victim.prone),
            "The target is too low for this arm attack."
        );
    }
    let heading = source.motion().map_or(0.0, |motion| motion.heading);
    let twist = if attack.uses_leg() {
        0.0
    } else {
        source.facing().torso.offset()
    };
    let angle = (range.bearing.unwrap_or(heading) - heading - twist).rem_euclid(360.0);
    let forward = angle <= 60.0 || angle >= 300.0;
    let side = match attack.arm() {
        _ if attack.hand_weapon().is_some_and(ArmAttack::forward_only) => false,
        Some(Arm::Left) => (240.0..300.0).contains(&angle),
        Some(Arm::Right) => angle > 60.0 && angle <= 120.0,
        None => false,
    };
    // A four-legged 'Mech can also kick behind itself, at +1 (Total Warfare p. 147).
    let rear_kick = matches!(attack, PhysicalAttack::Kick { .. })
        && source.chassis() == MechChassis::Quad
        && (120.0..=240.0).contains(&angle);
    ensure!(
        forward || side || rear_kick,
        "Target is outside the attacking limb's arc!"
    );
    let limb = attack.limb(source.chassis(), rear_kick);
    attack.validate_limb(source, limb)?;
    // A limb that fired this turn cannot also attack physically, however short its weapons'
    // recycle (Total Warfare pp. 145-147).
    let now = world.btech.simulation_time();
    // A prone punch ties up the propping arm as well as the punching one.
    let limbs = if prone_punch {
        vec![MechSection::LeftArm, MechSection::RightArm]
    } else {
        attack.limbs(source.chassis(), rear_kick)
    };
    for &section in &limbs {
        ensure!(
            !source.section_fired_this_turn(section, now),
            "Your {} fired weapons this turn and cannot also make a physical attack.",
            source.chassis().section_name(section).replace('_', " ")
        );
    }
    let upper = actuator(source, limb, 1, System::UpperActuator)?;
    let lower = actuator(source, limb, 2, System::LowerActuator)?;
    let foot = actuator(source, limb, 3, System::HandOrFootActuator)?;
    // A club takes +2 for each damaged or missing upper or lower actuator in either arm
    // (Total Warfare p. 146).
    let actuators = if club {
        let mut penalty = 0;
        for section in [MechSection::LeftArm, MechSection::RightArm] {
            for (slot, system) in [(1, System::UpperActuator), (2, System::LowerActuator)] {
                penalty += 2 * u8::from(!actuator(source, section, slot, system)?);
            }
        }
        penalty
    } else if attack.is_trip() {
        0
    } else {
        2 * u8::from(!upper)
            + 2 * u8::from(!lower)
            + u8::from(!foot && attack.hand_weapon().is_none())
    };
    let specialist = world
        .btech
        .character_values()
        .get(&pilot)
        .is_some_and(|values| super::advantages::enabled(values, "Melee_Specialist"));
    // Kick -2, trip and club -1 (Total Warfare p. 144; Tactical Operations: Advanced Rules
    // p. 89).
    let skill_bonus = match attack {
        PhysicalAttack::Kick { .. } => 2,
        PhysicalAttack::Trip { .. } | PhysicalAttack::Club => 1,
        PhysicalAttack::Punch { .. } => 0,
        PhysicalAttack::Weapon { weapon, .. } => weapon.skill_bonus(),
    };
    let base = unit_piloting_target(world, attacker, rules.fall.extended_piloting)? - skill_bonus;
    let weapon_modifier = attack.hand_weapon().map_or(0, ArmAttack::weapon_modifier);
    let rear_kick = u8::from(rear_kick);
    let movement = i16::from(source.attacker_movement_modifier());
    let attacker_movement = if specialist {
        (movement - 1).min(0)
    } else {
        movement
    };
    let target_movement = victim.movement;
    // Woods and smoke over the target's hex obscure it, light counting one and heavy two.
    let obscured = map.hex(i64::from(target_position.x), i64::from(target_position.y))?;
    let terrain = obscured.woods_density() + obscured.smoke_density();
    let light = super::light_aim::modifier(world, target, true)?;
    let sensors = source.blind_cockpit_modifier();
    let tons = source.definition().tons;
    let mut damage = match attack {
        PhysicalAttack::Punch { .. } => ArmAttack::Punch.damage(tons),
        PhysicalAttack::Weapon { weapon, .. } => weapon.damage(tons),
        _ => tons / 5,
    };
    if source.triple_myomer_active() && attack.hand_weapon().is_none_or(ArmAttack::myomer_doubles) {
        damage *= 2;
    }
    // Working talons in the kicking leg raise the kick by half, after any triple strength
    // myomer doubling (Tactical Operations: Advanced Units & Equipment p. 103).
    if matches!(attack, PhysicalAttack::Kick { .. }) && source.talons_working(limb)? {
        damage = super::talons::raise(damage);
    }
    damage += u16::from(specialist);
    // Each damaged or missing arm or leg actuator halves the blow, rounding down, to a minimum
    // of one point (Total Warfare pp. 145–147).
    let reduced = !club
        && attack
            .hand_weapon()
            .unwrap_or(ArmAttack::Punch)
            .actuators_reduce_damage();
    if reduced && !lower {
        damage = (damage / 2).max(1);
    }
    if reduced && !upper {
        damage = (damage / 2).max(1);
    }
    if attack.is_trip() {
        damage = 0;
    }
    let direction = unit_range(world, target, attacker)?;
    let hit_arc = HitArc::from_bearing(direction.bearing.unwrap_or(180.0), victim.heading)?;
    let punches = attack
        .hand_weapon()
        .unwrap_or(ArmAttack::Punch)
        .strikes_as_punch()
        && !club
        && !attack.uses_leg();
    Ok(PhysicalProfile {
        attack,
        target_number: i32::from(base)
            + i32::from(actuators)
            + i32::from(weapon_modifier)
            + i32::from(rear_kick)
            + i32::from(attacker_movement)
            + i32::from(target_movement)
            + i32::from(terrain)
            + i32::from(light)
            + i32::from(sensors),
        base,
        actuators,
        weapon_modifier,
        rear_kick,
        attacker_movement,
        target_movement,
        terrain,
        light,
        sensors,
        damage,
        hit_table: if victim.prone || victim.vehicle {
            HitTable::Weapon
        } else if elevation > target_elevation || (punches && elevation == target_elevation) {
            HitTable::Punch
        } else if (attack.hand_weapon().is_some() || club) && elevation == target_elevation {
            HitTable::Weapon
        } else {
            HitTable::Kick
        },
        vehicle_target: victim.vehicle,
        vtol_target: victim.vtol,
        hit_arc,
        limb,
        limbs,
    })
}

/// Whether a 'Mech at `elevation` in `tile` is under the water: standing in water two or more
/// levels deep, or lying in water at all.
pub(super) fn submerged(unit: &Mech, tile: Hex, elevation: i32) -> bool {
    submerged_at(unit.posture() == Posture::Prone, tile, elevation)
}

/// Whether a unit standing (or lying, when `prone`) at `elevation` in `tile` is under the water.
fn submerged_at(prone: bool, tile: Hex, elevation: i32) -> bool {
    let reach = if prone { 0 } else { 1 };
    tile.holds_water() && elevation + reach < i32::from(tile.water_line())
}

/// Derive the affected pilot's protection while preserving explicitly supplied fall policy,
/// whichever chassis the participant is.
pub(super) fn participant_fall_rules(
    world: &World,
    id: ObjectId,
    mut rules: FallRules,
) -> FallRules {
    let pilot = world
        .btech
        .unit(id)
        .and_then(|unit| super::with_unit!(unit, |unit| unit.pilot()));
    rules.toughness |= pilot
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    rules
}

/// Resolve one kick atomically, including its balance check.
pub fn resolve_kick(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    leg: Leg,
    rules: PhysicalRules,
) -> Result<PhysicalReport> {
    resolve_attack(
        world,
        attacker,
        pilot,
        target,
        PhysicalAttack::Kick { leg },
        rules,
        None,
    )
}

/// Resolve a trip atomically; only a successful attempt forces the target's balance check.
pub fn resolve_trip(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    leg: Leg,
    rules: PhysicalRules,
) -> Result<PhysicalReport> {
    resolve_attack(
        world,
        attacker,
        pilot,
        target,
        PhysicalAttack::Trip { leg },
        rules,
        None,
    )
}

/// Resolve a validated physical attack without publishing an incomplete impact cascade.
fn resolve_attack(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: PhysicalAttack,
    rules: PhysicalRules,
    completed_arm: Option<MechSection>,
) -> Result<PhysicalReport> {
    resolve_attack_inner(
        world,
        attacker,
        pilot,
        target,
        attack,
        rules,
        AttackContext {
            completed_arm,
            character: false,
        },
    )
}

/// Single physical attack inside a host action that owns injury and casualty publication.
pub(super) fn resolve_attack_in_action(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: PhysicalAttack,
    rules: PhysicalRules,
) -> Result<PhysicalReport> {
    resolve_attack_inner(
        world,
        attacker,
        pilot,
        target,
        attack,
        rules,
        AttackContext {
            completed_arm: None,
            character: true,
        },
    )
}

/// The side a vehicle takes a physical blow on, chosen by one die (Total Warfare p. 147).
fn vehicle_side(roll: u16) -> HitArc {
    match roll {
        1 => HitArc::Front,
        2 => HitArc::Left,
        3 => HitArc::Right,
        _ => HitArc::Rear,
    }
}

/// Apply the shared hit, damage and balance sequence with explicit publication capability.
fn resolve_attack_inner(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    attack: PhysicalAttack,
    rules: PhysicalRules,
    context: AttackContext,
) -> Result<PhysicalReport> {
    let character = context.character;
    let profile = attack_profile_inner(world, attacker, pilot, target, attack, rules, context)?;
    world.attempt(|world| {
        let unit = world.btech.constructed.get_mut(&attacker).unwrap();
        let roll = unit.dice.generic_roll();
        for &section in &profile.limbs {
            unit.limb_recycle.insert(section, PHYSICAL_RECOVERY_SECONDS);
        }
        let weapon = attack.hand_weapon();
        unit.heat.stored += f64::from(weapon.map_or(0, ArmAttack::heat));
        let threshold = profile.target_number;
        // A natural 2 swings flails and wrecking balls back into their wielder.
        let fumble = weapon
            .and_then(ArmAttack::fumble_damage)
            .filter(|_| roll == 2);
        let hit = fumble.is_none() && i32::from(roll) >= threshold;
        // Tactical Operations glancing blows: meeting the target number exactly halves damage.
        let glancing = hit && i32::from(roll) == threshold;
        let mut pilot_notices = Vec::new();
        let mut notices = vec![Notice {
            unit: attacker,
            text: format!(
                "You try to {} #{}.  BTH:  {},\tRoll:  {}",
                attack.verb(),
                target.0,
                profile.target_number,
                roll
            ),
        }];
        if unit_running(world, target) {
            notices.push(Notice {
                unit: target,
                text: format!("#{} tries to {} you!", attacker.0, attack.verb()),
            });
        }
        notices.extend(super::broadcast::interaction_notices(
            world,
            attacker,
            target,
            &if hit {
                format!("{}s", attack.verb())
            } else {
                format!("attempts to {}", attack.verb())
            },
        ));
        let mut impact = None;
        let mut vehicle_impact = None;
        let mut penetration = None;
        let mut spike_backlash = Vec::new();
        let mut rotor = None;
        let mut experience = None;
        let mut experience_messages = Vec::new();
        if glancing {
            notices.extend(super::broadcast::observer_notices(
                world,
                target,
                "is nicked by a glancing blow!",
            ));
            notices.push(Notice {
                unit: target,
                text: "You are nicked by a glancing blow!".into(),
            });
        }
        if hit && attack == PhysicalAttack::Club {
            let unit = world.btech.constructed.get_mut(&attacker).unwrap();
            if unit.carried_club.take().is_some() {
                notices.push(Notice {
                    unit: attacker,
                    text: "Your club shatters on contact.".into(),
                });
                notices.extend(super::broadcast::observer_notices(
                    world,
                    attacker,
                    "'s club shatters with a loud *CRACK*!",
                ));
            }
        }
        if hit && !attack.is_trip() {
            // A glancing blow does half damage, rounded down (Tactical Operations p. 78).
            let damage = if glancing {
                profile.damage / 2
            } else {
                profile.damage
            };
            if character
                && let Some(award) = super::physical_experience::award(
                    world,
                    attacker,
                    pilot,
                    target,
                    damage,
                    crate::clock::wall_time(),
                    rules.fall.extended_piloting,
                )?
            {
                experience = Some(award.award);
                experience_messages.extend(award.message);
            }
            let fall_rules = participant_fall_rules(world, target, rules.fall);
            if profile.vehicle_target {
                let side = world
                    .btech
                    .vehicles
                    .get_mut(&target)
                    .context("Vehicle target is unavailable")?
                    .dice
                    .die(4)?;
                let (result, result_notices) = super::blast_damage::resolve_packet(
                    world,
                    target,
                    super::blast_damage::MaterialPacket {
                        amount: damage,
                        table: HitTable::Weapon,
                        arc: vehicle_side(side),
                        character,
                        attacker: Some(attacker),
                        class: DamageClass::Impact,
                    },
                    false,
                    fall_rules,
                )?;
                result.append_feedback(&mut pilot_notices, notices.len());
                notices.extend(result_notices);
                if let super::blast_damage::BlastImpact::Vehicle(result) = result {
                    vehicle_impact = Some(result);
                }
                // Any successful physical attack by a 'Mech destroys a VTOL's rotor (Total
                // Warfare p. 198); a VTOL in the air then loses its lift.
                let vehicle = &world.btech.vehicles()[&target];
                if profile.vtol_target && !vehicle.is_destroyed() && !vehicle.rotor_destroyed() {
                    let damage = super::rotor_damage::apply_in_world(
                        world,
                        target,
                        RotorHit::Destroy,
                        rules.fall.extended_piloting,
                    )?;
                    notices.push(Notice {
                        unit: attacker,
                        text: "Your blow tears the rotor apart!".into(),
                    });
                    notices.push(Notice {
                        unit: target,
                        text: damage.message().into(),
                    });
                    notices.extend(super::broadcast::observer_notices(
                        world,
                        target,
                        "'s rotor is torn apart!",
                    ));
                    rotor = Some(damage);
                }
            } else {
                let victim = &world.btech.constructed_units()[&target];
                let mut dice = victim.dice.clone();
                let location = if profile.hit_table == HitTable::Weapon {
                    let roll = dice.generic_roll();
                    super::resolve_hit(victim, profile.hit_arc, roll, &mut dice)?
                } else {
                    Hit {
                        section: super::hit::d6_location(
                            victim,
                            profile.hit_table,
                            profile.hit_arc,
                            &mut dice,
                        )?,
                        rear_armor: profile.hit_arc == HitArc::Rear,
                        through_armor_critical: false,
                    }
                };
                world.btech.constructed.get_mut(&target).unwrap().dice = dice;
                // A spike cluster where the blow lands takes four points off it.
                let (damage, spiked) =
                    super::spikes::absorb(world, target, location.section, damage)?;
                // A glancing blow is also less likely to find a critical (TO:AR p. 78).
                let result = super::impact::resolve_attack_in_candidate_with_penalty(
                    world,
                    target,
                    location,
                    damage,
                    fall_rules,
                    super::impact::AttackImpact {
                        attacker: Some(attacker),
                        weapon_effect: Some(super::impact::WeaponEffect::Impact),
                        character,
                        followup: false,
                    },
                    if glancing {
                        super::impact::GLANCING_CRITICAL_PENALTY
                    } else {
                        0
                    },
                )?;
                super::piloting::append_feedback(
                    &mut pilot_notices,
                    result.pilot_notices.clone(),
                    notices.len(),
                );
                notices.extend(result.notices.iter().cloned());
                impact = Some(result);
                // A bare arm or leg that struck the spikes takes two points itself.
                if spiked && weapon.is_none() && attack != PhysicalAttack::Club {
                    let attacker_rules = participant_fall_rules(world, attacker, rules.fall);
                    spike_backlash = super::spikes::backlash(
                        world,
                        attacker,
                        &[profile.limb],
                        attacker_rules,
                        character,
                        &mut notices,
                    )?;
                    for result in &spike_backlash {
                        super::piloting::append_feedback(
                            &mut pilot_notices,
                            result.pilot_notices.clone(),
                            notices.len(),
                        );
                        notices.extend(result.notices.iter().cloned());
                    }
                }
                if weapon == Some(ArmAttack::Lance) {
                    penetration = lance_penetration(
                        world,
                        attacker,
                        target,
                        location,
                        rules,
                        character,
                        &mut notices,
                        &mut pilot_notices,
                    )?;
                }
            }
        }
        let mut fumble_impact = None;
        if let Some(damage) = fumble {
            let name = weapon.map_or("weapon", ArmAttack::name);
            notices.push(Notice {
                unit: attacker,
                text: format!("Your {name} swings wide and slams into you!"),
            });
            notices.extend(super::broadcast::observer_notices(
                world,
                attacker,
                &format!("is struck by its own {name}!"),
            ));
            let source = &world.btech.constructed_units()[&attacker];
            let mut dice = source.dice.clone();
            let roll = dice.generic_roll();
            let location = super::resolve_hit(source, HitArc::Front, roll, &mut dice)?;
            world.btech.constructed.get_mut(&attacker).unwrap().dice = dice;
            let fall_rules = participant_fall_rules(world, attacker, rules.fall);
            let result = super::impact::resolve_attack_in_candidate(
                world,
                attacker,
                location,
                damage,
                fall_rules,
                super::impact::AttackImpact {
                    attacker: None,
                    weapon_effect: Some(super::impact::WeaponEffect::Impact),
                    character,
                    followup: false,
                },
            )?;
            super::piloting::append_feedback(
                &mut pilot_notices,
                result.pilot_notices.clone(),
                notices.len(),
            );
            notices.extend(result.notices.iter().cloned());
            fumble_impact = Some(result);
        }
        // Unit making a balance check after this attack, and the check's modifier. Vehicles
        // never make one: only 'Mechs fall.
        let mech_target = !profile.vehicle_target;
        let balance_check = match weapon {
            _ if fumble.is_some() => Some((attacker, 0)),
            Some(ArmAttack::Mace) if !hit => Some((attacker, 2)),
            _ if matches!(attack, PhysicalAttack::Kick { .. }) && !hit => Some((attacker, 0)),
            _ if matches!(attack, PhysicalAttack::Kick { .. }) && mech_target => Some((target, 0)),
            _ if attack.is_trip() && hit => Some((target, 0)),
            _ => None,
        };
        let (balance, fall) = if let Some((balancing, modifier)) = balance_check {
            if !hit {
                notices.push(Notice {
                    unit: attacker,
                    text: "You miss and try to remain standing!".into(),
                });
            }
            let mut balance =
                roll_piloting(world, balancing, modifier, rules.fall.extended_piloting)?;
            super::piloting::capture_feedback(
                balancing,
                world.btech.constructed_units()[&balancing].pilot(),
                &balance,
                &mut notices,
                &mut pilot_notices,
            );
            if character {
                experience_messages.extend(super::piloting::award_control_check(
                    world,
                    balancing,
                    &mut balance,
                    rules.fall.extended_piloting,
                )?);
            }
            let fall = if !balance.success
                && !world.btech.constructed_units()[&balancing].is_destroyed()
                && world.btech.constructed_units()[&balancing].posture() != Posture::Prone
            {
                if attack.is_trip() {
                    notices.push(Notice {
                        unit: attacker,
                        text: format!("You trip #{}!", target.0),
                    });
                }
                if !attack.is_trip() || unit_running(world, target) {
                    notices.push(Notice {
                        unit: balancing,
                        text: if attack.is_trip() {
                            "You are tripped and fall to the ground!"
                        } else if hit && attack.hand_weapon().is_some() {
                            "The blow knocks you to the ground!"
                        } else if hit {
                            "The kick knocks you to the ground!"
                        } else {
                            "You lose your balance and fall down!"
                        }
                        .into(),
                    });
                }
                notices.extend(super::broadcast::observer_notices(
                    world,
                    balancing,
                    if attack.is_trip() {
                        "trips up and falls down!"
                    } else {
                        "stumbles and falls down!"
                    },
                ));
                let fall_rules = participant_fall_rules(world, balancing, rules.fall);
                let result =
                    if character && world.objects[&balancing].flags.contains(Flag::InCharacter) {
                        super::fall::resolve_character_fall(world, balancing, 1, fall_rules)?
                    } else {
                        resolve_fall(world, balancing, 1, fall_rules)?
                    };
                result.append_notices(balancing, &mut notices, &mut pilot_notices);
                Some(result)
            } else {
                None
            };
            if attack.is_trip() && fall.is_none() {
                notices.extend(super::broadcast::observer_notices(
                    world,
                    target,
                    "manages to stay upright!",
                ));
            }
            (Some(balance), fall)
        } else {
            (None, None)
        };
        world.btech.validate_action(world)?;
        Ok(PhysicalReport {
            attacker,
            target,
            profile,
            roll,
            hit,
            glancing,
            impact,
            vehicle_impact,
            penetration,
            rotor,
            fumble: fumble_impact,
            spike_backlash,
            experience,
            experience_messages,
            balance,
            fall,
            pilot_notices,
            notices,
        })
    })
}

/// Lance armor-penetration roll and the internal damage it drove through on 10 or more.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LancePenetration {
    pub roll: u8,
    pub impact: Option<TacticalImpact>,
}

/// Lowest 2d6 roll with which a lance drives one point through remaining armor.
const LANCE_PENETRATION_TARGET: u8 = 10;

/// Critical-roll penalty for the point a lance drives through armor.
const LANCE_CRITICAL_PENALTY: u8 = 2;

/// After a lance hit, roll to drive one point into internal structure while armor still covers
/// the struck location; that point's critical roll takes a -2 penalty.
#[allow(clippy::too_many_arguments)]
fn lance_penetration(
    world: &mut World,
    attacker: ObjectId,
    target: ObjectId,
    location: Hit,
    rules: PhysicalRules,
    character: bool,
    notices: &mut Vec<Notice>,
    pilot_notices: &mut Vec<super::PilotNotice>,
) -> Result<Option<LancePenetration>> {
    let victim = &world.btech.constructed_units()[&target];
    let section = location.section;
    let state = &victim.sections()[&section];
    let rear = location.rear_armor
        && matches!(
            section,
            MechSection::LeftTorso | MechSection::RightTorso | MechSection::CenterTorso
        );
    let armor = if rear { state.rear } else { state.armor };
    if victim.is_destroyed() || state.internal == 0 || armor == 0 {
        return Ok(None);
    }
    let roll = world
        .btech
        .constructed
        .get_mut(&target)
        .unwrap()
        .dice
        .generic_roll();
    if roll < LANCE_PENETRATION_TARGET {
        return Ok(Some(LancePenetration { roll, impact: None }));
    }
    notices.push(Notice {
        unit: attacker,
        text: "Your lance punches through the armor!".into(),
    });
    notices.push(Notice {
        unit: target,
        text: "A lance punches through your armor!".into(),
    });
    let fall_rules = participant_fall_rules(world, target, rules.fall);
    let result = super::impact::resolve_penetration_in_candidate(
        world,
        target,
        section,
        1,
        fall_rules,
        super::impact::AttackImpact {
            attacker: Some(attacker),
            weapon_effect: None,
            character,
            followup: false,
        },
        LANCE_CRITICAL_PENALTY,
    )?;
    super::piloting::append_feedback(pilot_notices, result.pilot_notices.clone(), notices.len());
    notices.extend(result.notices.iter().cloned());
    Ok(Some(LancePenetration {
        roll,
        impact: Some(result),
    }))
}

impl Mech {
    /// Physical recovery timers keyed by the limb that performed an attack.
    pub fn limb_recycle(&self) -> &std::collections::BTreeMap<MechSection, u16> {
        &self.limb_recycle
    }

    /// Tie up every surviving section for one turn. After a charge, a death from above attack
    /// or picking up a club, a 'Mech may neither fire a weapon nor make another physical attack
    /// that turn (Total Warfare pp. 146, 148-149).
    pub(super) fn tie_up_every_section(&mut self) {
        for section in MechSection::ALL {
            if self.sections[&section].internal > 0 {
                self.limb_recycle.insert(section, PHYSICAL_RECOVERY_SECONDS);
            }
        }
    }
}

/// Arms attempted in a physical command, always left before right when both are selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmSelection {
    Left,
    Right,
    Both,
}

/// An arm that could not attack, without discarding another arm's completed punch.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArmRejection {
    pub arm: Arm,
    pub reason: String,
}

/// Ordered arm attacks and per-arm failures, committed as one action with staged notices.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArmAttackReport {
    pub attacks: Vec<PhysicalReport>,
    pub rejections: Vec<ArmRejection>,
    /// Private feedback with positions in the combined arm sequence.
    pub pilot_notices: Vec<super::PilotNotice>,
    pub notices: Vec<Notice>,
}

/// Resolve a punch action using the shared arm-attack sequencer.
pub fn resolve_punch(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    arms: ArmSelection,
    rules: PhysicalRules,
) -> Result<ArmAttackReport> {
    resolve_arm_attack(
        world,
        attacker,
        pilot,
        target,
        arms,
        ArmAttack::Punch,
        rules,
    )
}

/// Attack each selected arm makes in one sequenced arm action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmWeapon {
    /// Every selected arm makes this attack.
    Fixed(ArmAttack),
    /// Each selected arm swings whichever hand weapon it carries.
    Installed,
}

impl From<ArmAttack> for ArmWeapon {
    fn from(kind: ArmAttack) -> Self {
        Self::Fixed(kind)
    }
}

/// Arm selection and physical weapon used by one sequenced host action.
#[derive(Debug, Clone, Copy)]
pub struct ArmAttackChoice {
    pub arms: ArmSelection,
    pub kind: ArmWeapon,
}

/// Attempt the selected arms in order; an impact error rolls back the entire action.
/// Expected per-arm targeting failures do not undo a successful attack with the other arm.
/// Punches may use both arms; a completed axe/sword swing blocks another arm through recovery.
pub fn resolve_arm_attack(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    arms: ArmSelection,
    kind: ArmAttack,
    rules: PhysicalRules,
) -> Result<ArmAttackReport> {
    resolve_arm_attack_inner(
        world,
        attacker,
        pilot,
        [target; 2],
        ArmAttackChoice {
            arms,
            kind: kind.into(),
        },
        rules,
        false,
    )
}

/// Punch two different targets, the left arm at `left` and the right at `right`; each punch is
/// rolled on its own (Total Warfare p. 145).
pub fn resolve_split_punch(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    [left, right]: [ObjectId; 2],
    rules: PhysicalRules,
) -> Result<ArmAttackReport> {
    resolve_arm_attack_inner(
        world,
        attacker,
        pilot,
        [left, right],
        ArmAttackChoice {
            arms: ArmSelection::Both,
            kind: ArmAttack::Punch.into(),
        },
        rules,
        false,
    )
}

/// Sequence selected arms inside a host checkpoint that can publish character casualties.
pub(super) fn resolve_arm_attack_in_action(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    choice: ArmAttackChoice,
    rules: PhysicalRules,
) -> Result<ArmAttackReport> {
    resolve_arm_attack_inner(world, attacker, pilot, [target; 2], choice, rules, true)
}

/// Punch two different targets inside a host checkpoint that publishes character casualties.
pub(super) fn resolve_split_punch_in_action(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    targets: [ObjectId; 2],
    rules: PhysicalRules,
) -> Result<ArmAttackReport> {
    let choice = ArmAttackChoice {
        arms: ArmSelection::Both,
        kind: ArmAttack::Punch.into(),
    };
    resolve_arm_attack_inner(world, attacker, pilot, targets, choice, rules, true)
}

/// Shared availability, rejection and ordered hit processing for both publication modes.
/// `targets` holds the left arm's target, then the right arm's.
fn resolve_arm_attack_inner(
    world: &mut World,
    attacker: ObjectId,
    pilot: ObjectId,
    targets: [ObjectId; 2],
    choice: ArmAttackChoice,
    rules: PhysicalRules,
    character: bool,
) -> Result<ArmAttackReport> {
    let ArmAttackChoice { arms, kind } = choice;
    super::power::controlled_unit(world, attacker, pilot)?;
    ensure!(
        world.btech.constructed_units()[&attacker]
            .limb_recycle
            .is_empty(),
        "Your limbs are still recovering from another attack"
    );
    let auto_select = arms == ArmSelection::Both;
    let arms: &[Arm] = match arms {
        ArmSelection::Left => &[Arm::Left],
        ArmSelection::Right => &[Arm::Right],
        ArmSelection::Both => &[Arm::Left, Arm::Right],
    };
    world.attempt(|world| {
        let mut report = ArmAttackReport {
            attacks: Vec::new(),
            rejections: Vec::new(),
            pilot_notices: Vec::new(),
            notices: Vec::new(),
        };
        let mut completed_arm = None;
        for &arm in arms {
            let target = match arm {
                Arm::Left => targets[0],
                Arm::Right => targets[1],
            };
            let unit = &world.btech.constructed_units()[&attacker];
            let kind = match kind {
                ArmWeapon::Fixed(kind) => kind,
                ArmWeapon::Installed => match ArmAttack::installed(unit, arm.section())? {
                    Some(kind) => kind,
                    None if auto_select => continue,
                    None => {
                        let reason = format!(
                            "{}: No physical weapon installed in this arm",
                            arm.section().name().replace('_', " ")
                        );
                        report.notices.push(Notice {
                            unit: attacker,
                            text: reason.clone(),
                        });
                        report.rejections.push(ArmRejection { arm, reason });
                        continue;
                    }
                },
            };
            if auto_select && !kind.available(unit, arm.section())? {
                continue;
            }
            // Only paired attacks may follow a completed arm; other swings block the second arm.
            let allowed_arm = completed_arm.filter(|_| kind.pairs());
            let attack = kind.attack(arm);
            if let Err(error) = attack_profile_inner(
                world,
                attacker,
                pilot,
                target,
                attack,
                rules,
                AttackContext {
                    completed_arm: allowed_arm,
                    character,
                },
            ) {
                let reason = format!("{}: {error:#}", arm.section().name().replace('_', " "));
                report.notices.push(Notice {
                    unit: attacker,
                    text: reason.clone(),
                });
                report.rejections.push(ArmRejection { arm, reason });
                continue;
            }
            let result = resolve_attack_inner(
                world,
                attacker,
                pilot,
                target,
                attack,
                rules,
                AttackContext {
                    completed_arm: allowed_arm,
                    character,
                },
            )?;
            if kind.pairs() {
                completed_arm = Some(arm.section());
            }
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                result.pilot_notices.clone(),
                report.notices.len(),
            );
            report.notices.extend(result.notices.iter().cloned());
            report.attacks.push(result);
        }
        if report.attacks.is_empty() {
            let reasons = report
                .rejections
                .iter()
                .map(|r| r.reason.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if reasons.is_empty() {
                match kind {
                    ArmWeapon::Fixed(kind) => {
                        anyhow::bail!("No usable {kind:?} in the selected arms")
                    }
                    ArmWeapon::Installed => {
                        anyhow::bail!("No usable physical weapon in the selected arms")
                    }
                }
            }
            anyhow::bail!("No selected arm could attack: {reasons}");
        }
        world.btech.validate_action(world)?;
        Ok(report)
    })
}

/// Parse one or both arms, defaulting to both.
fn selected_arms(value: Option<&str>) -> Result<ArmSelection> {
    match value.map(str::to_ascii_lowercase).as_deref() {
        None | Some("b" | "both") => Ok(ArmSelection::Both),
        Some("l" | "left") => Ok(ArmSelection::Left),
        Some("r" | "right") => Ok(ArmSelection::Right),
        _ => anyhow::bail!("Choose left, right, or both arms"),
    }
}

/// Shared native/Lua arm-attack selection with acquired-contact checks for an explicit target.
/// A `second` target splits a two-armed punch: the left arm punches `target` and the right arm
/// `second` (Total Warfare p. 145).
#[allow(clippy::too_many_arguments)]
pub(crate) fn configured_arm_attack(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    arms: Option<&str>,
    target: Option<ObjectId>,
    second: Option<ObjectId>,
    kind: ArmWeapon,
) -> Result<ArmAttackReport> {
    if let Some(second) = second {
        let targets = {
            let world = scripts.world.borrow();
            super::power::controlled_unit(&world, id, pilot)?;
            ensure!(
                kind == ArmWeapon::Fixed(ArmAttack::Punch),
                "Only punches can strike two targets."
            );
            ensure!(
                selected_arms(arms)? == ArmSelection::Both,
                "Punching two targets takes both arms."
            );
            let first = target.context("Name the left arm's target first.")?;
            [
                selected_target(&world, id, Some(first))?,
                selected_target(&world, id, Some(second))?,
            ]
        };
        return super::split_punch_action(
            scripts,
            config,
            id,
            pilot,
            targets,
            configured_rules(config),
        );
    }
    let (chosen, arms) = {
        let world = scripts.world.borrow();
        super::power::controlled_unit(&world, id, pilot)?;
        let arms = selected_arms(arms)?;
        (selected_target(&world, id, target)?, arms)
    };
    let choice = ArmAttackChoice { arms, kind };
    super::arm_attack_action(
        scripts,
        config,
        id,
        pilot,
        chosen,
        choice,
        configured_rules(config),
    )
}

/// Resolve the selected contact without acquiring one or advancing a lock timer.
pub(super) fn selected_target(
    world: &World,
    id: ObjectId,
    target: Option<ObjectId>,
) -> Result<ObjectId> {
    let chosen = target
        .or_else(|| {
            world.btech.constructed_units()[&id]
                .target_lock()
                .map(|lock| lock.target)
        })
        .context("You do not have a target set!")?;
    if target.is_some() {
        ensure!(
            visible_contact(world, id, chosen)?.is_some(),
            "Target is not in line of sight!"
        );
    }
    Ok(chosen)
}

/// Parse one leg; default selection is the right leg.
fn selected_leg(value: Option<&str>) -> Result<Leg> {
    match value.map(str::to_ascii_lowercase).as_deref() {
        None | Some("r" | "right") => Ok(Leg::Right),
        Some("l" | "left") => Ok(Leg::Left),
        _ => anyhow::bail!("Choose a single left or right leg"),
    }
}

/// Build one kick policy from the saved game settings.
pub(crate) fn configured_rules(config: &crate::Config) -> PhysicalRules {
    let settings = &config.battletech;
    PhysicalRules {
        fall: FallRules {
            vehicle_impact: crate::VehicleImpactRules::configured(settings, false),
            extended_piloting: settings.extended_piloting != 0,
            toughness: false,
        },
    }
}

/// Shared native/Lua target selection; explicit ids must identify a currently acquired contact.
pub(crate) fn configured_kick(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    leg: Option<&str>,
    target: Option<ObjectId>,
) -> Result<PhysicalReport> {
    let (chosen, leg) = {
        let world = scripts.world.borrow();
        super::power::controlled_unit(&world, id, pilot)?;
        let leg = selected_leg(leg)?;
        (selected_target(&world, id, target)?, leg)
    };
    let attack = PhysicalAttack::Kick { leg };
    super::physical_attack_action(
        scripts,
        config,
        id,
        pilot,
        chosen,
        attack,
        configured_rules(config),
    )
}

/// Shared native/Lua trip selection with acquired-contact checks for an explicit target.
pub(crate) fn configured_trip(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    leg: Option<&str>,
    target: Option<ObjectId>,
) -> Result<PhysicalReport> {
    let (chosen, leg) = {
        let world = scripts.world.borrow();
        super::power::controlled_unit(&world, id, pilot)?;
        let leg = selected_leg(leg)?;
        (selected_target(&world, id, target)?, leg)
    };
    let attack = PhysicalAttack::Trip { leg };
    super::physical_attack_action(
        scripts,
        config,
        id,
        pilot,
        chosen,
        attack,
        configured_rules(config),
    )
}

/// Physical native command routing, independent of selected arm or leg.
#[derive(Clone, Copy)]
enum PhysicalCommand {
    Kick,
    Punch,
    Trip,
    Melee,
    Club,
    GrabClub,
    Charge,
}

/// Native adapter swinging each selected arm's installed axe, sword, mace, saw or claw.
pub(crate) fn melee_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Melee)
}

/// Native trip adapter.
pub(crate) fn trip_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Trip)
}

/// Native kick adapter.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Kick)
}

/// Native punch adapter.
pub(crate) fn punch_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Punch)
}

/// A physical command owns both its world checkpoint and staged cockpit messages.
fn command_for(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
    kind: PhysicalCommand,
) -> Result<crate::CommandAction> {
    let usage = match kind {
        PhysicalCommand::Kick => "Usage: kick [left|right] [#unit]",
        PhysicalCommand::Trip => "Usage: trip [left|right] [#unit]",
        PhysicalCommand::Punch => "Usage: punch [left|right|both] [#unit [#unit]]",
        PhysicalCommand::Melee => "Usage: melee [left|right|both] [#unit]",
        PhysicalCommand::Club => "Usage: club [#unit]",
        PhysicalCommand::GrabClub => "Usage: grabclub [left|right|-]",
        PhysicalCommand::Charge => "Usage: charge [#unit|-]",
    };
    let result = ctx.scripts.atomic(|_| {
        let mut args = input.args.split_whitespace().peekable();
        let leg = if args.peek().is_some_and(|arg| !arg.starts_with('#')) {
            args.next()
        } else {
            None
        };
        let parse_target = |arg: &str| -> Result<ObjectId> {
            Ok(ObjectId(
                arg.strip_prefix('#')
                    .context(usage)?
                    .parse()
                    .context("Invalid target number")?,
            ))
        };
        let target = args.next().map(parse_target).transpose()?;
        // A punch may name a second target for the right arm (Total Warfare p. 145).
        let second = if matches!(kind, PhysicalCommand::Punch) {
            args.next().map(parse_target).transpose()?
        } else {
            None
        };
        ensure!(args.next().is_none(), "{usage}");
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let notices = match kind {
            PhysicalCommand::Charge => {
                let selection = match (leg, target) {
                    (None, None) => ChargeSelection::Default,
                    (None, Some(target)) => ChargeSelection::Target(target),
                    (Some("-"), None) => ChargeSelection::Cancel,
                    _ => anyhow::bail!("{usage}"),
                };
                select_charge(
                    &mut ctx.scripts.world.borrow_mut(),
                    id,
                    ctx.player,
                    selection,
                )?
            }
            PhysicalCommand::Club => {
                ensure!(leg.is_none(), "{usage}");
                configured_club(ctx.scripts, ctx.config, id, ctx.player, target)?;
                Vec::new()
            }
            PhysicalCommand::GrabClub => {
                ensure!(target.is_none(), "{usage}");
                super::club::grab_club(&mut ctx.scripts.world.borrow_mut(), id, ctx.player, leg)?
            }
            PhysicalCommand::Kick => {
                configured_kick(ctx.scripts, ctx.config, id, ctx.player, leg, target)?;
                Vec::new()
            }
            PhysicalCommand::Trip => {
                configured_trip(ctx.scripts, ctx.config, id, ctx.player, leg, target)?;
                Vec::new()
            }
            PhysicalCommand::Punch | PhysicalCommand::Melee => {
                let attack = match kind {
                    PhysicalCommand::Melee => ArmWeapon::Installed,
                    _ => ArmWeapon::Fixed(ArmAttack::Punch),
                };
                configured_arm_attack(
                    ctx.scripts,
                    ctx.config,
                    id,
                    ctx.player,
                    leg,
                    target,
                    second,
                    attack,
                )?;
                Vec::new()
            }
        };
        for notice in notices {
            super::notify_unit(ctx.scripts, notice)?;
        }
        Ok(())
    });
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Resolve a two-handed club swing with configured targeting and combat policy.
pub(crate) fn configured_club(
    scripts: &crate::Scripts,
    config: &crate::Config,
    id: ObjectId,
    pilot: ObjectId,
    target: Option<ObjectId>,
) -> Result<PhysicalReport> {
    let chosen = {
        let world = scripts.world.borrow();
        super::power::controlled_unit(&world, id, pilot)?;
        selected_target(&world, id, target)?
    };
    super::physical_attack_action(
        scripts,
        config,
        id,
        pilot,
        chosen,
        PhysicalAttack::Club,
        configured_rules(config),
    )
}

/// Inspect a two-handed club swing without changing state.
pub fn club_profile(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
) -> Result<PhysicalProfile> {
    attack_profile(world, id, pilot, target, PhysicalAttack::Club, rules, None)
}

/// Commit one two-handed club swing and its damage, recovery and breakage.
pub fn resolve_club(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    target: ObjectId,
    rules: PhysicalRules,
) -> Result<PhysicalReport> {
    resolve_attack(world, id, pilot, target, PhysicalAttack::Club, rules, None)
}

/// Native two-handed club adapter.
pub(crate) fn club_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Club)
}

/// Native carried-tree adapter.
pub(crate) fn grabclub_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::GrabClub)
}

/// Native charge selection adapter.
pub(crate) fn charge_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, input, PhysicalCommand::Charge)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A physical attack ties up its attacking limbs for exactly one turn.
    #[test]
    fn physical_recovery_is_one_turn() {
        assert_eq!(
            i64::from(PHYSICAL_RECOVERY_SECONDS),
            super::super::spotter::TURN_SECONDS
        );
    }

    /// Each attack names the limbs whose weapons it ties up and which must not have fired
    /// (Total Warfare pp. 145-147).
    #[test]
    fn attacks_tie_up_their_own_limbs() {
        use MechSection::*;
        let biped = MechChassis::Biped;
        let quad = MechChassis::Quad;
        let punch = PhysicalAttack::Punch { arm: Arm::Left };
        assert_eq!(punch.limbs(biped, false), [LeftArm]);
        let swing = PhysicalAttack::Weapon {
            arm: Arm::Right,
            weapon: ArmAttack::Sword,
        };
        assert_eq!(swing.limbs(biped, false), [RightArm]);
        assert_eq!(
            PhysicalAttack::Club.limbs(biped, false),
            [LeftArm, RightArm]
        );
        let kick = PhysicalAttack::Kick { leg: Leg::Right };
        assert_eq!(kick.limbs(biped, false), [RightLeg]);
        assert_eq!(kick.limbs(quad, false), [RightArm]);
        assert_eq!(kick.limbs(quad, true), [LeftLeg, RightLeg]);
        let trip = PhysicalAttack::Trip { leg: Leg::Left };
        assert_eq!(trip.limbs(biped, false), [LeftLeg]);
    }

    /// A quad kicking into its rear arc kicks with the rear leg on the chosen side; every
    /// other attack uses its selected limb (Total Warfare p. 147).
    #[test]
    fn rear_kicks_use_the_rear_leg() {
        use MechSection::*;
        let quad = MechChassis::Quad;
        let biped = MechChassis::Biped;
        let left = PhysicalAttack::Kick { leg: Leg::Left };
        let right = PhysicalAttack::Kick { leg: Leg::Right };
        assert_eq!(left.limb(quad, false), LeftArm);
        assert_eq!(left.limb(quad, true), LeftLeg);
        assert_eq!(right.limb(quad, false), RightArm);
        assert_eq!(right.limb(quad, true), RightLeg);
        assert_eq!(right.limb(biped, false), RightLeg);
        let punch = PhysicalAttack::Punch { arm: Arm::Left };
        assert_eq!(punch.limb(biped, true), LeftArm);
    }

    /// The level-difference rules for vehicles: a ground vehicle or landed VTOL is kicked,
    /// clubbed or struck on the 'Mech's own level and punched, clubbed or struck one level up
    /// (Total Warfare pp. 147, 150); a flying VTOL follows the VTOL table (p. 198).
    #[test]
    fn vehicle_reach_follows_the_level_tables() {
        let kick = PhysicalAttack::Kick { leg: Leg::Right };
        let punch = PhysicalAttack::Punch { arm: Arm::Right };
        let club = PhysicalAttack::Club;
        let sword = PhysicalAttack::Weapon {
            arm: Arm::Right,
            weapon: ArmAttack::Sword,
        };
        let reach = |attack, difference, flying| vehicle_reach(attack, difference, flying).is_ok();
        // Ground vehicles and landed VTOLs.
        for (difference, kicks, punches, swings) in [
            (-1, false, false, false),
            (0, true, false, true),
            (1, false, true, true),
            (2, false, false, false),
        ] {
            assert_eq!(reach(kick, difference, false), kicks, "{difference}");
            assert_eq!(reach(punch, difference, false), punches, "{difference}");
            assert_eq!(reach(club, difference, false), swings, "{difference}");
            assert_eq!(reach(sword, difference, false), swings, "{difference}");
        }
        // Flying VTOLs.
        for (difference, kicks, punches, swings) in [
            (-1, false, false, false),
            (0, true, false, true),
            (1, false, true, true),
            (2, false, true, true),
            (3, false, false, true),
            (4, false, false, false),
        ] {
            assert_eq!(reach(kick, difference, true), kicks, "{difference}");
            assert_eq!(reach(punch, difference, true), punches, "{difference}");
            assert_eq!(reach(club, difference, true), swings, "{difference}");
            assert_eq!(reach(sword, difference, true), swings, "{difference}");
        }
    }

    /// A lance deals one point per five tons, rounded up (Tactical Operations: Advanced Units
    /// & Equipment p. 101); a hatchet one point per five tons.
    #[test]
    fn lance_damage_rounds_up() {
        assert_eq!(ArmAttack::Lance.damage(55), 11);
        assert_eq!(ArmAttack::Lance.damage(57), 12);
        assert_eq!(ArmAttack::Axe.damage(57), 11);
    }

    /// Only the wrecking ball is limited to the forward arc (Total Warfare p. 146).
    #[test]
    fn only_the_wrecking_ball_swings_forward_only() {
        for weapon in ArmAttack::HAND_WEAPONS {
            assert_eq!(
                weapon.forward_only(),
                weapon == ArmAttack::WreckingBall,
                "{weapon:?}"
            );
        }
    }

    /// A charge, a death from above attack or a club pickup ties up every surviving section.
    #[test]
    fn every_surviving_section_is_tied_up() {
        let mut unit = Mech::from_template(
            MechTemplate::parse(
                "JR7-D",
                include_str!("../../tests/fixtures/btech/units/JR7-D.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        unit.sections
            .get_mut(&MechSection::LeftArm)
            .unwrap()
            .internal = 0;
        unit.tie_up_every_section();
        for section in MechSection::ALL {
            let expected = (section != MechSection::LeftArm).then_some(PHYSICAL_RECOVERY_SECONDS);
            assert_eq!(
                unit.limb_recycle.get(&section).copied(),
                expected,
                "{section:?}"
            );
        }
    }
}
