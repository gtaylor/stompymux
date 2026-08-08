/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997-2002 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "artillery_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "failures.h"
#include "failures_api.h"
#include "map.h"
#include "map_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_bth_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_heat_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_spot_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/attrs.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include "weapon_settings.h"

int mech_hit_damage_determine(Mech *mech, int wSection, int wCritSlot,
                              Mech *hitMech, int hitX, int hitY, int weapindx,
                              int wGattlingShots, int wBaseWeapDamage,
                              int wAmmoMode, int type, int modifier,
                              int isTempCalc) {
  BattleMap *mech_map;
  float fRange = 0.0;
  int wWeapDamage = wBaseWeapDamage;
  int wClearDamage = 0;

  /* Find the range to our target */
  if (hitMech)
    fRange = mech_range_to(mech, hitMech);
  else {
    float fx, fy;
    MapCoordToRealCoord(hitX, hitY, &fx, &fy);
    fRange = FindHexRange(mech_position_real_x(mech),
                          mech_position_real_y(mech), fx, fy);
  }

  /* If our Gattling shots are greater then 0, use that as the damage. */
  if (wGattlingShots > 0)
    wWeapDamage = wGattlingShots;

  /* If we're a heavy gauss rifle, damage gets altered by range. */
  if (MechWeapons[weapindx].special & HVYGAUSS) {
    if (fRange > (float)MechWeapons[weapindx].medrange)
      wWeapDamage = 10;
    else if (fRange > (float)MechWeapons[weapindx].shortrange)
      wWeapDamage = 20;
  }

  /* If we're a snub ppc, damage gets altered by range. */
  if (MechWeapons[weapindx].special & SNUBPPC) {
    if (fRange > (float)MechWeapons[weapindx].medrange)
      wWeapDamage = 5;
    else if (fRange > (float)MechWeapons[weapindx].shortrange)
      wWeapDamage = 8;
  }

  wWeapDamage -= mech_weapon_critical_damage_penalty(mech, wSection, wCritSlot);

  /* See if we're using flechette ammo */
  if (hitMech) {
    if (wAmmoMode & AC_FLECHETTE_MODE) {
      if (mech_class(hitMech) == CLASS_MW) {
        if (mech_real_terrain_get(hitMech) == GRASSLAND)
          wWeapDamage *= 4;
        else
          wWeapDamage *= 2;
      } else if (mech_class(hitMech) != CLASS_BSUIT)
        wWeapDamage /= 2;
    }

    if (wAmmoMode & AC_INCENDIARY_MODE) {
      if (mech_class(hitMech) == CLASS_MW)
        wWeapDamage += 2;
    }
  }

  /* Check to see if we have an energy weapon and we're modding the damage based
   * on range */
  if (btech_context_range_modifies_damage(mech_context(mech)) &&
      weapon_catalogue_is_energy(weapindx)) {
    if (fRange <= 1.0F)
      wWeapDamage++;
    else {
      if (mech_section_is_underwater(mech, wSection)) {
        if (fRange > (float)MechWeapons[weapindx].longrange_water)
          wWeapDamage = (wWeapDamage / 2);
        else if (fRange > (float)MechWeapons[weapindx].medrange_water)
          wWeapDamage--;
      } else {
        if (fRange > (float)MechWeapons[weapindx].longrange)
          wWeapDamage = (wWeapDamage / 2);
        else if (fRange > (float)MechWeapons[weapindx].medrange)
          wWeapDamage--;
      }
    }
  }

  /* Check to see if we're modding the damage based on woods cover */
  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  /* If there was a damage type failure, mod the damage */
  if (type == DAMAGE)
    wWeapDamage -= modifier;

  if (hitMech && !isTempCalc) {
    if (btech_context_woods_modify_damage(mech_context(mech)) &&
        battle_terrain_is_forest(map_real_terrain_get(
            mech_map, mech_position_x(hitMech), mech_position_y(hitMech))) &&
        ((mech_position_z(hitMech) - 2) <=
         battle_map_hex_elevation(mech_map, mech_position_x(hitMech),
                                  mech_position_y(hitMech)))) {
      wClearDamage = wWeapDamage;

      if (map_real_terrain_get(mech_map, mech_position_x(hitMech),
                               mech_position_y(hitMech)) == LIGHT_FOREST)
        wWeapDamage -= 2;
      else if (map_real_terrain_get(mech_map, mech_position_x(hitMech),
                                    mech_position_y(hitMech)) == HEAVY_FOREST)
        wWeapDamage -= 4;

      mech_notify(mech, MECHALL, "The woods absorb some of your shot!");
      mech_notify(hitMech, MECHALL, "The woods absorb some of the damage!");

      mech_terrain_possibly_ignite_or_clear(
          mech, weapindx, wAmmoMode, wClearDamage, mech_position_x(hitMech),
          mech_position_y(hitMech), 1);
    }
  }

  if (wWeapDamage <= 0)
    wWeapDamage = 1;

  return wWeapDamage;
}

static int missile_hit_count(const Mech *mech, int weapon_index,
                             const char *fake_name, bool uses_fake_name,
                             int roll_index) {
  BtechContext *context = mech_context(mech);
  return uses_fake_name ? btech_context_missile_hit_count_by_name(
                              context, fake_name, roll_index)
                        : btech_context_missile_hit_count(context, weapon_index,
                                                          roll_index);
}

void mech_hit_resolve(Mech *mech, int weapindx, int wSection, int wCritSlot,
                      Mech *hitMech, int hitX, int hitY, int LOS, int type,
                      int modifier, int reallyhit, int bth, int wGattlingShots,
                      int tIsSwarmAttack, int player_roll) {
  int isrear = 0, iscritical = 0;
  int hitloc = 0;
  int roll;
  int aim_hit = 0;
  int wBaseWeapDamage = MechWeapons[weapindx].damage;
  int wWeapDamage = 0;
  int num_missiles_hit;
  int wFireMode = mech_critical_fire_mode(mech, wSection, wCritSlot);
  int wAmmoMode = mech_critical_ammo_mode(mech, wSection, wCritSlot);
  int tIsUltra = ((wFireMode & ULTRA_MODE) || (wFireMode & RFAC_MODE));
  int tIsRAC = (wFireMode & RAC_MODES);
  int tIsLBX = (wAmmoMode & LBX_MODE);
  int tIsSwarm = ((wAmmoMode & SWARM_MODE) || (wAmmoMode & SWARM1_MODE));
  const char *missile_fake_name = nullptr;
  int maximum_missile_hits;
  int tUsingTC =
      ((wFireMode & ON_TC) && !weapon_catalogue_is_artillery(weapindx) &&
       !weapon_catalogue_is_missile(weapindx) &&
       !mech_condition_summary(mech).targeting_computer_destroyed &&
       ((mech_aim_section(mech) != NUM_SECTIONS) && hitMech &&
        (mech_aim_unit_class(mech) == mech_class(hitMech))));
  int missileindex = 0;

  if (hitMech) {

    /* Check to see if we're aiming at a particular location. Swarm attacks
     * can't aim. */
    if ((mech_aim_section(mech) != NUM_SECTIONS) && hitMech &&
        mech_is_immobile(hitMech) && !tIsSwarmAttack) {

      roll = btech_random_roll(mech_context(mech));

      if (roll == 6 || roll == 7 || roll == 8)
        aim_hit = 1;
    }
  }

  if (!weapon_catalogue_is_missile(weapindx)) {
    wWeapDamage = mech_hit_damage_determine(
        mech, wSection, wCritSlot, hitMech, hitX, hitY, weapindx,
        wGattlingShots, wBaseWeapDamage, wAmmoMode, type, modifier, 0);

    /* Check if it is a glancing blow, if so, make an emit */
    if (btech_context_glancing_blows_enabled(mech_context(mech)) &&
        (player_roll == bth) && hitMech) {
      /* Yes, even though we have two different glance modes, the above is
       * correct because we modified the bth in FireWeapon. Nothing to see here.
       * move along
       */
      mech_los_broadcast(hitMech, "is nicked by a glancing blow!");
      mech_notify(hitMech, MECHALL, "You are nicked by a glancing blow!");
      wWeapDamage = (int)(wWeapDamage + 1) / 2;
      if (wWeapDamage < 1)
        wWeapDamage = 1; /* very rare case */
    }
  }

  /*
   * Ok, if we're not an artillery weapon or missile and we're not in
   * LBX, RAC, Ultra or RFAC mode...
   */
  if (!weapon_catalogue_is_artillery(weapindx) &&
      !weapon_catalogue_is_missile(weapindx) && !tIsUltra && !tIsLBX &&
      !tIsRAC) {

    if (hitMech) {

      /* Flamers - if in heat mode don't do damage */
      if ((weapon_catalogue_is_flamer(weapindx)) && (wFireMode & HEAT_MODE)) {

        mech_notify(
            hitMech, MECHALL,
            "[fg=yellow bold]The flaming plasma sprays all over you![reset]");
        mech_notify(
            mech, MECHALL,
            "[fg=green]You cover your target in flaming plasma![reset]");
        mech_weapon_heat_add(hitMech, (float)wBaseWeapDamage);
        return;

      } else if ((weapon_catalogue_is_coolant(weapindx)) &&
                 (mech_class(hitMech) != CLASS_MW)) {

        /* Its a Coolant Gun */
        /* So now we figure out if we want to hit our unit with it
         * or a target */

        if (wFireMode & HEAT_MODE) {

          /* Hit our own unit with the coolant gun */
          mech_notify(mech, MECHALL,
                      "[fg=cyan]Coolant washes over your systems!![reset]");
          mech_weapon_heat_add(mech, -(float)wBaseWeapDamage);

        } else {

          /* Hit the target with the coolant gun */
          mech_notify(mech, MECHALL,
                      "[fg=cyan]You hit with the stream of coolant!![reset]");
          mech_notify(hitMech, MECHALL,
                      "[fg=cyan]Coolant washes over your systems!![reset]");
          mech_weapon_heat_add(hitMech, -(float)wBaseWeapDamage);
        }

        /* Never does damage so return */
        return;
      }

      if (aim_hit)
        hitloc = mech_aimed_hit_location(mech, hitMech, &isrear, &iscritical);
      else if (tUsingTC)
        hitloc = mech_targeting_computer_hit_location(mech, hitMech, &isrear,
                                                      &iscritical);
      else
        hitloc = mech_target_hit_location(mech, hitMech, &isrear, &iscritical);

      DamageMech(hitMech, mech, LOS, mech_gunner_dbref(mech), hitloc, isrear,
                 iscritical,
                 personal_combat_damage_to_unit(hitMech, weapindx, wWeapDamage),
                 0, weapindx, bth, weapindx, wAmmoMode, tIsSwarmAttack);

    } else {
      mech_terrain_hex_hit(mech, hitX, hitY, weapindx, wAmmoMode, wWeapDamage,
                           1);
    }

    return;
  }

  /*
   * Since we're here, we're either
   *      - A missile weapon
   *      - An artillery weapon
   *      - An AC in Ultra, RF or LBX mode
   *      - A RAC in RAC mode
   */

  /*
   * Do special case for RACs since they don't have an entry in the
   * missile cluster registry.
   *
   * We're gonna fake it by pretending we're either an SRM-2, SRM-4 or SRM-6,
   * depending upon the mode
   */
  if (tIsRAC) {
    if (mech_critical_fire_mode(mech, wSection, wCritSlot) & RAC_TWOSHOT_MODE)
      missile_fake_name = "IS.SRM-2";
    else if (mech_critical_fire_mode(mech, wSection, wCritSlot) &
             RAC_FOURSHOT_MODE)
      missile_fake_name = "IS.SRM-4";
    else if (mech_critical_fire_mode(mech, wSection, wCritSlot) &
             RAC_SIXSHOT_MODE)
      missile_fake_name = "IS.SRM-6";
  }
  maximum_missile_hits =
      missile_hit_count(mech, weapindx, missile_fake_name, tIsRAC, 10);
  if (maximum_missile_hits == 0)
    return;

  if (weapon_catalogue_is_missile(weapindx)) {
    if (player_roll < bth) {
      return;
    } else

        if (tIsSwarm && hitMech) /* No swarms on hex hits */
      mech_swarm_missile_hit_target(
          mech, weapindx, wSection, wCritSlot, hitMech, LOS, bth,
          reallyhit ? bth + 1 : bth - 1,
          (type == CRAZY_MISSILES) ? maximum_missile_hits * modifier / 100
                                   : maximum_missile_hits,
          (mech_critical_ammo_mode(mech, wSection, wCritSlot) & SWARM1_MODE),
          tIsSwarmAttack, player_roll);
    else
      mech_missile_hit_target(
          mech, weapindx, wSection, wCritSlot, hitMech, hitX, hitY, LOS ? 1 : 0,
          bth, reallyhit ? bth + 1 : bth - 1,
          (type == CRAZY_MISSILES) ? maximum_missile_hits * modifier / 100
                                   : maximum_missile_hits,
          tIsSwarmAttack, player_roll);

    return;
  }

  missileindex = mech_missile_hit_index(
      mech, hitMech, weapindx, wSection, wCritSlot,
      btech_context_glancing_blows_enabled(mech_context(mech)) &&
              (player_roll == bth)
          ? 1
          : 0);
  /* This is how we'll handle glancing. Any roll < 2 is considering just one
   * missile hit, full damage */
  if (missileindex == -1)
    num_missiles_hit = 1;
  else
    num_missiles_hit = missile_hit_count(mech, weapindx, missile_fake_name,
                                         tIsRAC, missileindex);

  /*
   * Check for non-missile, multiple hit weapons, like LBXs, RACs, RFACs and
   * Ultras
   */
  if (LOS)
    mech_printf(mech, MECHALL, "[fg=green]You hit with %d %s%s![reset]",
                num_missiles_hit,
                (tIsUltra || tIsRAC ? "slug"
                 : tIsLBX           ? "pellet"
                                    : "missile"),
                (num_missiles_hit > 1 ? "s" : ""));

  if (tIsLBX)
    mech_missile_apply_hits(
        mech, hitMech, hitX, hitY, isrear, iscritical, weapindx, wFireMode,
        wAmmoMode, num_missiles_hit, tIsLBX ? 1 : wWeapDamage,
        weapon_catalogue_cluster_size(weapindx), LOS, bth, tIsSwarmAttack);
  else {
    while (num_missiles_hit) {
      if (hitMech) {
        if (aim_hit)
          hitloc = mech_aimed_hit_location(mech, hitMech, &isrear, &iscritical);
        if (tUsingTC)
          hitloc = mech_targeting_computer_hit_location(mech, hitMech, &isrear,
                                                        &iscritical);
        else
          hitloc =
              mech_target_hit_location(mech, hitMech, &isrear, &iscritical);
        DamageMech(
            hitMech, mech, LOS, mech_gunner_dbref(mech), hitloc, isrear,
            iscritical,
            personal_combat_damage_to_unit(hitMech, weapindx, wWeapDamage), 0,
            weapindx, bth, weapindx, wAmmoMode, tIsSwarmAttack);
      } else
        mech_terrain_hex_hit(mech, hitX, hitY, weapindx, wAmmoMode, wWeapDamage,
                             1);

      num_missiles_hit--;
    }
  }
}

/****************************************
 * Start: Hex hitting related functions
 ****************************************/
