//! Atomic conventional hit resolution, including immediate critical and ammunition damage cascades.
use super::{
    BattleCriticalLoss, BattleDamagePhase, BattleDamageResult, BattleHit, BattleSection,
    BattleUnit, CriticalLocation,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Effects the enclosing combat action must handle alongside material/equipment damage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum BattleImpactEffect {
    HeadInjury,
    ExplosionInjury,
    /// An explosion vented by CASE II injures the pilot once instead of twice.
    VentedExplosionInjury,
    CrewStun,
    SectionLost(BattleSection),
}

/// Weapon-specific effects carried only by the original damage path, never nested explosions.
#[derive(Clone, Copy)]
pub(super) enum WeaponEffect {
    Conventional,
    ArmorPiercing(super::BattleWeapon),
    Plasma,
}

/// Ordered material phases and critical losses from one hit, including nested explosions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BattleImpactReport {
    /// Section-loss reactor blasts retain secondary consequences through grouped damage reports.
    pub reactor_explosions: Vec<super::BattleReactorExplosion>,
    /// Newly breached sections, including any immediate secondary falls or reactor blast.
    pub exposures: Vec<super::BattleSectionExposureReport>,
    pub phases: Vec<BattleDamageResult>,
    pub criticals: Vec<(CriticalLocation, BattleCriticalLoss)>,
    pub pending_effects: Vec<BattleImpactEffect>,
    /// Character injuries applied at their event positions in an in-character action.
    pub character_injuries: Vec<super::BattleCharacterPilotInjury>,
    pub destroyed: bool,
    /// Plasma heat rolls after the primary damage path, in transfer-unwind order.
    pub plasma_heat: Vec<u8>,
    /// Dumped salvo ignition, distinct from an internal ammunition-bin explosion.
    pub dump_ignitions: Vec<super::BattleDumpIgnition>,
    /// This hit destroyed the unit's searchlight.
    pub searchlight_destroyed: bool,
}

/// Material damage that kills conventional biped crew, distinct from ordinary unit destruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleCrewCasualty {
    HeadDestroyed,
    CockpitDestroyed,
    CharacterInjury,
    VacuumExposure,
}

impl BattleImpactReport {
    /// Identify lethal crew damage in this cascade; the adapter still owns IC policy and evacuation.
    /// Head destruction takes precedence when the same cascade also loses cockpit equipment.
    pub fn crew_casualty(&self) -> Option<BattleCrewCasualty> {
        if self
            .phases
            .iter()
            .any(|phase| phase.destroyed_sections.contains(&BattleSection::Head))
        {
            return Some(BattleCrewCasualty::HeadDestroyed);
        }
        if self.criticals.iter().any(|(_, loss)| {
            matches!(
                loss,
                BattleCriticalLoss::System {
                    system: super::BattleSystem::Cockpit
                }
            )
        }) {
            return Some(BattleCrewCasualty::CockpitDestroyed);
        }
        if self
            .character_injuries
            .iter()
            .any(|report| report.injury.fatal)
        {
            return Some(BattleCrewCasualty::CharacterInjury);
        }
        if self
            .exposures
            .iter()
            .any(|report| report.section == BattleSection::Head)
        {
            return Some(BattleCrewCasualty::VacuumExposure);
        }
        None
    }
}

/// Resolve a known hit in a private unit candidate, publishing only after the whole cascade validates.
/// This does not establish firing permission, consume weapon ammunition or apply pending crew/fall effects.
pub fn resolve_impact(
    world: &mut World,
    id: ObjectId,
    hit: BattleHit,
    damage: u16,
) -> Result<BattleImpactReport> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    world.attempt(|world| {
        let report = resolve_in_candidate(world, id, hit, damage, None, None)?;
        Ok(report.impact)
    })
}

/// Resolve character injuries at their damage events; the host action owns evacuation and rollback.
pub(super) fn resolve_character_impact(
    world: &mut World,
    id: ObjectId,
    hit: BattleHit,
    damage: u16,
) -> Result<super::BattleTacticalImpact> {
    resolve_character_impact_with_rules(world, id, hit, damage, None)
}

/// Character damage with optional fall and flooding policy supplied by an enclosing action.
pub(super) fn resolve_character_impact_with_rules(
    world: &mut World,
    id: ObjectId,
    hit: BattleHit,
    damage: u16,
    rules: Option<super::BattleFallRules>,
) -> Result<super::BattleTacticalImpact> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| object.flags.contains(Flag::InCharacter)
                && !object.flags.contains(Flag::Going)),
        "Character impact requires a live in-character unit"
    );
    let toughness = rules.is_some_and(|rules| rules.toughness)
        || world
            .btech
            .constructed_units()
            .get(&id)
            .and_then(|unit| unit.pilot())
            .and_then(|pilot| world.btech.character_values().get(&pilot))
            .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    let mut context = ImpactContext::new(world, id, rules)?;
    context.character_effects = true;
    context.character_toughness = Some(toughness);
    resolve_packet(context, hit, damage, None)
}

/// Wizard damage attributes the hit to the damaged unit while retaining ordinary injury policy.
pub(super) fn resolve_scenario_impact(
    world: &mut World,
    id: ObjectId,
    hit: BattleHit,
    damage: u16,
) -> Result<super::BattleTacticalImpact> {
    let character = world
        .objects
        .get(&id)
        .is_some_and(|object| object.flags.contains(Flag::InCharacter));
    resolve_attack_in_candidate(
        world,
        id,
        hit,
        damage,
        None,
        AttackImpact {
            attacker: Some(id),
            weapon_effect: None,
            character,
            followup: false,
        },
    )
}

/// Detonate one available nonempty bin, including internal transfer, nested explosions and crew effects.
/// Heat checks and other callers own trigger probability and publication of the returned notices.
pub fn explode_ammunition(
    world: &mut World,
    id: ObjectId,
    index: usize,
    rules: super::BattleFallRules,
) -> Result<super::BattleTacticalImpact> {
    explode_ammunition_inner(world, id, index, rules, false)
}

/// Detonate a bin inside an action that publishes character health and crew casualties.
pub(super) fn explode_ammunition_in_action(
    world: &mut World,
    id: ObjectId,
    index: usize,
    rules: super::BattleFallRules,
) -> Result<super::BattleTacticalImpact> {
    explode_ammunition_inner(world, id, index, rules, true)
}

/// Shared bin validation and ordered explosion traversal for either publication mode.
fn explode_ammunition_inner(
    world: &mut World,
    id: ObjectId,
    index: usize,
    rules: super::BattleFallRules,
    character: bool,
) -> Result<super::BattleTacticalImpact> {
    let object = world.objects.get(&id).context("Unit is unavailable")?;
    ensure!(
        !object.flags.contains(Flag::Going)
            && (character || !object.flags.contains(Flag::InCharacter)),
        "Ammunition explosion requires a live tactical unit"
    );
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    unit.validate()?;
    let hazard = unit.ammunition_hazard(index)?;
    ensure!(
        hazard.damage > 0 && !unit.critical_unavailable(hazard.location),
        "Ammunition bin is empty or unavailable"
    );
    let character_effects = character;
    let character = character && object.flags.contains(Flag::InCharacter);
    world.attempt(|world| {
        let mut context = ImpactContext::new(world, id, Some(rules))?;
        context.character_effects = character_effects;
        context.character_toughness = character.then_some(rules.toughness);
        context.lose_critical(hazard.location)?;
        context.unit().validate()?;
        context.report.impact.destroyed = context.unit().is_destroyed();
        let report = context.report;
        Ok(report)
    })
}

/// Shared ordered damage traversal; the owner must discard the candidate on any error.
pub(super) fn resolve_in_candidate(
    world: &mut World,
    id: ObjectId,
    hit: BattleHit,
    damage: u16,
    rules: Option<super::BattleFallRules>,
    weapon_effect: Option<WeaponEffect>,
) -> Result<super::BattleTacticalImpact> {
    let context = ImpactContext::new(world, id, rules)?;
    resolve_packet(context, hit, damage, weapon_effect)
}

/// Attribution, casualty policy and weapon effects supplied by an admitted attack.
pub(super) struct AttackImpact {
    pub attacker: Option<ObjectId>,
    pub weapon_effect: Option<WeaponEffect>,
    /// Host capability for primary and secondary character casualties.
    pub character: bool,
    pub followup: bool,
}

/// Reuse the same damage cascade while retaining attack attribution and weapon-specific effects.
/// Omitting battlefield rules permits unplaced scenario units to receive material and crew damage.
pub(super) fn resolve_attack_in_candidate(
    world: &mut World,
    id: ObjectId,
    hit: BattleHit,
    damage: u16,
    rules: impl Into<Option<super::BattleFallRules>>,
    attack: AttackImpact,
) -> Result<super::BattleTacticalImpact> {
    let context = attack_context(world, id, rules.into(), &attack)?;
    resolve_packet(context, hit, damage, attack.weapon_effect)
}

/// Build the damage context for an attributed attack packet.
fn attack_context<'a>(
    world: &'a mut World,
    id: ObjectId,
    rules: Option<super::BattleFallRules>,
    attack: &AttackImpact,
) -> Result<ImpactContext<'a>> {
    let toughness = rules.map(|rules| rules.toughness).unwrap_or_else(|| {
        world
            .btech
            .constructed_units()
            .get(&id)
            .and_then(|unit| unit.pilot())
            .and_then(|pilot| world.btech.character_values().get(&pilot))
            .is_some_and(|values| super::advantages::enabled(values, "Toughness"))
    });
    let mut context = if attack.followup {
        ImpactContext::continuation(world, id, rules)?
    } else {
        ImpactContext::new(world, id, rules)?
    };
    context.attacker = attack.attacker;
    context.character_effects = attack.character;
    context.character_toughness = (attack.character
        && context.world.objects[&id].flags.contains(Flag::InCharacter))
    .then_some(toughness);
    Ok(context)
}

/// Drive `damage` straight into a section's internal structure, bypassing its armor.
/// Critical-hit rolls for the packet take `critical_penalty`; used by lance penetration.
pub(super) fn resolve_penetration_in_candidate(
    world: &mut World,
    id: ObjectId,
    section: BattleSection,
    damage: u16,
    rules: super::BattleFallRules,
    attack: AttackImpact,
    critical_penalty: u8,
) -> Result<super::BattleTacticalImpact> {
    let mut context = attack_context(world, id, Some(rules), &attack)?;
    if context.enter_damage() {
        context.report.impact.destroyed = context.unit().is_destroyed();
        return Ok(context.report);
    }
    context.warn_attacker();
    resolve_path(
        &mut context,
        DamagePacket {
            announced: true,
            direct_hit: false,
            section,
            damage,
            internal_only: true,
            transfer: true,
            rear: false,
            tac: false,
            weapon_effect: None,
            critical_penalty,
        },
    )?;
    context.unit().validate()?;
    context.report.impact.destroyed = context.unit().is_destroyed();
    Ok(context.report)
}

/// Traverse one packet with the admission policy already selected by its attack owner.
fn resolve_packet(
    mut context: ImpactContext<'_>,
    hit: BattleHit,
    damage: u16,
    weapon_effect: Option<WeaponEffect>,
) -> Result<super::BattleTacticalImpact> {
    if context.enter_damage() {
        context.report.impact.destroyed = context.unit().is_destroyed();
        return Ok(context.report);
    }
    context.warn_attacker();
    if damage > 0 {
        // Hit-table stun precedes material interception, like vehicle routing effects.
        if hit.crew_stun {
            context.effect(BattleImpactEffect::CrewStun)?;
        }
        resolve_path(
            &mut context,
            DamagePacket {
                announced: true,
                direct_hit: true,
                section: hit.section,
                damage,
                internal_only: false,
                transfer: true,
                rear: hit.rear_armor,
                tac: hit.through_armor_critical,
                weapon_effect,
                critical_penalty: 0,
            },
        )?;
    }
    context.unit().validate()?;
    context.report.impact.destroyed = context.unit().is_destroyed();
    Ok(context.report)
}

/// Internal environmental damage uses the same transfer, critical, CASE and injury cascade as other impacts.
pub(super) fn resolve_internal_stress(
    world: &mut World,
    id: ObjectId,
    section: BattleSection,
    damage: u16,
    rules: super::BattleFallRules,
) -> Result<super::BattleTacticalImpact> {
    let character = world.objects[&id].flags.contains(Flag::InCharacter);
    let mut context = ImpactContext::new(world, id, Some(rules))?;
    context.character_effects = true;
    context.character_toughness = character.then_some(rules.toughness);
    context.attacker = Some(id);
    resolve_path(
        &mut context,
        DamagePacket {
            announced: false,
            direct_hit: false,
            section,
            damage,
            internal_only: true,
            transfer: true,
            rear: false,
            tac: false,
            weapon_effect: None,
            critical_penalty: 0,
        },
    )?;
    context.unit().validate()?;
    context.report.impact.destroyed = context.unit().is_destroyed();
    Ok(context.report)
}

/// Resolve a misload or propellant ignition inside its mounting section; nested explosions retain their own rules.
/// The shot transaction has already disabled the weapon and owns commit or rollback.
pub(super) fn resolve_misload_in_candidate(
    world: &mut World,
    id: ObjectId,
    section: BattleSection,
    damage: u16,
    rules: super::BattleFallRules,
) -> Result<super::BattleTacticalImpact> {
    resolve_misload_inner(world, id, section, damage, rules, false)
}

/// Resolve an internal misload with character injury capability.
pub(super) fn resolve_character_misload_in_candidate(
    world: &mut World,
    id: ObjectId,
    section: BattleSection,
    damage: u16,
    rules: super::BattleFallRules,
) -> Result<super::BattleTacticalImpact> {
    resolve_misload_inner(world, id, section, damage, rules, true)
}

/// Shared internal non-transferring misload traversal.
fn resolve_misload_inner(
    world: &mut World,
    id: ObjectId,
    section: BattleSection,
    damage: u16,
    rules: super::BattleFallRules,
    character: bool,
) -> Result<super::BattleTacticalImpact> {
    let mut context = ImpactContext::new(world, id, Some(rules))?;
    context.character_effects = character;
    context.character_toughness = character.then_some(rules.toughness);
    resolve_path(
        &mut context,
        DamagePacket {
            announced: false,
            direct_hit: false,
            section,
            damage,
            internal_only: true,
            transfer: false,
            rear: false,
            tac: false,
            weapon_effect: None,
            critical_penalty: 0,
        },
    )?;
    context.unit().validate()?;
    context.report.impact.destroyed = context.unit().is_destroyed();
    Ok(context.report)
}

/// Own the current damage cascade while immediate crew and balance effects mutate the same world.
struct ImpactContext<'a> {
    attacker: Option<ObjectId>,
    world: &'a mut World,
    id: ObjectId,
    rules: Option<super::BattleFallRules>,
    work: u16,
    character_toughness: Option<bool>,
    /// The enclosing host can publish injuries and evacuate nested collision casualties.
    character_effects: bool,
    report: super::BattleTacticalImpact,
}

impl<'a> ImpactContext<'a> {
    /// Start an ordered cascade inside a world whose transaction is owned by the caller.
    fn new(
        world: &'a mut World,
        id: ObjectId,
        rules: Option<super::BattleFallRules>,
    ) -> Result<Self> {
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit construction state is unavailable")?;
        unit.validate()?;
        ensure!(!unit.is_destroyed(), "Unit is already destroyed");
        Self::continuation(world, id, rules)
    }

    /// Preserve packet traversal after destruction within an accepted multi-packet attack.
    fn continuation(
        world: &'a mut World,
        id: ObjectId,
        rules: Option<super::BattleFallRules>,
    ) -> Result<Self> {
        world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit construction state is unavailable")?
            .validate()?;
        Ok(Self {
            attacker: None,
            world,
            id,
            rules,
            work: 4096,
            character_toughness: None,
            character_effects: false,
            report: super::BattleTacticalImpact {
                impact: BattleImpactReport::default(),
                pilot_injuries: Vec::new(),
                pilot_notices: Vec::new(),
                notices: Vec::new(),
                balance: Vec::new(),
                flooding: Vec::new(),
            },
        })
    }

    /// Each material entry consumes its diagnostic roll before immunity, cover or injury.
    /// Transfers and nested explosions enter separately; armor/internal phases do not.
    fn enter_damage(&mut self) -> bool {
        self.unit_mut().dice.generic_roll();
        if !super::combat_safe::protects(self.world, self.attacker, self.id) {
            return false;
        }
        self.report
            .notices
            .extend(super::combat_safe::notice(self.attacker, self.id));
        true
    }

    /// Retain damage-entry feedback before target cover and material changes.
    fn warn_attacker(&mut self) {
        self.report
            .notices
            .extend(super::weapons_hold::damage_notice(
                self.world,
                self.attacker,
                self.id,
            ));
    }

    /// Snapshot visible mechanical damage while the original power and geometry still apply.
    fn component_observers(&self, location: CriticalLocation) -> Result<Vec<super::BattleNotice>> {
        use super::BattleSystem as System;
        let unit = self.unit();
        if self.rules.is_none() || unit.is_destroyed() {
            return Ok(Vec::new());
        }
        let loadout = unit.loadout()?;
        let Some(part) = loadout
            .systems
            .iter()
            .find(|part| part.location == location)
        else {
            return Ok(Vec::new());
        };
        let protected_gyro = part.system == System::Gyro
            && unit.gyro() == super::BattleGyro::Hardened
            && !unit.gyro_condition().1;
        if unit.power() != super::BattlePower::Running
            && !protected_gyro
            && !matches!(part.system, System::Cockpit | System::HeatSink)
        {
            return Ok(Vec::new());
        }
        let leg = unit.chassis().is_leg(location.section);
        let section = unit
            .chassis()
            .section_name(location.section)
            .replace('_', " ");
        let text = match part.system {
            System::Cockpit => "spasms for a second then remains oddly still.".to_owned(),
            System::HeatSink => format!("'s {section} is covered in a green mist!"),
            System::Engine => format!("'s {section} spews black smoke!"),
            System::JumpJet => format!("'s {section} flares as superheated plasma spews out!"),
            System::Gyro if protected_gyro => {
                "emits a screech as its hardened gyro buckles slightly!".to_owned()
            }
            System::Gyro if unit.gyro_damage() == 0 => {
                "emits a loud screech as its gyro buckles under the impact!".to_owned()
            }
            System::ShoulderOrHip if leg => "'s hip locks into place!".to_owned(),
            System::UpperActuator | System::LowerActuator | System::HandOrFootActuator if leg => {
                if !loadout.systems.iter().any(|part| {
                    part.location.section == location.section
                        && part.system == System::ShoulderOrHip
                        && !unit.critical_unavailable(part.location)
                }) {
                    return Ok(Vec::new());
                }
                format!("'s {section} twists in an odd way!")
            }
            _ => return Ok(Vec::new()),
        };
        Ok(super::broadcast::observer_notices(
            self.world, self.id, &text,
        ))
    }

    /// Describe equipment losses to occupants independently of power or visibility.
    fn occupant_component_notice(
        &self,
        location: CriticalLocation,
        system: super::BattleSystem,
    ) -> Option<super::BattleNotice> {
        use super::BattleSystem as System;
        self.rules?;
        let leg = self.unit().chassis().is_leg(location.section);
        let arm = !leg
            && matches!(
                location.section,
                BattleSection::LeftArm | BattleSection::RightArm
            );
        let text = match system {
            System::HeatSink => "You lost a heat sink!".to_owned(),
            System::Cockpit => "Your cockpit is destroyed, your blood boils, and your body is fried! [fg=yellow]You're dead![reset]".to_owned(),
            System::ShoulderOrHip if leg => {
                "Your hip takes a direct hit and freezes up!".to_owned()
            }
            System::ShoulderOrHip if arm => {
                "Your shoulder joint takes a hit and is frozen!".to_owned()
            }
            System::UpperActuator | System::LowerActuator | System::HandOrFootActuator if leg => {
                "One of your leg actuators is destroyed!".to_owned()
            }
            System::UpperActuator | System::LowerActuator | System::HandOrFootActuator if arm => {
                let side = if location.section == BattleSection::LeftArm {
                    "left"
                } else {
                    "right"
                };
                let part = match system {
                    System::UpperActuator => "upper arm",
                    System::LowerActuator => "lower arm",
                    _ => "hand",
                };
                format!("Your {side} {part} actuator is destroyed!")
            }
            _ => return None,
        };
        Some(super::BattleNotice {
            unit: self.id,
            text,
        })
    }

    /// Disable selected equipment before resolving nested damage and explosion injury.
    fn lose_critical(&mut self, location: CriticalLocation) -> Result<()> {
        let computer_was_operational = self.unit().targeting_computer_operational()?;
        let null_signature_was_enabled = self.unit().null_signature().enabled;
        let stealth_was_enabled = self.unit().stealth().enabled;
        let component_observers = self.component_observers(location)?;
        let weapon_before = if self.rules.is_some() {
            self.unit()
                .loadout()?
                .weapons
                .iter()
                .enumerate()
                .find(|(_, mount)| mount.criticals.contains(&location))
                .map(|(index, mount)| -> Result<_> {
                    Ok((self.unit().weapon_intact(index)?, mount.weapon))
                })
                .transpose()?
        } else {
            None
        };
        let mut destructive_table = None;
        if let Some(BattleCriticalLoss::Weapon {
            index,
            explosion_damage: 0,
        }) = self.unit().critical_loss(location)?
            && self.unit().weapon_intact(index)?
            && !self
                .unit()
                .weapon_damage()
                .iter()
                .any(|damage| damage.location == location)
        {
            if self.unit().loadout()?.weapons[index].weapon.is_ams() {
                self.unit_mut().ams_enabled = false;
            }
            let roll = self.unit_mut().dice.generic_roll();
            if let Some(damage) = self.unit_mut().degrade_weapon(location, roll)? {
                let weapon = self.unit().loadout()?.weapons[index].weapon;
                let name = weapon.name().split_once('.').expect("catalog namespace").1;
                let detail = match damage {
                    super::BattleWeaponDamageKind::Superficial => {
                        "takes a hit but suffers no noticeable damage!!"
                    }
                    super::BattleWeaponDamageKind::Moderate => {
                        "takes a hit but continues working!!"
                    }
                    super::BattleWeaponDamageKind::Focus => {
                        "has its focusing mechanism knocked out of alignment!!"
                    }
                    super::BattleWeaponDamageKind::Crystal => "has its charging crystal damaged!!",
                    super::BattleWeaponDamageKind::Ranging => "has its ranging system damaged!!",
                    super::BattleWeaponDamageKind::Barrel => "has its barrel warped!!",
                    super::BattleWeaponDamageKind::Feed => "has its ammunition feed damaged!!",
                };
                self.report.notices.push(super::BattleNotice {
                    unit: self.id,
                    text: format!("Your {name} {detail}"),
                });
                self.report
                    .impact
                    .criticals
                    .push((location, BattleCriticalLoss::WeaponDamage { index, damage }));
                return Ok(());
            }
            destructive_table = Some(index);
        }
        let carried_before = self.unit().carried_club();
        let was_destroyed = self.unit().is_destroyed();
        let loss = if let Some(index) = destructive_table {
            self.unit_mut().destroy_degraded_weapon(index)?
        } else {
            let Some(loss) = self.unit_mut().destroy_critical(location)? else {
                return Ok(());
            };
            loss
        };
        let destroyed = self.unit().is_destroyed();
        super::kill_counters::transition(
            self.world,
            self.id,
            self.attacker,
            was_destroyed,
            destroyed,
        )?;
        if null_signature_was_enabled && !self.unit().null_signature().enabled {
            self.report.notices.push(super::BattleNotice {
                unit: self.id,
                text: "Your Null Signature System shuts down!".into(),
            });
        }
        if stealth_was_enabled && !self.unit().stealth().enabled {
            self.report.notices.push(super::BattleNotice {
                unit: self.id,
                text: "Your stealth armor system shuts down!".into(),
            });
        }
        if carried_before.is_some() && self.unit().carried_club().is_none() {
            self.report
                .notices
                .extend(super::club::dropped_notices(self.world, self.id));
        }
        let mut explosion = match &loss {
            BattleCriticalLoss::Ammunition {
                explosion_damage, ..
            } => *explosion_damage,
            BattleCriticalLoss::Weapon {
                explosion_damage, ..
            } => u32::from(*explosion_damage),
            _ => 0,
        };
        let explosion_section = if let BattleCriticalLoss::Weapon { index, .. } = loss {
            self.unit().loadout()?.weapons[index].criticals[0].section
        } else {
            location.section
        };
        if explosion == 0
            && let Some((intact, weapon)) = weapon_before
        {
            let name = weapon.name().split_once('.').expect("catalog namespace").1;
            self.report.notices.push(super::BattleNotice {
                unit: self.id,
                text: if intact {
                    format!("Your {name} has been destroyed!!")
                } else {
                    format!("Part of your non-working {name} has been hit!")
                },
            });
        }
        self.report.impact.criticals.push((location, loss.clone()));
        if let BattleCriticalLoss::System { system } = loss {
            let occupant_notice = self.occupant_component_notice(location, system);
            if system == super::BattleSystem::ShoulderOrHip {
                self.report.notices.extend(component_observers);
                self.report.notices.extend(occupant_notice);
            } else {
                self.report.notices.extend(occupant_notice);
                self.report.notices.extend(component_observers);
            }
            if self.rules.is_some() {
                use super::BattleSystem as System;
                let hits = self.unit().system_hits(system);
                let text = match system {
                    System::Engine => match hits {
                        1 | 2 => {
                            Some("Your engine shielding takes a hit! It's getting hotter in here!")
                        }
                        3 => Some("Your engine is destroyed!"),
                        _ => None,
                    },
                    System::Sensors => Some(if hits == 1 {
                        "Your sensors have been damaged!"
                    } else {
                        "Your sensors have been destroyed!"
                    }),
                    System::LifeSupport => Some("Your life support has been destroyed!"),
                    System::Gyro => Some(
                        match (self.unit().protected_gyro_hit(), self.unit().gyro_damage()) {
                            (true, _) => "Your hardened gyro takes a hit!",
                            (_, 1) => "Your Gyro has been damaged!",
                            (_, 2) => "Your Gyro has been destroyed!",
                            _ => "Your destroyed gyro takes another hit!",
                        },
                    ),
                    System::TargetingComputer if computer_was_operational => {
                        Some("Your targeting computer is destroyed!")
                    }
                    System::Axe => Some("Your axe has been destroyed!"),
                    System::Sword => Some("Your sword has been destroyed!"),
                    System::Mace => Some("Your mace has been destroyed!"),
                    System::DualSaw => Some("Your dual saw has been destroyed!"),
                    System::RetractableBlade => Some("Your retractable blade has been destroyed!"),
                    System::Lance => Some("Your lance has been destroyed!"),
                    System::Flail => Some("Your flail has been destroyed!"),
                    System::WreckingBall => Some("Your wrecking ball has been destroyed!"),
                    System::ChainWhip => Some("Your chain whip has been destroyed!"),
                    System::SmallVibroblade => Some("Your small vibroblade has been destroyed!"),
                    System::MediumVibroblade => Some("Your medium vibroblade has been destroyed!"),
                    System::LargeVibroblade => Some("Your large vibroblade has been destroyed!"),
                    System::ArtemisIv => Some("Your Artemis IV system has been destroyed!"),
                    System::Ecm => Some("Your ECM system has been destroyed!"),
                    System::NullSignature => Some("Your Null Signature System has been destroyed!"),
                    System::Tag => Some("Your TAG system has been destroyed!"),
                    System::BeagleProbe => Some("Your Beagle Active Probe has been destroyed!"),
                    System::LightProbe => Some("Your Light Active Probe has been destroyed!"),
                    System::BloodhoundProbe => {
                        Some("Your Bloodhound Active Probe has been destroyed!")
                    }
                    System::AngelEcm => Some("Your Angel ECM system has been destroyed!"),
                    System::JumpJet => Some("One of your jump jet engines has shut down!"),
                    _ => None,
                };
                if let Some(text) = text {
                    self.report.notices.push(super::BattleNotice {
                        unit: self.id,
                        text: text.to_owned(),
                    });
                }
            }
            self.balance(super::BattleBalanceCause::Critical { location, system })?;
        }
        if self.rules.is_some()
            && let BattleCriticalLoss::Ammunition { index, .. } = loss
            && self.unit().loadout()?.ammunition[index]
                .weapon
                .weapon_explosion_damage()
                > 0
        {
            self.report.notices.push(super::BattleNotice {
                unit: self.id,
                text: "One of your Gauss Rifle ammo feeds is destroyed".to_owned(),
            });
        }
        let inferno = if let BattleCriticalLoss::Ammunition { index, .. } = &loss {
            self.unit().loadout()?.ammunition[*index].mode == super::BattleAmmunitionMode::Inferno
        } else {
            false
        };
        if explosion > 0 {
            if self.rules.is_some() {
                if let BattleCriticalLoss::Weapon { index, .. } = loss {
                    let weapon = self.unit().loadout()?.weapons[index].weapon;
                    let name = weapon.name().split_once('.').expect("catalog namespace").1;
                    let detail = if weapon.weapon_explosion_damage() > 0 {
                        format!("It explodes for {explosion} points damage.")
                    } else if weapon.supports_hotload() {
                        format!(
                            "[fg=red bold]Your hotloaded launcher explodes for {explosion} points of damage![reset]"
                        )
                    } else {
                        format!(
                            "[fg=red bold]The incendiary ammunition in your launcher ignites for {explosion} points of damage![reset]"
                        )
                    };
                    let notices = [format!("Your {name} has been destroyed!"), detail];
                    for text in notices {
                        self.report.notices.push(super::BattleNotice {
                            unit: self.id,
                            text,
                        });
                    }
                } else {
                    self.report.notices.push(super::BattleNotice {
                        unit: self.id,
                        text: "Ammunition explosion!".to_owned(),
                    });
                }
                let broadcast = match loss {
                    BattleCriticalLoss::Ammunition { .. } => Some(
                        if inferno {
                            "is suddenly enveloped by a brilliant fireball!"
                        } else {
                            "has an internal ammo explosion!"
                        }
                        .to_owned(),
                    ),
                    BattleCriticalLoss::Weapon { index, .. } if !self.unit().is_destroyed() => {
                        let weapon = self.unit().loadout()?.weapons[index].weapon;
                        let section = self
                            .unit()
                            .chassis()
                            .section_name(explosion_section)
                            .replace('_', " ");
                        Some(if weapon.weapon_explosion_damage() > 0 {
                            format!("'s {section} is covered in a large electrical discharge!")
                        } else if weapon.supports_hotload() {
                            "loses a launcher in a brilliant explosion!".to_owned()
                        } else {
                            format!("'s {section} is engulfed in a brilliant blue flame!")
                        })
                    }
                    _ => None,
                };
                if let Some(text) = broadcast {
                    self.report.notices.extend(
                        super::observer_messages(self.world, self.id, &text)
                            .into_iter()
                            .map(|(unit, text)| super::BattleNotice { unit, text }),
                    );
                }
            }
            if inferno {
                let missiles = u16::try_from(explosion / 4)?;
                if missiles > 0 {
                    let exposure = super::resolve_inferno_hit(self.world, self.id, missiles)?;
                    self.report.notices.extend(exposure.notices);
                }
                if self.rules.is_some_and(|rules| rules.hit.inferno_penalty) {
                    self.unit_mut().heat.stored += 30.0;
                }
                explosion /= 2;
            }
            let vented = self.unit().has_case_ii(explosion_section);
            if vented {
                self.vent_explosion(explosion_section, u16::try_from(explosion)?)?;
            } else {
                resolve_path(
                    self,
                    DamagePacket {
                        announced: false,
                        direct_hit: false,
                        section: explosion_section,
                        damage: u16::try_from(explosion)?,
                        internal_only: true,
                        transfer: true,
                        rear: false,
                        tac: false,
                        weapon_effect: None,
                        critical_penalty: 0,
                    },
                )?;
            }
            let hotload = if let BattleCriticalLoss::Weapon { index, .. } = loss {
                self.unit().loadout()?.weapons[index]
                    .weapon
                    .supports_hotload()
            } else {
                false
            };
            if self.rules.is_some() && matches!(loss, BattleCriticalLoss::Weapon { .. }) && !hotload
            {
                self.report.notices.push(super::BattleNotice {
                    unit: self.id,
                    text: "You take personal injury from the weapon's explosion!".to_owned(),
                });
            }
            if !hotload {
                self.effect(if vented {
                    BattleImpactEffect::VentedExplosionInjury
                } else {
                    BattleImpactEffect::ExplosionInjury
                })?;
            }
        }
        Ok(())
    }

    /// CASE II: the section takes one internal point, with its normal critical roll, and the
    /// rest of the blast is vented through its armor (rear armor on torsos). Damage beyond that
    /// armor is lost and never transfers to another section.
    fn vent_explosion(&mut self, section: BattleSection, damage: u16) -> Result<()> {
        resolve_path(
            self,
            DamagePacket {
                announced: false,
                direct_hit: false,
                section,
                damage: damage.min(1),
                internal_only: true,
                transfer: false,
                rear: false,
                tac: false,
                weapon_effect: None,
                critical_penalty: 0,
            },
        )?;
        let vented = damage.saturating_sub(1);
        if vented == 0 || self.unit().sections()[&section].internal == 0 {
            return Ok(());
        }
        let vented = self
            .unit()
            .hardened_hit(section, true, vented)
            .map_or(vented, |(removed, _)| removed);
        let armor = self.damage_phase(section, vented, BattleDamagePhase::Armor { rear: true })?;
        self.record_phase(armor)?;
        if self.rules.is_some() {
            self.report.notices.push(super::BattleNotice {
                unit: self.id,
                text: "Your CASE II vents the explosion!".to_owned(),
            });
        }
        Ok(())
    }

    /// Announce computer loss caused by section destruction at the material-damage event.
    fn damage_phase(
        &mut self,
        section: BattleSection,
        damage: u16,
        phase: BattleDamagePhase,
    ) -> Result<BattleDamageResult> {
        let power_before = self.unit().power();
        let engine_hits_before = self.unit().system_hits(super::BattleSystem::Engine);
        let assisted = self.unit().targeting_computer_operational()?;
        let was_destroyed = self.unit().is_destroyed();
        let result = self.unit_mut().damage_phase(section, damage, phase);
        let destroyed = self.unit().is_destroyed();
        super::kill_counters::transition(
            self.world,
            self.id,
            self.attacker,
            was_destroyed,
            destroyed,
        )?;
        if !result.destroyed_sections.is_empty()
            && let Some(rules) = self.rules
            && let Some(blast) = super::reactor_instability::section_loss(
                self.world,
                self.id,
                power_before,
                engine_hits_before,
                rules,
            )?
        {
            super::piloting::append_feedback(
                &mut self.report.pilot_notices,
                blast.pilot_notices.iter().cloned(),
                self.report.notices.len(),
            );
            self.report.notices.extend(blast.notices.iter().cloned());
            self.report.impact.reactor_explosions.push(blast);
        }

        if assisted && !self.unit().targeting_computer_operational()? && self.rules.is_some() {
            self.report.notices.push(super::BattleNotice {
                unit: self.id,
                text: "Your Targeting Computer is Destroyed".to_owned(),
            });
        }
        Ok(result)
    }

    /// Borrow the latest unit, including any intervening fall damage.
    fn unit(&self) -> &BattleUnit {
        &self.world.btech.constructed_units()[&self.id]
    }

    /// Front torso armor hits expose an installed lamp before armor and critical resolution.
    fn strike_searchlight(&mut self, section: BattleSection, rear: bool) {
        if rear
            || !matches!(
                section,
                BattleSection::LeftTorso | BattleSection::CenterTorso | BattleSection::RightTorso
            )
        {
            return;
        }
        if let Some((notices, broadcasts)) = super::searchlight::strike(self.world, self.id) {
            self.report.impact.searchlight_destroyed = true;
            if self.rules.is_some() {
                self.report.notices.extend(broadcasts);
                self.report.notices.extend(notices);
            }
        }
    }

    /// Mutate only the unit owned by this cascade.
    fn unit_mut(&mut self) -> &mut BattleUnit {
        self.world.btech.constructed.get_mut(&self.id).unwrap()
    }

    /// Bound nested ammunition work; the outer checkpoint owns rollback.
    fn spend(&mut self) -> Result<()> {
        ensure!(self.work > 0, "Damage cascade exceeds work limit");
        self.work -= 1;
        Ok(())
    }

    /// Tactical crew consequences occur at their damage event, before subsequent criticals.
    fn effect(&mut self, effect: BattleImpactEffect) -> Result<()> {
        if effect == BattleImpactEffect::CrewStun
            && (self.character_toughness.is_some() || self.rules.is_some())
        {
            self.report
                .notices
                .push(super::stun_unit(self.world, self.id)?);
            return Ok(());
        }
        if let Some(toughness) = self.character_toughness
            && matches!(
                effect,
                BattleImpactEffect::HeadInjury
                    | BattleImpactEffect::ExplosionInjury
                    | BattleImpactEffect::VentedExplosionInjury
            )
            && !self.unit().is_destroyed()
            && self.unit().pilot().is_some()
        {
            let resistant = self
                .unit()
                .pilot()
                .and_then(|pilot| self.world.btech.character_values().get(&pilot))
                .is_some_and(|values| super::advantages::enabled(values, "Pain_Resistance"));
            let hits = if effect == BattleImpactEffect::ExplosionInjury && !resistant {
                2
            } else {
                1
            };
            let injury =
                super::pilot_injury::injure_in_candidate(self.world, self.id, hits, toughness)?;
            let destroyed = self.unit().is_destroyed();
            super::kill_counters::transition(self.world, self.id, self.attacker, false, destroyed)?;
            self.record_injury(injury);
            return Ok(());
        }
        let Some(rules) = self.rules else {
            self.report.impact.pending_effects.push(effect);
            return Ok(());
        };
        let hits = match effect {
            BattleImpactEffect::HeadInjury => Some(1),
            BattleImpactEffect::ExplosionInjury => {
                let resistant = self
                    .unit()
                    .pilot()
                    .and_then(|pilot| self.world.btech.character_values().get(&pilot))
                    .is_some_and(|values| super::advantages::enabled(values, "Pain_Resistance"));
                Some(if resistant { 1 } else { 2 })
            }
            BattleImpactEffect::VentedExplosionInjury => Some(1),
            _ => None,
        };
        if let Some(hits) = hits.filter(|_| !self.unit().is_destroyed()) {
            if matches!(
                effect,
                BattleImpactEffect::ExplosionInjury | BattleImpactEffect::VentedExplosionInjury
            ) {
                self.report.notices.push(super::BattleNotice {
                    unit: self.id,
                    text: "You take personal injury from the ammunition explosion!".to_owned(),
                });
            }
            let injury = super::pilot_injury::injure_in_candidate(
                self.world,
                self.id,
                hits,
                rules.toughness,
            )?;
            let destroyed = self.unit().is_destroyed();
            super::kill_counters::transition(self.world, self.id, self.attacker, false, destroyed)?;
            self.record_injury(injury);
            return Ok(());
        }
        self.report.impact.pending_effects.push(effect);
        Ok(())
    }

    /// Preserve each health mode's report while using the common injury dispatcher.
    fn record_injury(&mut self, injury: super::pilot_injury::PilotInjury) {
        match injury {
            super::pilot_injury::PilotInjury::Character(report) => {
                self.report.impact.character_injuries.push(report)
            }
            super::pilot_injury::PilotInjury::Tactical(report) => {
                if let Some(notice) = report.notice(self.id) {
                    self.report.notices.push(notice);
                }
                self.report.pilot_injuries.push(report);
            }
        }
    }

    /// Complete each new balance consequence before the next critical or transfer phase.
    fn balance(&mut self, cause: super::BattleBalanceCause) -> Result<()> {
        let Some(rules) = self.rules else {
            return Ok(());
        };
        let airborne = self.unit().airborne();
        if let Some(mut report) =
            super::balance::resolve_balance(self.world, self.id, cause, rules)?
        {
            if let Some(check) = &report.check {
                super::piloting::capture_feedback(
                    self.id,
                    report.pilot,
                    check,
                    &mut self.report.notices,
                    &mut self.report.pilot_notices,
                );
            }
            if let Some(fall) = &report.fall {
                self.report.notices.push(super::BattleNotice {
                    unit: self.id,
                    text: (if airborne {
                        if matches!(
                            cause,
                            super::BattleBalanceCause::Critical {
                                system: super::BattleSystem::JumpJet,
                                ..
                            } | super::BattleBalanceCause::SectionLost(_)
                        ) {
                            "Losing your last jump jet, you fall from the sky!"
                        } else {
                            "You fall from the sky!"
                        }
                    } else {
                        "You lose your balance and fall down!"
                    })
                    .to_owned(),
                });
                self.report
                    .notices
                    .extend(report.observer_notices.iter().cloned());
                fall.append_notices(
                    self.id,
                    &mut self.report.notices,
                    &mut self.report.pilot_notices,
                );
                if airborne {
                    let input = super::stacking::physical_input(
                        self.world,
                        self.id,
                        super::BattleStackingEntry::Fall,
                    )?;
                    let offset = self.report.notices.len();
                    let notices = if self.character_effects {
                        let mut effects = super::stacking::StackingEffects::default();
                        let notices = super::stacking::resolve_in_action(
                            self.world,
                            self.id,
                            input,
                            rules.stacking,
                            rules,
                            &mut effects,
                            (&mut self.report.pilot_notices, offset),
                        )?;
                        report
                            .experience_messages
                            .extend(effects.experience_messages);
                        report.collision_impacts = effects.impacts;
                        report.collision_falls = effects.falls;
                        notices
                    } else {
                        super::stacking::resolve_pure_with_feedback(
                            self.world,
                            self.id,
                            input,
                            rules.stacking,
                            rules,
                            &mut self.report.pilot_notices,
                            offset,
                        )?
                    };
                    self.report.notices.extend(notices);
                }
            }
            self.report.balance.push(report);
        }
        Ok(())
    }

    /// Check vacuum at the end of this section's material path, before damage transfers.
    fn check_vacuum(&mut self, section: BattleSection, penetrating: bool) -> Result<()> {
        if !super::vacuum::check_mech(self.world, self.id, section, penetrating)? {
            return Ok(());
        }
        let assisted = self.unit().targeting_computer_operational()?;
        let jets_before = self.unit().jump_capacity(100)?.speed;
        let jet_location = self
            .unit()
            .loadout()?
            .systems
            .iter()
            .find(|part| {
                part.location.section == section
                    && part.system == super::BattleSystem::JumpJet
                    && !self.unit().critical_unavailable(part.location)
            })
            .map(|part| part.location);
        ensure!(
            self.character_toughness.is_some()
                || !self.world.objects[&self.id]
                    .flags
                    .contains(Flag::InCharacter),
            "Character vacuum damage requires casualty publication"
        );
        let mut report = super::section_exposure::disable_section(
            self.world,
            self.id,
            section,
            super::BattleSectionExposure::Vacuum,
            self.rules,
            self.attacker,
        )?;
        if assisted && !self.unit().targeting_computer_operational()? {
            report.notices.push(super::BattleNotice {
                unit: self.id,
                text: "Your Targeting Computer is Destroyed".into(),
            });
        }
        super::piloting::append_feedback(
            &mut self.report.pilot_notices,
            report.pilot_notices.iter().cloned(),
            self.report.notices.len(),
        );
        self.report.notices.extend(report.notices.iter().cloned());
        self.report.impact.exposures.push(report);
        if self.unit().airborne()
            && jets_before >= 10.75
            && self.unit().jump_capacity(100)?.speed < 10.75
            && let Some(location) = jet_location
        {
            self.balance(super::BattleBalanceCause::Critical {
                location,
                system: super::BattleSystem::JumpJet,
            })?;
        }
        Ok(())
    }

    /// Preserve section notices while immediately applying any forced leg-loss fall.
    fn record_phase(&mut self, phase: BattleDamageResult) -> Result<()> {
        let section = phase.section;
        let destroyed = phase.destroyed_sections.clone();
        self.report.impact.phases.push(phase);
        for section in destroyed {
            self.effect(BattleImpactEffect::SectionLost(section))?;
            self.balance(super::BattleBalanceCause::SectionLost(section))?;
        }
        let assisted = self.unit().targeting_computer_operational()?;
        let flood = if let Some(rules) = self.rules {
            if self.character_toughness.is_some() {
                super::flooding::flood_section_in_action(self.world, self.id, section, rules)?
            } else {
                super::flooding::flood_section(self.world, self.id, section, rules)?
            }
        } else {
            None
        };
        if let Some(mut report) = flood {
            if assisted && !self.unit().targeting_computer_operational()? {
                report.notices.push(super::BattleNotice {
                    unit: self.id,
                    text: "Your Targeting Computer is Destroyed".to_owned(),
                });
            }
            super::piloting::append_feedback(
                &mut self.report.pilot_notices,
                report.pilot_notices.iter().cloned(),
                self.report.notices.len(),
            );
            self.report.notices.extend(report.notices.iter().cloned());
            self.report.flooding.push(report);
        }
        Ok(())
    }
}

/// Transfer policy and remaining damage for one weapon hit or internal explosion.
struct DamagePacket {
    announced: bool,
    /// Initial weapon-hit effects wait until cocoon interception has allowed material damage.
    direct_hit: bool,
    section: BattleSection,
    damage: u16,
    internal_only: bool,
    transfer: bool,
    rear: bool,
    tac: bool,
    weapon_effect: Option<WeaponEffect>,
    /// Subtracted from this packet's critical-hit rolls, as for a glancing blow.
    critical_penalty: u8,
}

/// Follow armor/internal transfer links, resolving explosion recursion before the interrupted hit resumes.
fn resolve_path(context: &mut ImpactContext<'_>, packet: DamagePacket) -> Result<()> {
    let DamagePacket {
        mut announced,
        mut direct_hit,
        mut section,
        mut damage,
        internal_only,
        transfer,
        rear,
        tac,
        weapon_effect,
        critical_penalty,
    } = packet;
    let mut initial_packet = true;
    let mut plasma_returns = 0;
    let mut ignition_section = None;
    while damage > 0 {
        context.spend()?;
        if !announced {
            if context.enter_damage() {
                return Ok(());
            }
            context.warn_attacker();
        }
        announced = false;
        if context.unit().sections()[&section].internal == 0 {
            if !transfer || (internal_only && context.unit().has_case(section)) {
                break;
            }
            let Some(next) = section.damage_transfer() else {
                break;
            };
            initial_packet = false;
            section = next;
            continue;
        }
        if let Some(notices) = super::orbital_drop_combat::intercept(
            context.world,
            context.id,
            context.attacker,
            u32::from(damage),
        )? {
            context.report.notices.extend(notices);
            return Ok(());
        }
        if initial_packet {
            super::damage_counters::record(
                context.world,
                context.id,
                context.attacker,
                u32::from(damage),
            )?;
            initial_packet = false;
        }
        if direct_hit {
            direct_hit = false;
            context
                .report
                .notices
                .extend(super::hiding::damage(context.world, context.id));
            if let Some(rules) = context
                .rules
                .filter(|_| context.unit().posture() != super::BattlePosture::Prone)
            {
                // Errata: each hardened armor point lost counts as one damage toward the
                // twenty-damage piloting check; damage beyond the armor counts in full.
                let counted = context
                    .unit()
                    .hardened_hit(section, rear, damage)
                    .map_or(damage, |(removed, overflow)| removed + overflow);
                context.unit_mut().stagger.record(counted, rules.stagger)?;
            }
        }
        plasma_returns += 1;
        if rear
            && weapon_effect.is_some()
            && matches!(
                section,
                BattleSection::LeftTorso | BattleSection::RightTorso | BattleSection::CenterTorso
            )
        {
            ignition_section = Some(section);
        }

        if section == BattleSection::Head {
            context.effect(BattleImpactEffect::HeadInjury)?;
        }
        let mut tac_criticals = 0;
        if !internal_only {
            context.strike_searchlight(section, rear);
            let previous_warning =
                super::combat_warnings::armor_level(context.unit(), section, rear);
            // Hardened armor points each stop two damage; overflow passes at full value.
            let hardened = context.unit().hardened_hit(section, rear, damage);
            let armor = context.damage_phase(
                section,
                hardened.map_or(damage, |(removed, _)| removed),
                BattleDamagePhase::Armor { rear },
            )?;
            let warning = super::combat_warnings::armor_level(context.unit(), section, rear);
            damage = hardened.map_or(armor.remaining, |(_, overflow)| overflow);
            // Hardened armor negates armor-piercing critical chances.
            let ap = match weapon_effect {
                Some(WeaponEffect::ArmorPiercing(weapon)) if hardened.is_none() => Some(weapon),
                _ => None,
            }
            .filter(|_| {
                if tac || damage > 0 {
                    return false;
                }
                let state = &context.unit().sections()[&section];
                let original = &context.unit().definition().sections[&section];
                let (remaining, original) = if rear
                    && matches!(
                        section,
                        BattleSection::LeftTorso
                            | BattleSection::RightTorso
                            | BattleSection::CenterTorso
                    ) {
                    (state.rear, original.rear)
                } else {
                    (state.armor, original.armor)
                };
                u32::from(remaining) * 2 < u32::from(original)
            });
            context.record_phase(armor)?;
            if tac || ap.is_some() {
                let roll = context.unit_mut().dice.generic_roll();
                let adjusted =
                    roll.saturating_sub(ap.map_or(0, super::BattleWeapon::armor_piercing_penalty));
                tac_criticals = critical_count(adjusted);
                resolve_criticals(context, section, tac_criticals)?;
            }
            if warning > previous_warning
                && context.rules.is_some()
                && context.unit().armor_warning()
            {
                context.report.notices.push(super::BattleNotice {
                    unit: context.id,
                    text: super::combat_warnings::armor_message(section, rear, warning),
                });
            }
        }
        let penetrating = !internal_only && damage > 0;
        if damage > 0 {
            // The reference consumes this roll even when a TAC already supplied criticals.
            // Damage that penetrated hardened armor rolls two lower.
            let penalty = if !internal_only && context.unit().hardened_armor() {
                super::BattleTechnology::HARDENED_CRITICAL_PENALTY
            } else {
                0
            };
            let roll = context
                .unit_mut()
                .dice
                .generic_roll()
                .saturating_sub(critical_penalty.saturating_add(penalty));
            if tac_criticals == 0 {
                if roll == 12
                    && matches!(
                        section,
                        BattleSection::LeftArm
                            | BattleSection::RightArm
                            | BattleSection::LeftLeg
                            | BattleSection::RightLeg
                            | BattleSection::Head
                    )
                {
                    let remaining = context.unit().sections()[&section].internal;
                    let phase =
                        context.damage_phase(section, remaining, BattleDamagePhase::Internal)?;
                    context.record_phase(phase)?;
                    break;
                }
                resolve_criticals(context, section, critical_count(roll))?;
            }
            if section == BattleSection::CenterTorso
                && damage > 0
                && context.unit().sections()[&section].internal
                    == context.unit().definition().sections[&section].internal
            {
                context.unit_mut().reactor_instability_remaining = Some(31);
            }
            // Reinforced structure halves and composite structure doubles internal damage,
            // including any overflow that transfers onward.
            let structural = context.unit().structure_damage(damage);
            let phase = context.damage_phase(section, structural, BattleDamagePhase::Internal)?;
            damage = phase.remaining;
            context.record_phase(phase)?;
        }
        if internal_only {
            if context.unit().sections()[&section].internal > 0 {
                context.check_vacuum(section, false)?;
            }
        } else if !penetrating || (damage == 0 && context.unit().sections()[&section].internal > 0)
        {
            context.check_vacuum(section, penetrating)?;
        }
        if !transfer || (internal_only && context.unit().has_case(section)) {
            break;
        }
        let Some(next) = section.damage_transfer() else {
            // Overflow beyond the core returns before its plasma effect; callers still unwind.
            if damage > 0 {
                plasma_returns -= 1;
            }
            break;
        };
        section = next;
    }
    if matches!(weapon_effect, Some(WeaponEffect::Plasma)) {
        for _ in 0..plasma_returns {
            let heat = context.unit_mut().dice.d6();
            context.unit_mut().heat.stored += f64::from(heat);
            context.report.impact.plasma_heat.push(heat);
        }
    }
    if let Some(section) = ignition_section {
        resolve_dump_ignition(context, section)?;
    }
    Ok(())
}

/// Ejected ammunition strikes rear armor and uses the same ordered damage/crew cascade.
fn resolve_dump_ignition(context: &mut ImpactContext<'_>, section: BattleSection) -> Result<()> {
    let Some(ignition) = super::dumping::ignition(context.unit_mut(), section)? else {
        return Ok(());
    };
    if context.rules.is_some() {
        context
            .report
            .notices
            .extend(super::broadcast::observer_notices(
                context.world,
                context.id,
                "'s rear armor lights up as ammo being dumped ignites!",
            ));
        context.report.notices.push(super::BattleNotice {
            unit: context.id,
            text: format!(
                "[fg=red bold]Some of the {} ammo dumping out of your mech ignites![reset]",
                ignition.weapon.name()
            ),
        });
    }
    let attacker = context.attacker.take();
    let result = resolve_path(
        context,
        DamagePacket {
            announced: false,
            direct_hit: true,
            section,
            damage: ignition.damage,
            internal_only: false,
            transfer: true,
            rear: true,
            tac: false,
            weapon_effect: None,
            critical_penalty: 0,
        },
    );
    context.attacker = attacker;
    result?;
    let unit = context.unit_mut();
    unit.ammunition[ignition.bin_index] = unit.ammunition[ignition.bin_index].saturating_sub(1);
    unit.live_mass.invalidate();
    unit.dumping = None;
    if context.rules.is_some() {
        context.report.notices.push(super::BattleNotice {
            unit: context.id,
            text: "[fg=red bold]All ammo dumping operations have stopped![reset]".into(),
        });
    }
    context.report.impact.dump_ignitions.push(ignition);
    Ok(())
}

/// Conventional 2d6 critical multiplicities; limb/head severing is handled by the internal phase.
fn critical_count(roll: u8) -> u8 {
    match roll {
        8 | 9 => 1,
        10 | 11 => 2,
        12 => 3,
        _ => 0,
    }
}

/// Select critical outcomes from current eligible slots; linked extensions can select their primary repeatedly.
fn resolve_criticals(
    context: &mut ImpactContext<'_>,
    section: BattleSection,
    count: u8,
) -> Result<()> {
    for _ in 0..count {
        context.spend()?;
        let Some(location) = context.unit_mut().choose_critical(section) else {
            break;
        };
        context.lose_critical(location)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Jenner, optionally with hardened armor, seeded for a deterministic damage stream.
    fn world(hardened: bool, seed: u8) -> (World, ObjectId) {
        let mut template = super::super::BattleTemplate::parse(include_str!(
            "../../tests/fixtures/btech/mechs/JR7-D"
        ))
        .unwrap();
        if hardened {
            let specials = template.attributes.entry("specials".into()).or_default();
            specials.push_str(" HardenedArmor_Tech");
        }
        let mut unit = BattleUnit::from_template(template).unwrap();
        unit.dice = super::super::BattleDice::seeded([seed; 32]);
        let mut world = World::default();
        let id = ObjectId(41);
        world.btech.constructed.insert(id, unit);
        (world, id)
    }

    /// An unguided hit on the left arm, with or without armor-piercing ammunition.
    fn strike(world: &mut World, id: ObjectId, damage: u16, effect: WeaponEffect) {
        resolve_attack_in_candidate(
            world,
            id,
            BattleHit {
                section: BattleSection::LeftArm,
                rear_armor: false,
                through_armor_critical: false,
                crew_stun: false,
            },
            damage,
            None,
            AttackImpact {
                attacker: None,
                weapon_effect: Some(effect),
                character: false,
                followup: false,
            },
        )
        .unwrap();
    }

    /// Hardened armor negates the armor-piercing critical check, so an AP round consumes
    /// exactly the dice of an ordinary round. Against standard armor the check still rolls.
    #[test]
    fn hardened_armor_negates_armor_piercing_criticals() {
        let ap = WeaponEffect::ArmorPiercing(super::super::BattleWeapon::Ac10);
        // Six hardened damage leaves one of four armor points: exposed but not breached.
        let (mut piercing, id) = world(true, 3);
        let (mut conventional, _) = world(true, 3);
        strike(&mut piercing, id, 6, ap);
        strike(&mut conventional, id, 6, WeaponEffect::Conventional);
        let (piercing, conventional) = (
            &piercing.btech.constructed_units()[&id],
            &conventional.btech.constructed_units()[&id],
        );
        assert_eq!(piercing.dice, conventional.dice);
        assert_eq!(piercing.sections(), conventional.sections());
        assert_eq!(piercing.lost_criticals(), conventional.lost_criticals());
        let (mut piercing, _) = world(false, 3);
        let (mut conventional, _) = world(false, 3);
        strike(&mut piercing, id, 3, ap);
        strike(&mut conventional, id, 3, WeaponEffect::Conventional);
        assert_ne!(
            piercing.btech.constructed_units()[&id].dice,
            conventional.btech.constructed_units()[&id].dice
        );
    }

    /// Damage that penetrates hardened armor rolls criticals two lower.
    #[test]
    fn hardened_armor_lowers_penetrating_critical_rolls() {
        let mut exercised = [false, false];
        for seed in 0..=63 {
            let mut dice = super::super::BattleDice::seeded([seed; 32]);
            dice.two_d6();
            let roll = dice.two_d6();
            // Twelve damage strips four hardened points and puts four into the structure.
            let (mut world, id) = world(true, seed);
            let before = world.btech.constructed_units()[&id].lost_criticals().len();
            strike(&mut world, id, 12, WeaponEffect::Conventional);
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.sections()[&BattleSection::LeftArm].internal, 2);
            let critical = unit.lost_criticals().len() > before;
            assert_eq!(critical, roll >= 10, "seed {seed} roll {roll}");
            if (8..10).contains(&roll) {
                exercised[0] = true;
            }
            if roll >= 10 {
                exercised[1] = true;
            }
        }
        assert_eq!(exercised, [true, true]);
    }
}
