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
#include "legacy_macros.h"
#include "map.h"
#include "map_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_bth_api.h"
#include "mech_build_api.h"
#include "mech_combat.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_spot_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/attrs.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"
#include "weapon_settings.h"

int determineDamageFromHit(Mech *mech, int wSection, int wCritSlot,
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
    fRange = FaMechRange(mech, hitMech);
  else {
    float fx, fy;
    MapCoordToRealCoord(hitX, hitY, &fx, &fy);
    fRange = FindHexRange(MechFX(mech), MechFY(mech), fx, fy);
  }

  /* If our Gattling shots are greater then 0, use that as the damage. */
  if (wGattlingShots > 0)
    wWeapDamage = wGattlingShots;

  /* If we're a heavy gauss rifle, damage gets altered by range. */
  if (MechWeapons[weapindx].special & HVYGAUSS) {
    if (fRange > MechWeapons[weapindx].medrange)
      wWeapDamage = 10;
    else if (fRange > MechWeapons[weapindx].shortrange)
      wWeapDamage = 20;
  }

  /* If we're a snub ppc, damage gets altered by range. */
  if (MechWeapons[weapindx].special & SNUBPPC) {
    if (fRange > MechWeapons[weapindx].medrange)
      wWeapDamage = 5;
    else if (fRange > MechWeapons[weapindx].shortrange)
      wWeapDamage = 8;
  }

  wWeapDamage -= getCritSubDamage(mech, wSection, wCritSlot);

  /* See if we're using flechette ammo */
  if (hitMech) {
    if (wAmmoMode & AC_FLECHETTE_MODE) {
      if (MechType(hitMech) == CLASS_MW) {
        if (mech_real_terrain_get(hitMech) == GRASSLAND)
          wWeapDamage *= 4;
        else
          wWeapDamage *= 2;
      } else if (MechType(hitMech) != CLASS_BSUIT)
        wWeapDamage /= 2;
    }

    if (wAmmoMode & AC_INCENDIARY_MODE) {
      if (MechType(hitMech) == CLASS_MW)
        wWeapDamage += 2;
    }
  }

  /* Check to see if we have an energy weapon and we're modding the damage based
   * on range */
  if (mech->xcode.context->configuration->btech_moddamagewithrange &&
      IsEnergy(weapindx)) {
    if (fRange <= 1.0)
      wWeapDamage++;
    else {
      if (SectionUnderwater(mech, wSection)) {
        if (fRange > MechWeapons[weapindx].longrange_water)
          wWeapDamage = (wWeapDamage / 2);
        else if (fRange > MechWeapons[weapindx].medrange_water)
          wWeapDamage--;
      } else {
        if (fRange > MechWeapons[weapindx].longrange)
          wWeapDamage = (wWeapDamage / 2);
        else if (fRange > MechWeapons[weapindx].medrange)
          wWeapDamage--;
      }
    }
  }

  /* Check to see if we're modding the damage based on woods cover */
  mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  /* If there was a damage type failure, mod the damage */
  if (type == DAMAGE)
    wWeapDamage -= modifier;

  if (hitMech && !isTempCalc) {
    if (mech->xcode.context->configuration->btech_moddamagewithwoods &&
        IsForestHex(mech_map, MechX(hitMech), MechY(hitMech)) &&
        ((MechZ(hitMech) - 2) <=
         Elevation(mech_map, MechX(hitMech), MechY(hitMech)))) {
      wClearDamage = wWeapDamage;

      if (map_real_terrain_get(mech_map, MechX(hitMech), MechY(hitMech)) ==
          LIGHT_FOREST)
        wWeapDamage -= 2;
      else if (map_real_terrain_get(mech_map, MechX(hitMech), MechY(hitMech)) ==
               HEAVY_FOREST)
        wWeapDamage -= 4;

      mech_notify(mech, MECHALL, "The woods absorb some of your shot!");
      mech_notify(hitMech, MECHALL, "The woods absorb some of the damage!");

      mech_terrain_possibly_ignite_or_clear(mech, weapindx, wAmmoMode,
                                            wClearDamage, MechX(hitMech),
                                            MechY(hitMech), 1);
    }
  }

  if (wWeapDamage <= 0)
    wWeapDamage = 1;

  return wWeapDamage;
}

void HitTarget(Mech *mech, int weapindx, int wSection, int wCritSlot,
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
  int wFireMode = GetPartFireMode(mech, wSection, wCritSlot);
  int wAmmoMode = GetPartAmmoMode(mech, wSection, wCritSlot);
  int tIsUltra = ((wFireMode & ULTRA_MODE) || (wFireMode & RFAC_MODE));
  int tIsRAC = (wFireMode & RAC_MODES);
  int tIsLBX = (wAmmoMode & LBX_MODE);
  int tIsSwarm = ((wAmmoMode & SWARM_MODE) || (wAmmoMode & SWARM1_MODE));
  const char *missile_fake_name = nullptr;
  const MissileHitEntry *missile_entry;
  int tUsingTC =
      ((wFireMode & ON_TC) && !IsArtillery(weapindx) && !IsMissile(weapindx) &&
       (!(MechCritStatus(mech) & TC_DESTROYED)) &&
       ((MechAim(mech) != NUM_SECTIONS) && hitMech &&
        (MechAimType(mech) == MechType(hitMech))));
  int missileindex = 0;

  if (hitMech) {

    /* Check to see if we're aiming at a particular location. Swarm attacks
     * can't aim. */
    if ((MechAim(mech) != NUM_SECTIONS) && hitMech && Immobile(hitMech) &&
        !tIsSwarmAttack) {

      roll = btech_random_roll(mech->xcode.context);

      if (roll == 6 || roll == 7 || roll == 8)
        aim_hit = 1;
    }
  }

  if (!IsMissile(weapindx)) {
    wWeapDamage = determineDamageFromHit(
        mech, wSection, wCritSlot, hitMech, hitX, hitY, weapindx,
        wGattlingShots, wBaseWeapDamage, wAmmoMode, type, modifier, 0);

    /* Check if it is a glancing blow, if so, make an emit */
    if ((mech->xcode.context->configuration->btech_glancing_blows) &&
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
  if (!IsArtillery(weapindx) && !IsMissile(weapindx) && !tIsUltra && !tIsLBX &&
      !tIsRAC) {

    if (hitMech) {

      /* Flamers - if in heat mode don't do damage */
      if ((IsFlamer(weapindx)) && (wFireMode & HEAT_MODE)) {

        mech_notify(
            hitMech, MECHALL,
            "[fg=yellow bold]The flaming plasma sprays all over you![reset]");
        mech_notify(
            mech, MECHALL,
            "[fg=green]You cover your target in flaming plasma![reset]");
        MechWeapHeat(hitMech) += (float)wBaseWeapDamage;
        return;

      } else if ((IsCoolant(weapindx)) && (MechType(hitMech) != CLASS_MW)) {

        /* Its a Coolant Gun */
        /* So now we figure out if we want to hit our unit with it
         * or a target */

        if (wFireMode & HEAT_MODE) {

          /* Hit our own unit with the coolant gun */
          mech_notify(mech, MECHALL,
                      "[fg=cyan]Coolant washes over your systems!![reset]");
          MechWeapHeat(mech) -= (float)wBaseWeapDamage;

        } else {

          /* Hit the target with the coolant gun */
          mech_notify(mech, MECHALL,
                      "[fg=cyan]You hit with the stream of coolant!![reset]");
          mech_notify(hitMech, MECHALL,
                      "[fg=cyan]Coolant washes over your systems!![reset]");
          MechWeapHeat(hitMech) -= (float)wBaseWeapDamage;
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

      DamageMech(hitMech, mech, LOS, GunPilot(mech), hitloc, isrear, iscritical,
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
    if (GetPartFireMode(mech, wSection, wCritSlot) & RAC_TWOSHOT_MODE)
      missile_fake_name = "IS.SRM-2";
    else if (GetPartFireMode(mech, wSection, wCritSlot) & RAC_FOURSHOT_MODE)
      missile_fake_name = "IS.SRM-4";
    else if (GetPartFireMode(mech, wSection, wCritSlot) & RAC_SIXSHOT_MODE)
      missile_fake_name = "IS.SRM-6";
    missile_entry = missile_hit_registry_find_name(
        &mech->xcode.context->missile_hits, missile_fake_name);
  } else
    missile_entry = missile_hit_registry_find_weapon(
        &mech->xcode.context->missile_hits, weapindx);
  if (missile_entry == nullptr)
    return;

  if (IsMissile(weapindx)) {
    if (player_roll < bth) {
      return;
    } else

        if (tIsSwarm && hitMech) /* No swarms on hex hits */
      SwarmHitTarget(mech, weapindx, wSection, wCritSlot, hitMech, LOS, bth,
                     reallyhit ? bth + 1 : bth - 1,
                     (type == CRAZY_MISSILES)
                         ? missile_entry->num_missiles[10] * modifier / 100
                         : missile_entry->num_missiles[10],
                     (GetPartAmmoMode(mech, wSection, wCritSlot) & SWARM1_MODE),
                     tIsSwarmAttack, player_roll);
    else
      MissileHitTarget(mech, weapindx, wSection, wCritSlot, hitMech, hitX, hitY,
                       LOS ? 1 : 0, bth, reallyhit ? bth + 1 : bth - 1,
                       (type == CRAZY_MISSILES)
                           ? missile_entry->num_missiles[10] * modifier / 100
                           : missile_entry->num_missiles[10],
                       tIsSwarmAttack, player_roll);

    return;
  }

  missileindex = MissileHitIndex(
      mech, hitMech, weapindx, wSection, wCritSlot,
      (mech->xcode.context->configuration->btech_glancing_blows) &&
              (player_roll == bth)
          ? 1
          : 0);
  /* This is how we'll handle glancing. Any roll < 2 is considering just one
   * missile hit, full damage */
  if (missileindex == -1)
    num_missiles_hit = 1;
  else
    num_missiles_hit = missile_entry->num_missiles[missileindex];

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
    Missile_Hit(
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
            hitMech, mech, LOS, GunPilot(mech), hitloc, isrear, iscritical,
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
