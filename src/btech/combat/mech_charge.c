// NOLINTBEGIN(misc-include-cleaner): Direct dependencies exceed file-size cap.
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_physical_internal.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_targeting_api.h"
#include "mux/support/alloc.h"
#include <stdio.h>
static int charge_forward_arc(Mech *mech, const Mech *target) {
  MechConditionSummary condition = mech_condition_summary(mech);
  mech_torso_twist_set(mech, MECH_TORSO_CENTER);
  int arc = in_weapon_arc(mech, mech_position_real_x(target),
                          mech_position_real_y(target));
  if (condition.torso_left)
    mech_torso_twist_set(mech, MECH_TORSO_LEFT);
  else if (condition.torso_right)
    mech_torso_twist_set(mech, MECH_TORSO_RIGHT);
  return arc;
}
typedef struct ChargeDamageRequest {
  const Mech *moving;
  const Mech *opponent;
  const Mech *mass_source;
  bool uses_new_rules;
  int divisor;
  int bonus;
} ChargeDamageRequest;
static int charge_damage_calculate(const ChargeDamageRequest *request) {
  const Mech *moving = request->moving;
  const Mech *opponent = request->opponent;
  constexpr float DEGREES_TO_RADIANS = 0.017453292519943295F;
  const float CHARGE_DISTANCE = mech_charge_distance(moving);
  const float MOVING_SPEED = request->uses_new_rules
                                 ? CHARGE_DISTANCE * MP1
                                 : mech_current_speed(moving);
  const float OPPONENT_SPEED = mech_current_speed(opponent);
  const int HEADING_DIFFERENCE =
      mech_heading_degrees(moving) - mech_heading_degrees(opponent);
  const float COLLISION_SPEED =
      MOVING_SPEED -
      (OPPONENT_SPEED * cosf((float)HEADING_DIFFERENCE * DEGREES_TO_RADIANS));
  const int MASS = mech_real_tonnage(request->mass_source);
  const float DAMAGE = (COLLISION_SPEED * MP_PER_KPH * ((float)MASS + 5.0F) /
                        (float)request->divisor) +
                       (float)request->bonus;
  return (int)DAMAGE;
}
void charge_mech(Mech *mech, Mech *target) {
  char message_buffer[LBUF_SIZE];
  int base_to_hit = 5;
  int roll;
  int hit_group;
  int hitloc;
  int isrear = 0;
  int iscritical = 0;
  int target_damage;
  int mech_damage;
  int received_damage;
  int inflicted_damage;
  int spread;
  int i;
  int mech_charge;
  int target_charge;
  int mech_base_to_hit;
  int targ_base_to_hit;
  int mech_roll;
  int targ_roll;
  int done = 0;
  char location[50];
  int iwa;
  BtechContext *context = mech_context(mech);
  char emit_buff[LBUF_SIZE];
  if (mech_charge_target_dbref(target) == mech_dbref(mech)) {
    mech_charge = 1;
    target_charge = 1;
    /* Check the sections of the first unit for weapons that are cycling */
    done = 0;
    for (i = 0; i < CHARGE_SECTIONS && !done; i++) {
      const int SECTION = physical_charge_section(i);
      if (mech_section_has_recycling_weapon(mech, SECTION)) {
        armor_string_from_index(SECTION, location, mech_class(mech),
                                mech_movement_type(mech));
        mech_printf(mech, MECHALL, "You have weapons recycling on your %s.",
                    location);
        mech_charge = 0;
        done = 1;
      }
    }
    /* Check the sections of the second unit for weapons that are cycling */
    done = 0;
    for (i = 0; i < CHARGE_SECTIONS && !done; i++) {
      const int SECTION = physical_charge_section(i);
      if (mech_section_has_recycling_weapon(target, SECTION)) {
        armor_string_from_index(SECTION, location, mech_class(target),
                                mech_movement_type(target));
        mech_printf(target, MECHALL, "You have weapons recycling on your %s.",
                    location);
        target_charge = 0;
        done = 1;
      }
    }
    /* Is the second unit capable of charging */
    if (!mech_is_started(target) || mech_pilot_is_unconscious(target) ||
        mech_is_blinded(target))
      target_charge = 0;
    /* Is the first unit capable of charging */
    if (!mech_is_started(mech) || mech_pilot_is_unconscious(mech) ||
        mech_is_blinded(mech))
      mech_charge = 0;
    /* Is the first unit moving fast enough to charge */
    if (mech_current_speed(mech) < MP1) {
      mech_notify(mech, MECHALL, "You aren't moving fast enough to charge.");
      mech_charge = 0;
    }
    /* Is the second unit moving fast enough to charge */
    if (mech_current_speed(target) < MP1) {
      mech_notify(target, MECHALL, "You aren't moving fast enough to charge.");
      target_charge = 0;
    }
    /* Check to see if any sections cycling from a previous attack */
    if (mech_class(mech) == CLASS_MECH) {
      /* Is the first unit's legs cycling */
      if (mech_section_recycle_ticks(mech, LLEG) ||
          mech_section_recycle_ticks(mech, RLEG)) {
        mech_notify(mech, MECHALL,
                    "Your legs are still recovering from your last attack.");
        mech_charge = 0;
      }
      /* Is the first unit's arms cycling */
      if (mech_section_recycle_ticks(mech, RARM) ||
          mech_section_recycle_ticks(mech, LARM)) {
        mech_notify(mech, MECHALL,
                    "Your arms are still recovering from your last attack.");
        mech_charge = 0;
      }
    } else {
      /* Is the first unit's front side cycling */
      if (mech_section_recycle_ticks(mech, FSIDE)) {
        mech_notify(mech, MECHALL,
                    "You are still recovering from your last attack!");
        mech_charge = 0;
      }
    }
    /* Check to see if any sections cycling from a previous attack */
    if (mech_class(target) == CLASS_MECH) {
      /* Is the second unit's legs cycling */
      if (mech_section_recycle_ticks(target, LLEG) ||
          mech_section_recycle_ticks(target, RLEG)) {
        mech_notify(target, MECHALL,
                    "Your legs are still recovering from your last attack.");
        target_charge = 0;
      }
      /* Is the second unit's arms cycling */
      if (mech_section_recycle_ticks(target, RARM) ||
          mech_section_recycle_ticks(target, LARM)) {
        mech_notify(target, MECHALL,
                    "Your arms are still recovering from your last attack.");
        target_charge = 0;
      }
    } else {
      /* Is the second unit's front side cycling */
      if (mech_section_recycle_ticks(target, FSIDE)) {
        mech_notify(target, MECHALL,
                    "You are still recovering from your last attack!");
        target_charge = 0;
      }
    }
    /* Is the second unit jumping */
    if (mech_is_jumping(target)) {
      mech_notify(mech, MECHALL,
                  "Your target is jumping, you charge underneath it.");
      mech_notify(target, MECHALL,
                  "You can't charge while jumping, try death from above.");
      mech_charge = 0;
      target_charge = 0;
    }
    /* Is the first unit jumping */
    if (mech_is_jumping(mech)) {
      mech_notify(target, MECHALL,
                  "Your target is jumping, you charge underneath it.");
      mech_notify(mech, MECHALL,
                  "You can't charge while jumping, try death from above.");
      mech_charge = 0;
      target_charge = 0;
    }
    /* Is the second unit fallen and the first unit not a tank */
    if (mech_condition_summary(target).fallen &&
        (mech_class(mech) != CLASS_VEH_GROUND)) {
      mech_notify(mech, MECHALL, "Your target's too low for you to charge it!");
      mech_charge = 0;
    }
    /* Not sure at the moment if I need this here, but I figured
     * couldn't hurt for now */
    /* Is the first unit fallen and the second unit not a tank */
    if (mech_condition_summary(mech).fallen &&
        (mech_class(target) != CLASS_VEH_GROUND)) {
      mech_notify(target, MECHALL,
                  "Your target's too low for you to charge it!");
      target_charge = 0;
    }
    /* If the second unit is a mech it can only charge mechs */
    if ((mech_class(target) == CLASS_MECH) &&
        (mech_class(mech) != CLASS_MECH)) {
      mech_notify(target, MECHALL, "You can only charge mechs!");
      target_charge = 0;
    }
    /* If the first unit is a mech it can only charge mechs */
    if ((mech_class(mech) == CLASS_MECH) &&
        (mech_class(target) != CLASS_MECH)) {
      mech_notify(mech, MECHALL, "You can only charge mechs!");
      mech_charge = 0;
    }
    /* If the second unit is a tank, it can only charge tanks and mechs */
    if ((mech_class(target) == CLASS_VEH_GROUND) &&
        ((mech_class(mech) != CLASS_MECH) &&
         (mech_class(mech) != CLASS_VEH_GROUND))) {
      mech_notify(target, MECHALL, "You can only charge mechs and tanks!");
      target_charge = 0;
    }
    /* If the first unit is a tank, it can only charge tanks and mechs */
    if ((mech_class(mech) == CLASS_VEH_GROUND) &&
        ((mech_class(target) != CLASS_MECH) &&
         (mech_class(target) != CLASS_VEH_GROUND))) {
      mech_notify(mech, MECHALL, "You can only charge mechs and tanks!");
      mech_charge = 0;
    }
    /* Are they stunned ? */
    if (mech_event_count(mech, EVENT_UNSTUN_CREW)) {
      mech_notify(mech, MECHALL, "You are too stunned to ram!");
      mech_charge = 0;
    }
    if (mech_event_count(target, EVENT_UNSTUN_CREW)) {
      mech_notify(target, MECHALL, "You are too stunned to ram!");
      target_charge = 0;
    }
    /* Are they trying to unjam their turrets ? */
    if (mech_event_count(mech, EVENT_UNJAM_TURRET)) {
      mech_notify(mech, MECHALL, "You are too busy unjamming your turret!");
      mech_charge = 0;
    }
    if (mech_event_count(target, EVENT_UNJAM_TURRET)) {
      mech_notify(mech, MECHALL, "You are too busy unjamming your turret!");
      target_charge = 0;
    }
    /* Check the arcs to make sure the target is in the front arc */
    if (!(charge_forward_arc(mech, target) & FORWARDARC)) {
      mech_notify(mech, MECHALL,
                  "Your charge target is not in your forward arc and you are "
                  "unable to charge it.");
      mech_charge = 0;
    }
    if (!(in_weapon_arc(target, mech_position_real_x(mech),
                        mech_position_real_y(mech)) &
          FORWARDARC)) {
      mech_notify(target, MECHALL,
                  "Your charge target is not in your forward arc and you are "
                  "unable to charge it.");
      target_charge = 0;
    }
    mech_torso_twist_merge(mech, target);
    /* Now to calculate how much damage the first unit will do */
    target_damage = charge_damage_calculate(&(ChargeDamageRequest){
        .moving = mech,
        .opponent = target,
        .mass_source = mech,
        .uses_new_rules = btech_context_uses_new_charge_rules(context),
        .divisor = 10});
    if (has_bool_advantage(context, mech_pilot_dbref(mech), "melee_specialist"))
      target_damage++;
    /* Not able to do any damage */
    if (target_damage <= 0) {
      mech_notify(
          mech, MECHPILOT,
          "Your target unit will not sustain any damage. Charge aborted!");
      mech_charge = 0;
    }
    /* Now see how much damage the second unit will do */
    mech_damage = (mech_real_tonnage(target) + 5) / 10;
    if (has_bool_advantage(context, mech_pilot_dbref(target),
                           "melee_specialist"))
      mech_damage++;
    /* Not able to do any damage */
    if (mech_damage <= 0) {
      mech_notify(target, MECHPILOT,
                  "Your unit won't sustain any dmage. Charge aborted!");
      target_charge = 0;
    }
    /* BTH for first unit */
    mech_base_to_hit = 5;
    mech_base_to_hit += find_pilot_piloting(mech) - find_pilot_piloting(target);
    mech_base_to_hit +=
        (has_bool_advantage(context, mech_pilot_dbref(mech), "melee_specialist")
             ? min(0, mech_attacker_movement_modifier(mech) - 1)
             : mech_attacker_movement_modifier(mech));
    mech_base_to_hit += mech_target_movement_modifier(mech, target, 0.0);
    /* BTH for second unit */
    targ_base_to_hit = 5;
    targ_base_to_hit += find_pilot_piloting(target) - find_pilot_piloting(mech);
    targ_base_to_hit +=
        (has_bool_advantage(context, mech_pilot_dbref(target),
                            "melee_specialist")
             ? min(0, mech_attacker_movement_modifier(target) - 1)
             : mech_attacker_movement_modifier(target));
    targ_base_to_hit += mech_target_movement_modifier(target, mech, 0.0);
    /* Now check to see if its possible for them to even charge */
    if (mech_charge) {
      if (mech_base_to_hit > 12) {
        mech_printf(mech, MECHALL, "Charge: BTH %d\tYou choose not to charge.",
                    mech_base_to_hit);
        mech_charge = 0;
      }
    }
    if (target_charge) {
      if (targ_base_to_hit > 12) {
        mech_printf(target, MECHALL,
                    "Charge: BTH %d\tYou choose not to charge.",
                    targ_base_to_hit);
        target_charge = 0;
      }
    }
    /* Since neither can charge lets exit */
    if (!mech_charge && !target_charge) {
      /* mech_charge_target_dbref(mech) and the others are set
         after the return */
      mech_charge_reset(target);
      return;
    }
    /* Roll */
    mech_roll = btech_random_roll(context);
    targ_roll = btech_random_roll(context);
    if (mech_charge)
      mech_printf(mech, MECHALL, "Charge: BTH %d\tRoll: %d", mech_base_to_hit,
                  mech_roll);
    if (target_charge)
      mech_printf(target, MECHALL, "Charge: BTH %d\tRoll: %d", targ_base_to_hit,
                  targ_roll);
    /* Ok the first unit made its roll */
    if (mech_charge && mech_roll >= mech_base_to_hit) {
      /* OUCH */
      mech_printf(target, MECHALL, "CRASH!!!\n%s charges into you!",
                  mech_to_mech_display_id(target, mech).text);
      mech_notify(mech, MECHALL, "SMASH!!! You crash into your target!");
      hit_group = mech_hit_group(mech, target);
      isrear = (hit_group == BACK);
      /* Record the damage for debugging then dish it out */
      inflicted_damage = target_damage;
      spread = target_damage / 5;
      for (i = 0; i < spread; i++) {
        hitloc = mech_hit_location(target, hit_group, &iscritical, &isrear);
        physical_damage_apply(target, mech, 1, mech_pilot_dbref(mech), hitloc,
                              isrear, iscritical, 5, 0);
      }
      if (target_damage % 5) {
        hitloc = mech_hit_location(target, hit_group, &iscritical, &isrear);
        physical_damage_apply(target, mech, 1, mech_pilot_dbref(mech), hitloc,
                              isrear, iscritical, (target_damage % 5), 0);
      }
      hit_group = mech_hit_group(target, mech);
      isrear = (hit_group == BACK);
      /* Ok now how much damage will the first unit take from
       * charging */
      if (btech_context_uses_new_charge_rules(context) &&
          btech_context_uses_technology_level_three_charge_rules(context)) {
        target_damage = charge_damage_calculate(
            &(ChargeDamageRequest){.moving = mech,
                                   .opponent = target,
                                   .mass_source = mech,
                                   .uses_new_rules = true,
                                   .divisor = 20});
      } else {
        target_damage = (mech_real_tonnage(target) + 5) / 10; /* REUSED! */
      }
      /* Record the damage for debugging then dish it out */
      received_damage = target_damage;
      spread = target_damage / 5;
      for (i = 0; i < spread; i++) {
        hitloc = mech_hit_location(mech, hit_group, &iscritical, &isrear);
        physical_damage_apply_without_experience(mech, mech, 0, -1, hitloc,
                                                 isrear, iscritical, 5, 0);
      }
      if (target_damage % 5) {
        hitloc = mech_hit_location(mech, hit_group, &iscritical, &isrear);
        physical_damage_apply_without_experience(mech, mech, 0, -1, hitloc,
                                                 isrear, iscritical,
                                                 (target_damage % 5), 0);
      }
      /* Stop him */
      mech_current_speed_set(mech, 0);
      mech_desired_speed_set(mech, 0);
      /* Emit the damage for debugging purposes */
      (void)snprintf(emit_buff, LBUF_SIZE,
                     "#%li charges #%li (%i/%i) Distance:"
                     " %.2f DI: %i DR: %i",
                     mech_dbref(mech), mech_dbref(target), mech_base_to_hit,
                     mech_roll, (double)mech_charge_distance(mech),
                     inflicted_damage, received_damage);
      btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "%s", emit_buff);
      /* Make the first unit roll for doing the charge if it is a mech */
      if (mech_class(mech) == CLASS_MECH && !made_pilot_skill_roll(mech, 2)) {
        mech_notify(mech, MECHALL,
                    "Your piloting skill fails and you fall over!!");
        mech_fall(mech, 1, true);
      }
      /* Make the second unit roll for receiving the charge if it is a mech */
      if (mech_class(mech) == CLASS_MECH && !made_pilot_skill_roll(target, 2)) {
        mech_notify(target, MECHALL,
                    "Your piloting skill fails and you fall over!!");
        mech_fall(target, 1, true);
      }
    }
    /* Ok the second unit made its roll */
    if (target_charge && targ_roll >= targ_base_to_hit) {
      /* OUCH */
      mech_printf(mech, MECHALL, "CRASH!!!\n%s charges into you!",
                  mech_to_mech_display_id(mech, target).text);
      mech_notify(target, MECHALL, "SMASH!!! You crash into your target!");
      hit_group = mech_hit_group(target, mech);
      isrear = (hit_group == BACK);
      /* Record the damage for debugging then dish it out */
      inflicted_damage = mech_damage;
      spread = mech_damage / 5;
      for (i = 0; i < spread; i++) {
        hitloc = mech_hit_location(mech, hit_group, &iscritical, &isrear);
        physical_damage_apply(mech, target, 1, mech_pilot_dbref(target), hitloc,
                              isrear, iscritical, 5, 0);
      }
      if (mech_damage % 5) {
        hitloc = mech_hit_location(mech, hit_group, &iscritical, &isrear);
        physical_damage_apply(mech, target, 1, mech_pilot_dbref(target), hitloc,
                              isrear, iscritical, (mech_damage % 5), 0);
      }
      hit_group = mech_hit_group(mech, target);
      isrear = (hit_group == BACK);
      /* Ok now how much damage will the second unit take from
       * charging */
      if (btech_context_uses_new_charge_rules(context) &&
          btech_context_uses_technology_level_three_charge_rules(context)) {
        target_damage = charge_damage_calculate(
            &(ChargeDamageRequest){.moving = target,
                                   .opponent = mech,
                                   .mass_source = mech,
                                   .uses_new_rules = true,
                                   .divisor = 20});
      } else {
        target_damage = (mech_real_tonnage(mech) + 5) / 10; /* REUSED! */
      }
      /* Record the damage for debugging then dish it out */
      received_damage = target_damage;
      spread = target_damage / 5;
      for (i = 0; i < spread; i++) {
        hitloc = mech_hit_location(target, hit_group, &iscritical, &isrear);
        physical_damage_apply_without_experience(target, target, 0, -1, hitloc,
                                                 isrear, iscritical, 5, 0);
      }
      if (mech_damage % 5) {
        hitloc = mech_hit_location(target, hit_group, &iscritical, &isrear);
        physical_damage_apply_without_experience(target, target, 0, -1, hitloc,
                                                 isrear, iscritical,
                                                 (mech_damage % 5), 0);
      }
      /* Stop him */
      mech_current_speed_set(target, 0);
      mech_desired_speed_set(target, 0);
      /* Emit the damage for debugging purposes */
      (void)snprintf(emit_buff, LBUF_SIZE,
                     "#%li charges #%li (%i/%i) Distance:"
                     " %.2f DI: %i DR: %i",
                     mech_dbref(target), mech_dbref(mech), targ_base_to_hit,
                     targ_roll, (double)mech_charge_distance(target),
                     inflicted_damage, received_damage);
      btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "%s", emit_buff);
      if (mech_class(mech) == CLASS_MECH && !made_pilot_skill_roll(mech, 2)) {
        mech_notify(mech, MECHALL,
                    "Your piloting skill fails and you fall over!!");
        mech_fall(mech, 1, true);
      }
      if (mech_class(target) == CLASS_MECH &&
          !made_pilot_skill_roll(target, 2)) {
        mech_notify(target, MECHALL,
                    "Your piloting skill fails and you fall over!!");
        mech_fall(target, 1, true);
      }
    }
    /* Cycle the sections so they can't make another attack for a while */
    if (mech_class(mech) == CLASS_MECH) {
      for (i = 0; i < CHARGE_SECTIONS; i++)
        mech_set_recycle_limb(mech, physical_charge_section(i),
                              PHYSICAL_RECYCLE_TIME);
    } else {
      mech_set_recycle_limb(mech, FSIDE, PHYSICAL_RECYCLE_TIME);
      mech_set_recycle_limb(mech, TURRET, PHYSICAL_RECYCLE_TIME);
    }
    if (mech_class(target) == CLASS_MECH) {
      for (i = 0; i < CHARGE_SECTIONS; i++)
        mech_set_recycle_limb(target, physical_charge_section(i),
                              PHYSICAL_RECYCLE_TIME);
    } else {
      mech_set_recycle_limb(target, FSIDE, PHYSICAL_RECYCLE_TIME);
      mech_set_recycle_limb(target, TURRET, PHYSICAL_RECYCLE_TIME);
    }
    /* mech_charge_target_dbref(mech) and the others are set
       after the return */
    mech_charge_reset(target);
    return;
  }
  /* Check to see if any weapons cycling in any of the sections */
  for (i = 0; i < CHARGE_SECTIONS; i++) {
    if (mech_section_has_recycling_weapon(mech, i)) {
      armor_string_from_index(i, location, mech_class(mech),
                              mech_movement_type(mech));
      mech_printf(mech, MECHALL, "You have weapons recycling on your %s.",
                  location);
      return;
    }
  }
  /* Check if they going fast enough to charge */
  if (mech_current_speed(mech) < MP1) {
    mech_notify(mech, MECHALL, "You aren't moving fast enough to charge.");
    return;
  }
  /* Check to see if their sections cycling */
  if (mech_class(mech) == CLASS_MECH) {
    if (mech_section_recycle_ticks(mech, LLEG) ||
        mech_section_recycle_ticks(mech, RLEG)) {
      mech_notify(mech, MECHALL,
                  "Your legs are still recovering from your last attack.");
      return;
    }
    if (mech_section_recycle_ticks(mech, RARM) ||
        mech_section_recycle_ticks(mech, LARM)) {
      mech_notify(mech, MECHALL,
                  "Your arms are still recovering from your last attack.");
      return;
    }
  } else {
    if (mech_section_recycle_ticks(mech, FSIDE)) {
      mech_notify(mech, MECHALL,
                  "You are still recovering from your last attack!");
      return;
    }
  }
  /* See if either the target or the attacker are jumping */
  if (mech_is_jumping(target)) {
    mech_notify(mech, MECHALL,
                "Your target is jumping, you charge underneath it.");
    return;
  }
  if (mech_is_jumping(mech)) {
    mech_notify(mech, MECHALL,
                "You can't charge while jumping, try death from above.");
    return;
  }
  /* If target is fallen make sure you in a tank */
  if (mech_condition_summary(target).fallen &&
      (mech_class(mech) != CLASS_VEH_GROUND)) {
    mech_notify(mech, MECHALL, "Your target's too low for you to charge it!");
    return;
  }
  /* Only mechs can charge mechs */
  if ((mech_class(mech) == CLASS_MECH) && (mech_class(target) != CLASS_MECH)) {
    mech_notify(mech, MECHALL, "You can only charge mechs!");
    return;
  }
  /* Only tanks can charge tanks and mechs */
  if ((mech_class(mech) == CLASS_VEH_GROUND) &&
      ((mech_class(target) != CLASS_MECH) &&
       (mech_class(target) != CLASS_VEH_GROUND))) {
    mech_notify(mech, MECHALL, "You can only charge mechs and tanks!");
    return;
  }
  /* Check the arc make sure target is in front arc */
  iwa = charge_forward_arc(mech, target);
  if (!(iwa & FORWARDARC)) {
    mech_notify(mech, MECHALL,
                "Your charge target is not in your forward "
                "arc and you are unable to charge it.");
    return;
  }
  /* Damage inflicted by the charge */
  target_damage = charge_damage_calculate(&(ChargeDamageRequest){
      .moving = mech,
      .opponent = target,
      .mass_source = mech,
      .uses_new_rules = btech_context_uses_new_charge_rules(context),
      .divisor = 10,
      .bonus = 1});
  if (has_bool_advantage(context, mech_pilot_dbref(mech), "melee_specialist"))
    target_damage++;
  /* Not enough damage done so no charge */
  if (target_damage <= 0) {
    mech_notify(
        mech, MECHPILOT,
        "Your target pulls away from you and you are unable to charge it.");
    return;
  }
  /* BTH */
  base_to_hit += find_pilot_piloting(mech) - find_s_pilot_piloting(target);
  base_to_hit +=
      (has_bool_advantage(context, mech_pilot_dbref(mech), "melee_specialist")
           ? min(0, mech_attacker_movement_modifier(mech) - 1)
           : mech_attacker_movement_modifier(mech));
  base_to_hit += mech_target_movement_modifier(mech, target, 0.0);
  if (base_to_hit > 12) {
    mech_printf(mech, MECHALL, "Charge: BTH %d\tYou choose not to charge.",
                base_to_hit);
    return;
  }
  /* Roll */
  roll = btech_random_roll(context);
  mech_printf(mech, MECHALL, "Charge: BTH %d\tRoll: %d", base_to_hit, roll);
  /* Did the charge work ? */
  if (roll >= base_to_hit) {
    (void)snprintf(message_buffer, sizeof(message_buffer), "%ss %%s!",
                   mech_class(mech) == CLASS_MECH ? "charge" : "ram");
    /* OUCH */
    mech_los_broadcast_unit(mech, target, message_buffer);
    mech_printf(target, MECHSTARTED, "CRASH!!!\n%s %ss into you!",
                mech_to_mech_display_id(target, mech).text,
                mech_class(mech) == CLASS_MECH ? "charge" : "ram");
    mech_notify(mech, MECHALL, "SMASH!!! You crash into your target!");
    hit_group = mech_hit_group(mech, target);
    if (hit_group == BACK)
      isrear = 1;
    else
      isrear = 0;
    /* Record the damage then dish it out */
    inflicted_damage = target_damage;
    spread = target_damage / 5;
    for (i = 0; i < spread; i++) {
      hitloc = mech_hit_location(target, hit_group, &iscritical, &isrear);
      physical_damage_apply(target, mech, 1, mech_pilot_dbref(mech), hitloc,
                            isrear, iscritical, 5, 0);
    }
    if (target_damage % 5) {
      hitloc = mech_hit_location(target, hit_group, &iscritical, &isrear);
      physical_damage_apply(target, mech, 1, mech_pilot_dbref(mech), hitloc,
                            isrear, iscritical, (target_damage % 5), 0);
    }
    hit_group = mech_hit_group(target, mech);
    isrear = (hit_group == BACK);
    /* Damage done to the attacker for the charge */
    if (btech_context_uses_new_charge_rules(context) &&
        btech_context_uses_technology_level_three_charge_rules(context)) {
      mech_damage =
          charge_damage_calculate(&(ChargeDamageRequest){.moving = mech,
                                                         .opponent = target,
                                                         .mass_source = target,
                                                         .uses_new_rules = true,
                                                         .divisor = 20});
    } else {
      mech_damage = (mech_real_tonnage(target) + 5) / 10;
    }
    /* Record the damage then dish it out */
    received_damage = mech_damage;
    spread = mech_damage / 5;
    for (i = 0; i < spread; i++) {
      hitloc = mech_hit_location(mech, hit_group, &iscritical, &isrear);
      physical_damage_apply_without_experience(mech, mech, 0, -1, hitloc,
                                               isrear, iscritical, 5, 0);
    }
    if (mech_damage % 5) {
      hitloc = mech_hit_location(mech, hit_group, &iscritical, &isrear);
      physical_damage_apply_without_experience(
          mech, mech, 0, -1, hitloc, isrear, iscritical, (mech_damage % 5), 0);
    }
    /* Force piloting roll for attacker if they are in a mech */
    if (mech_class(mech) == CLASS_MECH && !made_pilot_skill_roll(mech, 2)) {
      mech_notify(mech, MECHALL,
                  "Your piloting skill fails and you fall over!!");
      mech_fall(mech, 1, true);
    }
    /* Force piloting roll for target if they are in a mech */
    if (mech_class(target) == CLASS_MECH && !made_pilot_skill_roll(target, 2)) {
      mech_notify(target, MECHSTARTED,
                  "Your piloting skill fails and you fall over!!");
      mech_fall(target, 1, true);
    }
    /* Stop him */
    mech_current_speed_set(mech, 0);
    mech_desired_speed_set(mech, 0);
    /* Emit the damage for debugging purposes */
    (void)snprintf(emit_buff, LBUF_SIZE,
                   "#%li charges #%li (%i/%i) Distance:"
                   " %.2f DI: %i DR: %i",
                   mech_dbref(mech), mech_dbref(target), base_to_hit, roll,
                   (double)mech_charge_distance(mech), inflicted_damage,
                   received_damage);
    btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "%s", emit_buff);
  }
  /* Cycle the sections so they can't make another attack for a while */
  if (mech_class(mech) == CLASS_MECH) {
    for (i = 0; i < CHARGE_SECTIONS; i++)
      mech_set_recycle_limb(mech, physical_charge_section(i),
                            PHYSICAL_RECYCLE_TIME);
  } else {
    mech_set_recycle_limb(mech, FSIDE, PHYSICAL_RECYCLE_TIME);
    mech_set_recycle_limb(mech, TURRET, PHYSICAL_RECYCLE_TIME);
  }
} // end ChargeMech()
/*
 * Checks to see if we can grab a club with our arms.
 */
// NOLINTEND(misc-include-cleaner)
