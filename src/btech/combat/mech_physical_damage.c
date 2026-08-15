#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_units_api.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_physical.h"
#include "mech_physical_api.h"
#include "mech_physical_internal.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"
#include <stdlib.h>

void physical_damage_apply(Mech *target, Mech *attacker, int cause_pilot,
                           DbRef pilot, int hit_location, int rear,
                           int critical, int damage, int glancing) {
  BtechContext *context = mech_context(target);
  btech_context_damage_experience_mode_set(context, BTECH_DAMAGE_XP_PILOTING);
  mech_damage_apply(&(MechDamageRequest){.target = target,
                                         .attacker = attacker,
                                         .line_of_sight = cause_pilot != 0,
                                         .attack_pilot = pilot,
                                         .hit_location = hit_location,
                                         .rear = rear != 0,
                                         .critical = critical != 0,
                                         .armor_damage = damage,
                                         .internal_damage = glancing,
                                         .transfer = MECH_DAMAGE_NORMAL,
                                         .cause = -1,
                                         .base_to_hit = 0,
                                         .weapon_index = -1,
                                         .ammunition_mode = 0,
                                         .ignore_swarmers = false});
  btech_context_damage_experience_mode_set(context, BTECH_DAMAGE_XP_GUNNERY);
}

void physical_damage_apply_without_experience(Mech *target, Mech *attacker,
                                              int cause_pilot, DbRef pilot,
                                              int hit_location, int rear,
                                              int critical, int damage,
                                              int glancing) {
  BtechContext *context = mech_context(target);
  btech_context_damage_experience_mode_set(context, BTECH_DAMAGE_XP_NONE);
  mech_damage_apply(&(MechDamageRequest){.target = target,
                                         .attacker = attacker,
                                         .line_of_sight = cause_pilot != 0,
                                         .attack_pilot = pilot,
                                         .hit_location = hit_location,
                                         .rear = rear != 0,
                                         .critical = critical != 0,
                                         .armor_damage = damage,
                                         .internal_damage = glancing,
                                         .transfer = MECH_DAMAGE_NORMAL,
                                         .cause = -1,
                                         .base_to_hit = 0,
                                         .weapon_index = -1,
                                         .ammunition_mode = 0,
                                         .ignore_swarmers = false});
  btech_context_damage_experience_mode_set(context, BTECH_DAMAGE_XP_GUNNERY);
}

void physical_trip(Mech *mech, Mech *target) {
  // If we trip our target (who is a mech), make a roll to see if he falls.
  if (!made_pilot_skill_roll(target, 0) &&
      !mech_condition_summary(target).fallen) {

    // Emit to Attacker
    mech_printf(mech, MECHALL, "You trip %s!",
                mech_to_mech_display_id(mech, target).text);

    // Emit to victim and LOS.
    mech_notify(target, MECHSTARTED, "You are tripped and fall to the ground!");
    mech_los_broadcast(target, "trips up and falls down!");

    mech_fall(target, 1, false);
  } else {
    mech_los_broadcast(target, "manages to stay upright!");
  }
} // end PhysicalTrip()

/*
 * Damage the victim.
 */
void physical_damage_resolve(const PhysicalDamageRequest *request) {
  Mech *mech = request->attacker;
  Mech *target = request->target;
  const int WEIGHTDMG = request->weight_divisor;
  const PhysicalAttackType ATTACK_TYPE = request->attack_type;
  const int SECT = request->section;
  const int GLANCE = request->glancing_damage;

  int hitloc = 0;
  int damage;
  int hitgroup = 0;
  int isrear;
  int iscritical;

  isrear = 0;
  iscritical = 0;

  /* Two types of physical attack weapons - Those affected by TSM
   * and those not - Right now just saw but can add more to the list via
   * || (AttackType == PA_BLAH) */
  if (ATTACK_TYPE == PA_SAW) {

    /* Saws do a constant 7 damage due to their mechanical nature. */
    damage = 7;

  } else {

    /* Sword attack uses an odd weapon damage amount */
    if (ATTACK_TYPE == PA_SWORD) {
      damage = ((mech_tonnage(mech) + 5) / WEIGHTDMG) + 1;
    } else {
      /* Round Down to nearest ton -- TW Page 145 */
      damage = mech_tonnage(mech) / WEIGHTDMG;
    }

    /* Calc in affect by TSM */
    if ((mech_excess_heat(mech) >= 9.0F) &&
        (mech_technology_flags(mech) & TRIPLE_MYOMER_TECH)) {
      damage = damage * 2;
    }
  }

  /* If we have melee_specialist, add a point of damage. */
  if (has_bool_advantage(mech_context(mech), mech_pilot_dbref(mech),
                         "melee_specialist")) {
    damage++;
  }

  switch (ATTACK_TYPE) {
  case PA_PUNCH:

    if (!mech_critical_is_operational_special(
            &(CriticalSpecialCheck){.mech = mech,
                                    .slot = {.section = SECT, .critical = 2},
                                    .special = LOWER_ACTUATOR})) {
      damage = damage / 2;
    }

    if (!mech_critical_is_operational_special(
            &(CriticalSpecialCheck){.mech = mech,
                                    .slot = {.section = SECT, .critical = 1},
                                    .special = UPPER_ACTUATOR})) {
      damage = damage / 2;
    }

    hitgroup = mech_hit_group(mech, target);
    if (hitgroup == BACK) {
      isrear = 1;
    }

    if (mech_class(mech) == CLASS_MECH) {

      if (mech_condition_summary(mech).fallen) {

        /* Total Warfare page 151 - Prone mechs can only make
         * two types of physical attacks - Punching (with one arm)
         * vehicles in same hex and thrashing - But for now including
         * this. - Dany 01/2007 */
        if ((mech_class(target) != CLASS_MECH) ||
            (mech_condition_summary(target).fallen &&
             (mech_position_elevation(mech) ==
              mech_position_elevation(target)))) {
          hitloc = mech_target_hit_location(mech, target, &isrear, &iscritical);
        } else if (!mech_condition_summary(target).fallen &&
                   (mech_position_elevation(mech) >
                    mech_position_elevation(target))) {
          hitloc = mech_punch_hit_location(target, hitgroup);
        } else if (mech_position_elevation(mech) ==
                   mech_position_elevation(target)) {
          hitloc = mech_kick_hit_location(target, hitgroup);
        }

      } else if (mech_position_elevation(mech) <
                 mech_position_elevation(target)) {

        if (mech_condition_summary(target).fallen ||
            mech_class(target) != CLASS_MECH) {
          hitloc = mech_target_hit_location(mech, target, &isrear, &iscritical);
        } else {
          hitloc = mech_kick_hit_location(target, hitgroup);
        }

      } else {
        hitloc = mech_punch_hit_location(target, hitgroup);
      }

    } else {
      hitloc = mech_target_hit_location(mech, target, &isrear, &iscritical);
    }

    break;

  case PA_SWORD:
  case PA_AXE:
  case PA_MACE:
  case PA_CLUB:

    hitgroup = mech_hit_group(mech, target);
    if (hitgroup == BACK) {
      isrear = 1;
    }

    if (mech_class(mech) == CLASS_MECH) {

      if (mech_position_elevation(mech) < mech_position_elevation(target)) {

        if (mech_condition_summary(target).fallen ||
            mech_class(target) != CLASS_MECH) {
          hitloc = mech_target_hit_location(mech, target, &isrear, &iscritical);
        } else {
          hitloc = mech_kick_hit_location(target, hitgroup);
        }

      } else if (mech_position_elevation(mech) >
                 mech_position_elevation(target)) {
        hitloc = mech_punch_hit_location(target, hitgroup);
      } else {
        hitloc = mech_target_hit_location(mech, target, &isrear, &iscritical);
      }

    } else {
      hitloc = mech_target_hit_location(mech, target, &isrear, &iscritical);
    }
    break;

  case PA_KICK:

    if (!mech_critical_is_operational_special(
            &(CriticalSpecialCheck){.mech = mech,
                                    .slot = {.section = SECT, .critical = 2},
                                    .special = LOWER_ACTUATOR}))
      damage = damage / 2;

    if (!mech_critical_is_operational_special(
            &(CriticalSpecialCheck){.mech = mech,
                                    .slot = {.section = SECT, .critical = 1},
                                    .special = UPPER_ACTUATOR}))
      damage = damage / 2;

    if (mech_condition_summary(target).fallen ||
        mech_class(target) != CLASS_MECH) {
      hitloc = mech_target_hit_location(mech, target, &isrear, &iscritical);
    } else {

      hitgroup = mech_hit_group(mech, target);
      if (hitgroup == BACK) {
        isrear = 1;
      }

      if (mech_position_elevation(mech) > mech_position_elevation(target)) {
        hitloc = mech_punch_hit_location(target, hitgroup);
      } else {
        hitloc = mech_kick_hit_location(target, hitgroup);
      }
    }
    break;
  case PA_TRIP:
  case PA_SAW:
  case PA_CLAW:
  default:
    break;
  }

  if (GLANCE) {
    damage = (damage + 1) / 2;
  }

  // Damage the target.
  physical_damage_apply(target, mech, 1, mech_pilot_dbref(mech), hitloc, isrear,
                        iscritical, damage, 0);

  // If we've successfully hit a suit, knock him off.
  if (mech_class(target) == CLASS_BSUIT && mech_swarm_target(target) > 0 &&
      ATTACK_TYPE != PA_KICK) {
    bsuit_swarm_stop(target, 0);
  }

  // If we kick our target (who is a mech), make a roll to see if he falls.
  if (mech_class(target) == CLASS_MECH && ATTACK_TYPE == PA_KICK) {
    if (!made_pilot_skill_roll(target, 0) &&
        !mech_condition_summary(target).fallen) {
      mech_notify(target, MECHSTARTED, "The kick knocks you to the ground!");
      mech_los_broadcast(target, "stumbles and falls down!");
      mech_fall(target, 1, false);
    }
  }

} // end PhysicalDamage()

static constexpr int DFA_SECTIONS = 6;
/* Rules make no distinction about Torso not needing recycled  We'll let Head
 * slide for now */

int physical_charge_section(int index) {
  switch (index) {
  case 0:
    return LARM;
  case 1:
    return RARM;
  case 2:
    return LLEG;
  case 3:
    return RLEG;
  case 4:
    return LTORSO;
  case 5:
    return RTORSO;
  default:
    abort();
  }
}

/*
 * Executed at the end of a DFA
 */
bool death_from_above(Mech *mech, Mech *target) {
  int base_to_hit = 5;
  int roll;
  int hit_group;
  int hitloc;
  int isrear = 0;
  int iscritical = 0;
  int target_damage;
  int mech_damage;
  int spread;
  int i;
  int tmpi;
  char location[50];
  BtechContext *context = mech_context(mech);
  BattleMap *map = btech_context_get_map(context, mech_map_dbref(mech));

  /* Weapons recycling check on each major section */
  for (i = 0; i < DFA_SECTIONS; i++) {
    const int SECTION = physical_charge_section(i);
    if (mech_section_has_recycling_weapon(mech, SECTION)) {
      armor_string_from_index(SECTION, location, mech_class(mech),
                              mech_movement_type(mech));
      mech_printf(mech, MECHALL, "You have weapons recycling on your %s.",
                  location);
      return false;
    }
  }
  // Our target is no longer on the map.
  if ((mech_map_dbref(mech) != mech_map_dbref(target))) {
    mech_notify(mech, MECHALL, "Your target is no longer valid.");
    return false;
  }

  if (mech_section_recycle_ticks(mech, LLEG) ||
      mech_section_recycle_ticks(mech, RLEG)) {
    mech_notify(mech, MECHALL,
                "Your legs are still recovering from your last attack.");
    return false;
  }
  if (mech_section_recycle_ticks(mech, RARM) ||
      mech_section_recycle_ticks(mech, LARM)) {
    mech_notify(mech, MECHALL,
                "Your arms are still recovering from your last attack.");
    return false;
  }

  if (mech_is_jumping(target)) {
    mech_notify(mech, MECHALL,
                "Your target is airborne, you cannot land on it.");
    return false;
  }

  if ((mech_class(target) == CLASS_VTOL) ||
      (mech_class(target) == CLASS_AERO) || (mech_class(target) == CLASS_DS)) {
    if (!mech_is_landed(target)) {
      mech_notify(mech, MECHALL,
                  "Your target is airborne, you cannot land on it.");
      return false;
    }
  }

  if ((mech_team(mech) == mech_team(target)) &&
      battle_map_blocks_friendly_fire(map)) {
    mech_notify(mech, MECHALL, "Friendly DFA? I don't think so....");
    return false;
  }
  if (btech_context_physical_attacks_use_pilot_skill(context))
    base_to_hit = find_pilot_piloting(mech);

  base_to_hit +=
      (has_bool_advantage(context, mech_pilot_dbref(mech), "melee_specialist")
           ? min(0, mech_attacker_movement_modifier(mech)) - 1
           : mech_attacker_movement_modifier(mech));
  base_to_hit += mech_target_movement_modifier(mech, target, 0.0);
  base_to_hit += mech_class(target) == CLASS_BSUIT ? 1 : 0;

  if (base_to_hit > 12) {
    mech_printf(
        mech, MECHALL,
        "DFA: BTH %d\tYou choose not to attack and land from your jump.",
        base_to_hit);
    return false;
  }

  roll = btech_random_roll(context);
  mech_printf(mech, MECHALL, "DFA: BTH %d\tRoll: %d", base_to_hit, roll);

  mech_jump_abort(mech);

  if (roll >= base_to_hit) {
    /* OUCH */
    mech_printf(target, MECHSTARTED,
                "DEATH FROM ABOVE!!!\n%s lands on you from above!",
                mech_to_mech_display_id(target, mech).text);
    mech_notify(mech, MECHALL, "You land on your target legs first!");
    mech_los_broadcast_unit(mech, target, "lands on %s!");

    hit_group = mech_hit_group(mech, target);
    if (hit_group == BACK)
      isrear = 1;

    target_damage = (3 * mech_real_tonnage(mech)) / 10;

    if (mech_tonnage(mech) % 10)
      target_damage++;

    if (has_bool_advantage(context, mech_pilot_dbref(mech), "melee_specialist"))
      target_damage++;

    spread = target_damage / 5;

    for (i = 0; i < spread; i++) {
      if (mech_condition_summary(target).fallen ||
          mech_class(target) != CLASS_MECH)
        hitloc = mech_hit_location(target, hit_group, &iscritical, &isrear);
      else
        hitloc = mech_punch_hit_location(target, hit_group);

      physical_damage_apply(target, mech, 1, mech_pilot_dbref(mech), hitloc,
                            isrear, iscritical, 5, 0);
    }

    if (target_damage % 5) {
      if (mech_condition_summary(target).fallen ||
          (mech_class(target) != CLASS_MECH))
        hitloc = mech_hit_location(target, hit_group, &iscritical, &isrear);
      else
        hitloc = mech_punch_hit_location(target, hit_group);

      physical_damage_apply(target, mech, 1, mech_pilot_dbref(mech), hitloc,
                            isrear, iscritical, (target_damage % 5), 0);
    }

    mech_damage = mech_tonnage(mech) / 5;

    spread = mech_damage / 5;

    for (i = 0; i < spread; i++) {
      hitloc = mech_kick_hit_location(mech, FRONT);
      physical_damage_apply_without_experience(mech, mech, 0, -1, hitloc, 0, 0,
                                               5, 0);
    }

    if (mech_damage % 5) {
      hitloc = mech_kick_hit_location(mech, FRONT);
      physical_damage_apply_without_experience(mech, mech, 0, -1, hitloc, 0, 0,
                                               (mech_damage % 5), 0);
    }

    if (!mech_condition_summary(mech).fallen) {
      if (!made_pilot_skill_roll(mech, 4)) {
        mech_notify(mech, MECHALL,
                    "Your piloting skill fails and you fall over!!");
        mech_los_broadcast(mech, "stumbles and falls down!");
        mech_fall(mech, 1, false);
      }
      if (mech_class(target) == CLASS_MECH &&
          !made_pilot_skill_roll(target, 2)) {
        mech_notify(target, MECHSTARTED,
                    "Your piloting skill fails and you fall over!!");
        mech_los_broadcast(target, "stumbles and falls down!");
        mech_fall(target, 1, false);
      }
    }

  } else {
    /* Missed DFA attack */
    if (!mech_condition_summary(mech).fallen) {
      mech_notify(mech, MECHALL,
                  "You miss your DFA attack and fall on your back!!");
      mech_los_broadcast(mech, "misses DFA and falls down!");
    }

    mech_damage = mech_tonnage(mech) / 5;
    spread = mech_damage / 5;

    for (i = 0; i < spread; i++) {
      hitloc = mech_hit_location(mech, BACK, &iscritical, &tmpi);
      physical_damage_apply_without_experience(mech, mech, 0, -1, hitloc, 1,
                                               iscritical, 5, 0);
    }

    if (mech_damage % 5) {
      hitloc = mech_hit_location(mech, BACK, &iscritical, &tmpi);
      physical_damage_apply_without_experience(
          mech, mech, 0, -1, hitloc, 1, iscritical, (mech_damage % 5), 0);
    }

    /* now damage pilot */
    if (!made_pilot_skill_roll(mech, 2)) {
      mech_notify(mech, MECHALL, "You take personal injury from the fall!");
      headhitmwdamage(mech, mech, 1);
    }

    mech_current_speed_set(mech, 0.0);
    mech_desired_speed_set(mech, 0.0);

    mech_make_fall(mech);
    mech_position_z_set(mech, mech_position_elevation(mech));

    if (mech_position_z(mech) < 0)
      mech_flood(mech);
  }

  for (i = 0; i < DFA_SECTIONS; i++)
    mech_set_recycle_limb(mech, physical_charge_section(i),
                          PHYSICAL_RECYCLE_TIME);

  return true;
} // end DeathFromAbove()

/*
 * Executed when we're ready to finish the charge.
 */
