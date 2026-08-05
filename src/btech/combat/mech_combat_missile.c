/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2000 Thomas Wouters
 */

#include "map_terrain.h"
#include "mech_lifecycle.h"
#include <stdio.h>
#include <string.h>

#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_combat.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/support/alloc.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static void swap_ints(int *left, int *right) {
  int temporary = *left;
  *left = *right;
  *right = temporary;
}

void mech_missile_apply_hits(Mech *mech, Mech *target, int hitX, int hitY,
                             int isrear, int iscritical, int weapindx,
                             int fireMode, int ammoMode, int num_missiles_hit,
                             int damage, int salvo_size, int LOS, int bth,
                             int tIsSwarmAttack) {
  int orig_num_missiles = num_missiles_hit;
  int this_time;
  int this_damage;
  int total_damage = 0;
  int clear_damage = 0;
  int hitloc;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char buf[SBUF_SIZE];

  total_damage = num_missiles_hit * damage;

  if (target && btech_context_woods_modify_damage(mech_context(mech)) &&
      battle_terrain_is_forest(map_real_terrain_get(
          mech_map, mech_position_x(target), mech_position_y(target))) &&
      (fireMode > -1) && (ammoMode > -1) &&
      ((mech_position_z(target) - 2) <=
       battle_map_hex_elevation(mech_map, mech_position_x(target),
                                mech_position_y(target)))) {
    clear_damage = total_damage;

    if (map_real_terrain_get(mech_map, mech_position_x(target),
                             mech_position_y(target)) == LIGHT_FOREST)
      total_damage -= 2;
    else if (map_real_terrain_get(mech_map, mech_position_x(target),
                                  mech_position_y(target)) == HEAVY_FOREST)
      total_damage -= 4;

    if (total_damage <= 0)
      num_missiles_hit = 0;
    else
      num_missiles_hit = total_damage / damage;

    mech_terrain_possibly_ignite_or_clear(mech, weapindx, ammoMode,
                                          clear_damage, mech_position_x(target),
                                          mech_position_y(target), 1);

    strcpy(buf, "");

    if (IsMissile(weapindx))
      snprintf(buf, SBUF_SIZE, "%s%s", "missile",
               orig_num_missiles > 1 ? "s" : "");
    else if (ammoMode & LBX_MODE)
      snprintf(buf, SBUF_SIZE, "%s%s", "pellet",
               orig_num_missiles > 1 ? "s" : "");
    else if ((fireMode & ULTRA_MODE) || (fireMode & RFAC_MODE) ||
             (fireMode & RAC_MODES))
      snprintf(buf, SBUF_SIZE, "%s%s", "slug",
               orig_num_missiles > 1 ? "s" : "");
    else
      snprintf(buf, SBUF_SIZE, "%s", "damage");

    mech_printf(mech, MECHALL, "%s %s %s absorbed by the trees!",
                (orig_num_missiles == 1  ? "The"
                 : num_missiles_hit == 0 ? "All of the"
                                         : "Some of the"),
                buf, (orig_num_missiles == 1 ? "is" : "are"));
    mech_printf(target, MECHALL, "The trees absorb %s %s",
                ((orig_num_missiles == 1) || (num_missiles_hit == 0)
                     ? "the"
                     : "some of the"),
                buf);
  }

  while (num_missiles_hit) {
    this_time = MIN(salvo_size, num_missiles_hit);
    this_damage = this_time * damage;

    if (target) {
      hitloc = mech_target_hit_location(mech, target, &isrear, &iscritical);

      DamageMech(target, mech, LOS, mech_gunner_dbref(mech), hitloc, isrear,
                 iscritical,
                 personal_combat_damage_to_unit(target, weapindx, this_damage),
                 0, weapindx, bth, weapindx, ammoMode, tIsSwarmAttack);
    } else {
      mech_terrain_hex_hit(mech, hitX, hitY, weapindx, ammoMode, this_damage,
                           1);
    }

    num_missiles_hit -= this_time;
  }
}

int mech_missile_hit_index(Mech *mech, Mech *hitMech, int weapindx,
                           int wSection, int wCritSlot, int glance) {
  int hit_roll;
  int r1, r2, r3;
  int tHotloading =
      (mech_critical_fire_mode(mech, wSection, wCritSlot) & HOTLOAD_MODE) &&
      (MechWeapons[weapindx].special & IDF);
  int wRollInc = 0;
  int wFinalRoll = 0;
  int tUseArtemisBonus =
      mech_critical_ammo_mode(mech, wSection, wCritSlot) & ARTEMIS_MODE;
  int tUseNARCBonus = 0;

  if (hitMech) {
    MechConditionSummary target_condition = mech_condition_summary(hitMech);
    if (target_condition.ecm_protected ||
        target_condition.angel_ecm_protected) {
      tUseArtemisBonus = 0;
      tUseNARCBonus = 0;
    } else {
      tUseNARCBonus =
          (mech_critical_ammo_mode(mech, wSection, wCritSlot) & NARC_MODE) &&
          (mech_has_section_special(hitMech, NARC_ATTACHED) ||
           mech_has_section_special(hitMech, INARC_HOMING_ATTACHED));
    }
  }

  MechConditionSummary firing_condition = mech_condition_summary(mech);
  if (firing_condition.ecm_disturbed || firing_condition.angel_ecm_disturbed) {
    tUseArtemisBonus = 0;
    tUseNARCBonus = 0;
  }

  /*
   * Figure out the modifiers to the roll table for missiles
   */
  if (IsMissile(weapindx) && (tUseArtemisBonus || tUseNARCBonus))
    wRollInc = 2;

  /* Roll 3 times... if we're hotloading, we'll use the 2 lowest */
  r1 = btech_random_range(mech_context(mech), 1, 6);
  r2 = btech_random_range(mech_context(mech), 1, 6);
  r3 = btech_random_range(mech_context(mech), 1, 6);

  if (r1 > r2)
    swap_ints(&r1, &r2);
  if (r2 > r3)
    swap_ints(&r2, &r3);

  if (tHotloading)
    hit_roll = r1 + r2 - 2;
  else
    hit_roll = btech_random_roll(mech_context(mech)) - 2;

  if ((!hitMech || !mech_condition_summary(hitMech).angel_ecm_protected) &&
      !firing_condition.angel_ecm_disturbed &&
      (MechWeapons[weapindx].special & STREAK)) {
    return 10;
  }

  /* Glancing, per max tech, is -4 off the missile hit table */
  if (glance)
    wRollInc += -4;
  if (wRollInc)
    hit_roll = hit_roll + wRollInc;
  /* Glancing, per max tech, if its lower than 2 on the hit table, we hit with
   * one missile. return -1 so we can test for this elsewhere
   */
  if (glance && (hit_roll < 0))
    return -1;

  wFinalRoll = MAX(MIN(hit_roll, 10), 0);

  return wFinalRoll;
}

int mech_missile_hit_target(Mech *mech, int weapindx, int wSection,
                            int wCritSlot, Mech *hitMech, int hitX, int hitY,
                            int LOS, int baseToHit, int roll, int incoming,
                            int tIsSwarmAttack, int player_roll) {
  int isrear = 0, iscritical = 0;
  int AMStype, ammoLoc, ammoCrit;
  int AMSShotdown = 0;
  int hit;
  int wNARCType = 0;
  int ammoMode = mech_critical_ammo_mode(mech, wSection, wCritSlot);
  int tIsInferno = (ammoMode & INFERNO_MODE);
  int wNARCHitLoc = 0;
  int tIsRear = 0;
  char strLocName[30];
  int missileindex = 0;
  /* Check to see if we're a NARC or iNARC launcher firing homing missiles */
  if (IsMissile(weapindx)) {
    if ((MechWeapons[weapindx].special & NARC) &&
        !(mech_critical_ammo_mode(mech, wSection, wCritSlot) & NARC_MODE))
      wNARCType = 1;
    else if ((MechWeapons[weapindx].special & INARC) &&
             !(mech_critical_ammo_mode(mech, wSection, wCritSlot) &
               INARC_EXPLO_MODE)) {

      if (mech_critical_ammo_mode(mech, wSection, wCritSlot) &
          INARC_HAYWIRE_MODE)
        wNARCType = 3;
      else if (mech_critical_ammo_mode(mech, wSection, wCritSlot) &
               INARC_ECM_MODE)
        wNARCType = 4;
      else
        wNARCType = 2;
    }

    /* Prefill our AMS data */
    if (hitMech && (!((ammoMode & SWARM_MODE) || (ammoMode & SWARM1_MODE) ||
                      (ammoMode & MINE_MODE)))) {
      if (mech_ams_locate_defenses(hitMech, &AMStype, &ammoLoc, &ammoCrit))
        AMSShotdown =
            mech_ams_intercept(mech, hitMech, wNARCType ? 1 : incoming, AMStype,
                               ammoLoc, ammoCrit, LOS, roll >= baseToHit);
    }

    if (wNARCType) {
      if (roll >= baseToHit) {
        if (hitMech) {
          if (AMSShotdown > 0) {
            if (LOS)
              mech_notify(mech, MECHALL, "The pod is shot down by the target!");

            mech_notify(hitMech, MECHALL,
                        "Your Anti-Missile System activates and shoots down "
                        "the incoming pod!");

            return 0;
          }

          wNARCHitLoc = mech_narc_hit_location(mech, hitMech, &tIsRear);

          /* sanity check */
          if (wNARCHitLoc < 0) {
            mech_notify(mech, MECHALL,
                        "Your NARC Beacon attaches to the target!");

            return 0;
          }

          ArmorStringFromIndex(wNARCHitLoc, strLocName, mech_class(hitMech),
                               mech_movement_type(hitMech));

          if (wNARCType == 1)
            mech_section_special_add(hitMech, wNARCHitLoc, NARC_ATTACHED);
          else if (wNARCType == 2)
            mech_section_special_add(hitMech, wNARCHitLoc,
                                     INARC_HOMING_ATTACHED);
          else if (wNARCType == 3) {
            mech_section_special_add(hitMech, wNARCHitLoc,
                                     INARC_HAYWIRE_ATTACHED);

            mech_notify(hitMech, MECHALL,
                        "Your targetting system goes a bit haywire!");
          } else if (wNARCType == 4) {
            mech_section_special_add(hitMech, wNARCHitLoc, INARC_ECM_ATTACHED);

            mech_ecm_check(hitMech);
          }

          mech_printf(hitMech, MECHALL,
                      "A NARC Beacon has been attached to your %s%s!",
                      strLocName, tIsRear == 1 ? " (Rear)" : "");
          mech_printf(mech, MECHALL,
                      "Your NARC Beacon attaches to the target's %s%s!",
                      strLocName, tIsRear == 1 ? " (Rear)" : "");
        }
      } else
        mech_notify(mech, MECHALL,
                    "Your NARC Beacon flies off into the distance.");

      return 0;
    }
  }

  if (roll < baseToHit)
    return incoming;
  if (!btech_context_has_missile_hit_table(mech_context(mech), weapindx))
    return 0;

  missileindex = mech_missile_hit_index(
      mech, hitMech, weapindx, wSection, wCritSlot,
      btech_context_glancing_blows_enabled(mech_context(mech)) &&
              (player_roll == baseToHit)
          ? 1
          : 0);
  if (missileindex < 0)
    hit = MIN(incoming, 1);
  else
    hit = MIN(incoming, btech_context_missile_hit_count(
                            mech_context(mech), weapindx, missileindex));

  if (LOS) {
    mech_printf(mech, MECHALL, "[fg=green]%s with %d missile%s![reset]",
                LOS == 1 ? "You hit" : "The swarm hits", hit,
                hit > 1 ? "s" : "");
  }

  if (AMSShotdown > 0) {
    if (AMSShotdown >= hit) {
      if (LOS)
        mech_notify(mech, MECHALL,
                    "All of your missiles are shot down by the target!");

      mech_notify(hitMech, MECHALL,
                  "Your Anti-Missile System activates and shoots all the "
                  "incoming missiles!");
    } else {
      mech_printf(mech, MECHALL, "The target shoots down %d of your missiles!",
                  AMSShotdown);

      mech_printf(hitMech, MECHALL,
                  "Your Anti-Missile System activates and shoots down %d "
                  "incoming missiles!",
                  AMSShotdown);
    }
  }

  hit = MAX(0, hit - AMSShotdown);

  if (hit <= 0)
    return 0;

  if (tIsInferno) {
    if (hitMech)
      mech_inferno_hit(mech, hitMech, hit, LOS);
    else
      mech_terrain_hex_hit(mech, hitX, hitY, weapindx,
                           mech_critical_ammo_mode(mech, wSection, wCritSlot),
                           0, 0);
  } else {
    if (btech_context_glancing_blows_enabled(mech_context(mech)) &&
        (player_roll == baseToHit) && hitMech) {
      if (!(MechWeapons[weapindx].special & STREAK)) {
        mech_los_broadcast(hitMech, "is nicked by a glancing blow!");
        mech_notify(hitMech, MECHALL, "You are nicked by a glancing blow!");
      }
    }
    mech_missile_apply_hits(
        mech, hitMech, hitX, hitY, isrear, iscritical, weapindx,
        mech_critical_fire_mode(mech, wSection, wCritSlot),
        mech_critical_ammo_mode(mech, wSection, wCritSlot), hit,
        MechWeapons[weapindx].damage, weapon_catalogue_cluster_size(weapindx),
        LOS, baseToHit, tIsSwarmAttack);
  }

  return incoming - hit;
}

void mech_swarm_missile_hit_target(Mech *mech, int weapindx, int wSection,
                                   int wCritSlot, Mech *hitMech, int LOS,
                                   int baseToHit, int roll, int incoming,
                                   int fof, int tIsSwarmAttack,
                                   int player_roll) {
  enum { MAX_STAR = 10 };
  /* Max # of targets we'll try to hit: 10 */
  Mech *star[MAX_STAR];
  int present_target = 0;
  int missiles;
  BattleMap *map =
      btech_context_find_object(mech_context(mech), mech_map_dbref(mech));
  float r = 0.0, ran = 0, flrange = 0.0;
  Mech *source = mech, *tempMech;
  int i, j;
  if (!btech_context_has_missile_hit_table(mech_context(mech), weapindx))
    return;
  missiles = btech_context_missile_hit_count(mech_context(mech), weapindx, 10);
  while (missiles > 0) {
    flrange = flrange + mech_range_to(source, hitMech);
    ran = mech_range_to(mech, hitMech);
    if (flrange > weapon_catalogue_effective_range(
                      weapindx, btech_context_uses_extended_weapon_ranges(
                                    mech_context(mech)))) {
      mech_notify(hitMech, MECHALL, "Luckily, the missiles fall short of you!");
      return;
    }
    if (!(missiles = mech_missile_hit_target(
              mech, weapindx, wSection, wCritSlot, hitMech, -1, -1,
              mech_los_check_unblocked(mech, hitMech, mech_position_x(mech),
                                       mech_position_y(mech), ran)
                  ? present_target == 0 ? 1 : 2
                  : 0,
              baseToHit,
              present_target == 0 ? roll
                                  : btech_random_roll(mech_context(mech)),
              missiles, tIsSwarmAttack, player_roll)))
      return;
    /* Try to acquire a new target NOT in the star */
    if (present_target == MAX_STAR)
      return;
    star[present_target++] = hitMech;
    source = hitMech;
    hitMech = nullptr;
    for (i = 0; i < battle_map_unit_count(map); i++)
      if ((tempMech = btech_context_find_object(mech_context(mech),
                                                battle_map_unit_dbref(map, i))))
        if (!fof || (mech_team(tempMech) != mech_team(mech))) {
          for (j = 0; j < present_target; j++)
            if (tempMech == star[j])
              break;
          if (mech_condition_summary(tempMech).combat_safe)
            continue;
          if (j != present_target)
            continue;
          if (!hitMech && (r = mech_range_to(source, tempMech)) < 1.9)
            if (mech_los_check_unblocked(source, tempMech,
                                         mech_position_x(source),
                                         mech_position_y(source), r)) {
              hitMech = tempMech;
              ran = r;
            }
        }
    if (!hitMech)
      return;
    if (mech != hitMech)
      mech_notify(hitMech, MECHALL, "The missile-swarm turns towards you!");
    if (mech_los_check_unblocked(mech, source, mech_position_x(mech),
                                 mech_position_y(mech),
                                 mech_range_to(mech, source)))
      mech_printf(
          mech, MECHALL, "Your missile-swarm of %d missile%s targets %s!",
          missiles, missiles > 1 ? "s" : "",
          mech == hitMech ? "YOU!!"
                          : mech_to_mech_display_id(mech, hitMech).text);
    mech_los_broadcast_unit(mech, hitMech, "'s missile-swarm targets %s!");
  }
}

/*
 * Fix AMS:
 *
 * - Applied after number missiles is determined
 * - Ammo used == missiles shot down
 * - d6 for IS, 2d6 for clan
 * - Not used against Arrow IV, Thunder, Flare, Swarm or Swarm-1
 */

/****************************************
 * START: AMS related functions
 ****************************************/
int mech_ams_intercept(Mech *mech, Mech *hitMech, int incoming, int type,
                       int ammoLoc, int ammoCrit, int LOS, int missilesDidHit) {
  int num_missiles_shotdown;

  if (MechWeapons[type].special & CLAT)
    num_missiles_shotdown = btech_random_roll(mech_context(mech));
  else
    num_missiles_shotdown = btech_random_range(mech_context(mech), 1, 6);

  if (num_missiles_shotdown > incoming)
    num_missiles_shotdown = incoming;

  if (num_missiles_shotdown >= mech_critical_data(hitMech, ammoLoc, ammoCrit))
    mech_critical_data_set(hitMech, ammoLoc, ammoCrit, 0);
  else
    mech_critical_data_set(hitMech, ammoLoc, ammoCrit,
                           mech_critical_data(hitMech, ammoLoc, ammoCrit) -
                               num_missiles_shotdown);

  if (!missilesDidHit) {
    mech_notify(hitMech, MECHALL,
                "Your Anti-Missile System activates and shoots at the incoming "
                "missiles!");
    return 0;
  }

  return num_missiles_shotdown;
}

int mech_ams_locate_defenses(Mech *target, int *AMStype, int *ammoLoc,
                             int *ammoCrit) {
  int AMSsect, AMScrit;
  int i, j = 0, w, t = 0;

  if (!(mech_technology_flags(target) &
        (IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH)) ||
      !mech_is_started(target) || !mech_condition_summary(target).ams_enabled)
    return 0;

  for (i = 0; i < NUM_SECTIONS; i++) {
    for (j = 0; j < NUM_CRITICALS; j++)
      if (IsWeapon((t = mech_critical_part_type(target, i, j))))
        if (weapon_catalogue_is_anti_missile(Weapon2I(t)))
          if (!(mech_critical_is_nonfunctional(target, i, j) ||
                mech_weapon_is_recycling_at(target, i, j)))
            break;
    if (j < NUM_CRITICALS)
      break;
  }

  if (i == NUM_SECTIONS)
    return 0;

  w = Weapon2I(t);
  AMSsect = i;
  AMScrit = j;
  *AMStype = w;

  if (!(FindAmmoForWeapon(target, w, AMSsect, ammoLoc, ammoCrit)))
    return 0;

  if (!(mech_critical_data(target, *ammoLoc, *ammoCrit)))
    return 0;

  mech_set_recycle_part(
      target, AMSsect, AMScrit,
      btech_context_weapon_recycle_time(mech_context(target), w));
  mech_weapon_heat_add(target, (float)MechWeapons[w].heat);
  return 1;
}

/****************************************
 * END: AMS related functions
 ****************************************/
