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
#include "map_units_api.h"
#include "mech_bth_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_condition_api.h"
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
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_progress_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_spot_api.h"
#include "mech_targeting_api.h"
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
#include "section_types.h"
#include "weapon_catalogue_api.h"

static void swap_ints(int *left, int *right) {
  int temporary = *left;
  *left = *right;
  *right = temporary;
}

void FireWeapon(Mech *mech, BattleMap *mech_map, Mech *target, int LOS,
                int weapindx, int weapnum, int section, int critical,
                float enemyX, float enemyY, int mapx, int mapy, float range,
                int indirectFire, int sight, int ishex) {
  Mech *altTarget;
  int baseToHit, RbaseToHit;
  int ammoLoc;
  int ammoCrit;
  int ammoLoc1;
  int ammoCrit1;
  int roll;
  int r1, r2, r3;
  int type = -1, modifier;
  int isarty = (IsArtillery(weapindx));
  int range_ok = 1;
  int wGattlingShots =
      0; /* If we're a gattling MG, then we need to figure out how many shots */
  char buf[SBUF_SIZE] = {0};
  char buf3[SBUF_SIZE] = {0};
  char buf2[LBUF_SIZE] = {0};
  ;
  int wRACHeat = 0;
  int wHGRPSkillMod = 0;
  int tIsSwarmAttack = 0;
  DbRef c3Ref = -1;
  Mech *c3Mech = nullptr;
  int firstCrit = 0;
  int mode = mech_critical_fire_mode(mech, section, critical);
  int wAmmoMode = mech_critical_ammo_mode(mech, section, critical);

  DOCHECKMA((wAmmoMode & STINGER_MODE) && ishex,
            "Stinger missiles cannot shoot hexes!");
  DOCHECKMA((wAmmoMode & STINGER_MODE) && target &&
                !(mech_is_jumping(target) || mech_cocoon_integrity(target) ||
                  (mech_is_flying_type(target) && !mech_is_landed(target))),
            "Stinger missiles can only engage airborne targets!");

  /* If its a coolant gun set to heat, set the target
   * to the mech (ie it shoots itself with coolant gun) */
  if (IsCoolant(weapindx) && mode & HEAT_MODE)
    target = mech;

  DOCHECKMA((mech_section_is_underwater(mech, section) &&
             (MechWeapons[weapindx].shortrange_water <= 0)),
            "This weapon may not be fired underwater.");
  DOCHECKMA(mech_event_count(mech, EVENT_UNSTUN_CREW),
            "You are too stunned to fire a weapon!");
  DOCHECKMA(mech_event_count(mech, EVENT_UNJAM_TURRET),
            "You are too busy unjamming your turret!");
  DOCHECKMA(mech_event_count(mech, EVENT_UNJAM_AMMO),
            "You are too busy unjamming a weapon!");
  DOCHECKMA(mech_event_count(mech, EVENT_REMOVE_PODS),
            "You are too busy removing iNARC pods!");

  DOCHECKMA((mech_swarm_target(mech) > 0) &&
                ((!target) || (mech_swarm_target(mech) != mech_dbref(target))),
            "You're too busy holding on for dear life!");

  if (mech_swarm_target(mech) > 0) {
    if (target && (mech_swarm_target(mech) == mech_dbref(target)))
      tIsSwarmAttack = 1;
  }

  /*
   * Gattling MGs do d6 damage (all as one hit), causing the same in heat
   * and using 3 * damage in ammo.
   */
  if (mech_critical_fire_mode(mech, section, critical) & GATTLING_MODE)
    wGattlingShots = btech_random_range(mech_context(mech), 1, 6);

  /* Find and check Ammunition */
  if (!sight)
    if (!FindAndCheckAmmo(mech, weapindx, section, critical, &ammoLoc,
                          &ammoCrit, &ammoLoc1, &ammoCrit1, &wGattlingShots))
      return;

  /****************************************
   * START: Calc BTH and Roll
   ****************************************/
  if (!isarty) {
    baseToHit = mech_normal_to_hit_calculate(mech, mech_map, section, critical,
                                             weapindx, range, target,
                                             indirectFire, &c3Ref);

    if (c3Ref) {
      c3Mech = btech_context_get_mech(mech_context(mech), c3Ref);

      if (c3Mech && ((mech_team(c3Mech) != mech_team(mech)) ||
                     (c3Ref == mech_dbref(mech)))) {
        c3Mech = nullptr;
      }
    }

  } else
    baseToHit =
        mech_artillery_to_hit_calculate(mech, section, weapindx, !LOS, range);

  /* If it's a swarm attack, make the BTH 0 'cause they always hit */
  if (tIsSwarmAttack)
    baseToHit = 0;

  /* Mod the roll for DFMs and ELRMS */
  if ((MechWeapons[weapindx].special & DFM) ||
      ((MechWeapons[weapindx].special & ELRM) &&
       range < MechWeapons[weapindx].min)) {
    r1 = btech_random_range(mech_context(mech), 1, 6);
    r2 = btech_random_range(mech_context(mech), 1, 6);
    r3 = btech_random_range(mech_context(mech), 1, 6);
    /* Sort 'em to ascending order */
    if (r1 > r2)
      swap_ints(&r1, &r2);
    if (r2 > r3)
      swap_ints(&r2, &r3);
    roll = r1 + r2;
  } else {
    if (target)
      roll = btech_random_roll(mech_context(mech));
    else
      roll = btech_random_roll(mech_context(mech));
  }
  if (LOS)
    snprintf(buf, sizeof(buf), "Roll: %d ", roll);

  /****************************************
   * END: Calc BTH and Roll
   ****************************************/

  /****************************************
   * START: Do all the necessary emits for sighting and firing
   ****************************************/
  if (target && !ishex) {
    range = mech_range_to(mech, target);
    strcpy(buf2, "");
    if (mech_aim_section(mech) != NUM_SECTIONS &&
        mech_aim_unit_class(mech) == mech_class(target) &&
        !IsMissile(weapindx)) {
      ArmorStringFromIndex(mech_aim_section(mech), buf3, mech_class(target),
                           mech_movement_type(target));
      snprintf(buf2, sizeof(buf2), "'s %s", buf3);
    }

    if (sight) {
      DOCHECKMA(baseToHit >= 900,
                tprintf("You aim %s at %s%s - Out of range.",
                        &MechWeapons[weapindx].name[3],
                        mech_to_mech_display_id(mech, target).text, buf2));

      mech_c3_track_emit(mech, c3Ref, c3Mech);

      mech_printf(mech, MECHALL, "You aim %s at %s%s - BTH: %d %s",
                  &MechWeapons[weapindx].name[3],
                  mech_to_mech_display_id(mech, target).text, buf2, baseToHit,
                  mech_condition_summary(target).partial_cover
                      ? "(Partial cover)"
                      : "");
      return;
    }
    if (baseToHit > 12) {
      DOCHECKMA(baseToHit >= 900,
                tprintf("Fire %s at %s%s - Out of range.",
                        &MechWeapons[weapindx].name[3],
                        mech_to_mech_display_id(mech, target).text, buf2));
      mech_printf(
          mech, MECHALL, "Fire %s at %s%s - BTH: %d  Roll: Impossible! %s",
          &MechWeapons[weapindx].name[3],
          mech_to_mech_display_id(mech, target).text, buf2, baseToHit,
          mech_condition_summary(target).partial_cover ? "(Partial cover)"
                                                       : "");
      return;
    }
  } else {
    /* Hex target sight info */
    if (sight) {
      if (baseToHit > 900)
        mech_printf(mech, MECHPILOT,
                    "You aim your %s at (%d,%d) - Out of Range.",
                    &MechWeapons[weapindx].name[3], mapx, mapy);
      else {
        mech_c3_track_emit(mech, c3Ref, c3Mech);

        mech_printf(mech, MECHPILOT, "You aim your %s at (%d,%d) - BTH: %d",
                    &MechWeapons[weapindx].name[3], mapx, mapy, baseToHit);
      }

      return;
    }
    if (!isarty && baseToHit > 12) {
      mech_printf(mech, MECHALL,
                  "Fire %s at (%d,%d) - BTH: %d  Roll: Impossible!",
                  &MechWeapons[weapindx].name[3], mapx, mapy, baseToHit);
      return;
    }
  }

  if (target && !ishex) {
    mech_c3_track_emit(mech, c3Ref, c3Mech);

    mech_printf(
        mech, MECHALL, "You fire %s at %s%s - BTH: %d  %s%s",
        &MechWeapons[weapindx].name[3],
        mech_to_mech_display_id(mech, target).text, buf2, baseToHit, buf,
        mech_condition_summary(target).partial_cover ? "(Partial cover)" : "");

    /* Switching to Exile method of tracking xp, where we split
     * Attacking and Piloting xp into two different channels
     * And since this is neither it goes to its own channel
     */
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ATTACKS, "%s",
                       tprintf("#%li attacks #%li (weapon) (%i/%i)",
                               mech_dbref(mech), mech_dbref(target), baseToHit,
                               roll));
    /*
            btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_XP,
       tprintf("#%i attacks #%i (weapon)
       (%i/%i)", mech->mynum, target->mynum, baseToHit, roll));
    */
    /* If the target has the ATTACKEMIT_MECH flag on have it
     * output this info as well
     */
    if (mech_condition_summary(target).attack_emissions)
      btech_channel_send(
          mech_context(mech), BTECH_CHANNEL_MECH_ATTACK_EMITS, "%s",
          tprintf("#%li attacks #%li (weapon) (%i/%i)", mech_dbref(mech),
                  mech_dbref(target), baseToHit, roll));

  } else {
    mech_c3_track_emit(mech, c3Ref, c3Mech);

    mech_printf(mech, MECHALL, "You fire %s %s (%d,%d) - BTH: %d  %s",
                &MechWeapons[weapindx].name[3],
                mech_hex_target_description(mech), mapx, mapy, baseToHit, buf);

    /* Switching to Exile method of tracking xp, where we split
     * Attacking and Piloting xp into two different channels
     * And since this is neither it goes to its own channel
     */
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ATTACKS, "%s",
        tprintf("#%li attacks %d,%d (%s) (weapon) (%i/%i)", mech_dbref(mech),
                mapx, mapy, mech_hex_target_short_name(mech), baseToHit, roll));
    /*
            btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_XP,
       tprintf("#%i attacks %d,%d (%s) (weapon)
       (%i/%i)", mech->mynum, mapx, mapy, mech_hex_target_short_name(mech),
       baseToHit, roll));
    */

    /* Big Block of code here. Basicly it checks all the targets
     * in the hex the attacker is firing at. For each one that
     * has ATTACKEMIT_MECH set, it broadcasts that info
     */
    {
      Mech *tmpmech;
      int foo;

      for (foo = 0; foo < battle_map_unit_count(mech_map); foo++) {
        DbRef unit_dbref = battle_map_unit_dbref(mech_map, foo);

        if (unit_dbref >= 0) {

          if (!(tmpmech =
                    btech_context_get_mech(mech_context(mech), unit_dbref)))
            continue;
          if (mech_dbref(mech) == mech_dbref(tmpmech))
            continue;
          if (mech_position_x(tmpmech) != mapx &&
              mech_position_y(tmpmech) != mapy)
            continue;
          if (mech_condition_summary(tmpmech).attack_emissions)
            btech_channel_send(
                mech_context(mech), BTECH_CHANNEL_MECH_ATTACK_EMITS, "%s",
                tprintf("#%li attacks %d,%d (%s) (weapon)"
                        " (%i/%i)",
                        mech_dbref(mech), mapx, mapy,
                        mech_hex_target_short_name(mech), baseToHit, roll));
        }
      }
    }
  }

  /****************************************
   * END: Do all the necessary emits for sighting and firing
   ****************************************/

  /* Check for weapon failures */
  if (weapon_failure_stuff(mech, &weapnum, &weapindx, &section, &critical,
                           &ammoLoc, &ammoCrit, &ammoLoc1, &ammoCrit1,
                           &modifier, &type, range, &range_ok, wGattlingShots))
    return;

  /* See if our streaks work */
  if (MechWeapons[weapindx].special & STREAK) {
    if (target && (mech_condition_summary(mech).angel_ecm_disturbed ||
                   mech_condition_summary(target).angel_ecm_protected))
      mech_notify(mech, MECHALL, "The ECM confuses your streak homing system!");
    else if (roll < baseToHit) {
      mech_set_recycle_part(mech, section, critical,
                            WEAPON_TICK * btech_context_weapon_recycle_time(
                                              mech_context(mech), weapindx));
      mech_notify(mech, MECHALL, "Your streak fails to lock on.");
      return;
    }
  }

  /* Hotload LRM jams on 2 or 3 */
  if (mech_critical_fire_mode(mech, section, critical) & HOTLOAD_MODE) {
    if (roll == 2 || roll == 3) {
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]The ammo loading mechanism jams on your %s![reset]",
          &(MechWeapons[weapindx].name[3]));
      mech_critical_temporary_failure_set(mech, section, critical,
                                          FAIL_AMMOJAMMED);
      return;
    }
  }

  /* Caseless jams on a 2. Next internal roll of 8+ explodes */
  if (mech_critical_ammo_mode(mech, section, critical) & AC_CASELESS_MODE) {
    if (roll == 2 || roll == 3) {
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]The ammo loading mechanism jams on your %s![reset]",
          &(MechWeapons[weapindx].name[3]));
      mech_critical_temporary_failure_set(mech, section, critical,
                                          FAIL_AMMOJAMMED);
      /* do 8+ explosion check. Per tac handbook, the launcher explodes on
       * failure*/
      if (btech_random_roll(mech_context(mech)) > 7) {
        /* Rut roh shaggy. Time to cause some damage! */
        mech_printf(mech, MECHALL,
                    "[fg=red bold]Propellant from your %s ignites and "
                    "destroys it![reset]",
                    &(MechWeapons[weapindx].name[3]));
        firstCrit =
            FindFirstWeaponCrit(mech, section, -1, 0, I2Weapon(weapindx),
                                GetWeaponCrits(mech, weapindx));
        mech_weapon_destroy(mech, section, I2Weapon(weapindx), firstCrit,
                            GetWeaponCrits(mech, weapindx),
                            GetWeaponCrits(mech, weapindx));
        mech_los_broadcast(mech, "shudders from an internal explosion!");
        /* Apply damage equal to one shot, follow crits as well */

        DamageMech(mech, mech, 0, -1, section, 0, 1, 0,
                   MechWeapons[weapindx].damage, -1, 0, -1, 0, 1);
        mech_ammunition_decrement(mech, weapindx, section, critical, ammoLoc,
                                  ammoCrit, ammoLoc1, ammoCrit1,
                                  wGattlingShots);
      }

      return;
    }
  }

  /* Check for RFAC explosion/jams */
  if (mech_critical_fire_mode(mech, section, critical) & RFAC_MODE) {
    if (roll == 2) {
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]A catastrophic misload on your %s destroys it and "
          "causes an internal explosion![reset]",
          &(MechWeapons[weapindx].name[3]));
      firstCrit = FindFirstWeaponCrit(mech, section, -1, 0, I2Weapon(weapindx),
                                      GetWeaponCrits(mech, weapindx));
      mech_weapon_destroy(mech, section, I2Weapon(weapindx), firstCrit,
                          GetWeaponCrits(mech, weapindx),
                          GetWeaponCrits(mech, weapindx));
      mech_los_broadcast(mech, "shudders from an internal explosion!");
      DamageMech(mech, mech, 0, -1, section, 0, 0, 0,
                 MechWeapons[weapindx].damage, -1, 0, -1, 0, 1);
      mech_ammunition_decrement(mech, weapindx, section, critical, ammoLoc,
                                ammoCrit, ammoLoc1, ammoCrit1, wGattlingShots);
      return;
    } else if (roll < 5) {
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]The ammo loader mechanism jams on your %s![reset]",
          &(MechWeapons[weapindx].name[3]));
      mech_critical_temporary_failure_set(mech, section, critical,
                                          FAIL_AMMOJAMMED);
      return;
    }
  }

  /* Check for RAC explosion/jams */
  if (MechWeapons[weapindx].special & RAC) {
    if (((mech_critical_fire_mode(mech, section, critical) &
          RAC_TWOSHOT_MODE) &&
         (roll == 2)) ||
        ((mech_critical_fire_mode(mech, section, critical) &
          RAC_FOURSHOT_MODE) &&
         (roll <= 3)) ||
        ((mech_critical_fire_mode(mech, section, critical) &
          RAC_SIXSHOT_MODE) &&
         (roll <= 4))) {
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]The ammo loader mechanism jams on your %s![reset]",
          &(MechWeapons[weapindx].name[3]));
      mech_critical_temporary_failure_set(mech, section, critical,
                                          FAIL_AMMOJAMMED);
      return;
    }
  }

  /* Check for Ultra explosion/jams */
  if (mech_critical_fire_mode(mech, section, critical) & ULTRA_MODE) {
    if (roll == 2) {
      mech_printf(mech, MECHALL, "The loader jams on your %s, destroying it!",
                  &(MechWeapons[weapindx].name[3]));
      firstCrit = FindFirstWeaponCrit(mech, section, -1, 0, I2Weapon(weapindx),
                                      GetWeaponCrits(mech, weapindx));
      mech_weapon_destroy(mech, section, I2Weapon(weapindx), firstCrit,
                          GetWeaponCrits(mech, weapindx),
                          GetWeaponCrits(mech, weapindx));
      return;
    }
  }

  /* See if the sucker will explode from damage taken */
  if (mech_weapon_critical_can_explode(mech, section, critical, roll)) {
    if (IsEnergy(weapindx)) {
      mech_printf(mech, MECHALL,
                  "[fg=red bold]The damaged charging crystal on your %s "
                  "overloads![reset]",
                  &(MechWeapons[weapindx].name[3]));
    } else {
      mech_printf(mech, MECHALL,
                  "[fg=red bold]The damaged ammo feed on your %s triggers an "
                  "internal explosion![reset]",
                  &(MechWeapons[weapindx].name[3]));
      mech_ammunition_decrement(mech, weapindx, section, critical, ammoLoc,
                                ammoCrit, ammoLoc1, ammoCrit1, wGattlingShots);
    }

    mech_los_broadcast(mech, "shudders from an internal explosion!");
    firstCrit = FindFirstWeaponCrit(mech, section, -1, 0, I2Weapon(weapindx),
                                    GetWeaponCrits(mech, weapindx));
    mech_weapon_destroy(mech, section, I2Weapon(weapindx), firstCrit,
                        GetWeaponCrits(mech, weapindx),
                        GetWeaponCrits(mech, weapindx));
    DamageMech(mech, mech, 0, -1, section, 0, 0, 0,
               MechWeapons[weapindx].damage, -1, 0, -1, 0, 1);

    return;
  }

  /* See if the sucker will jam from damage taken */
  if (mech_weapon_critical_can_jam(mech, section, critical, roll)) {
    mech_printf(
        mech, MECHALL,
        "[fg=red bold]The ammo loader mechanism jams on your %s![reset]",
        &(MechWeapons[weapindx].name[3]));
    mech_critical_temporary_failure_set(mech, section, critical,
                                        FAIL_AMMOCRITJAMMED);

    return;
  }

  /* Trash our cocoon if we're OODing */
  if (mech_cocoon_integrity(mech)) {
    if (mech_position_z(mech) > mech_position_surface_elevation(mech)) {
      if (mech_jump_speed(mech) >= MP1) {
        mech_notify(
            mech, MECHALL,
            "You initiate your jumpjets to compensate for the opened cocoon!");
        mech_cocoon_integrity_set(mech, -1);
      } else {
        mech_notify(mech, MECHALL,
                    "Your action splits open the cocoon - have a nice fall!");
        mech_los_broadcast(mech,
                           "starts plummeting down, as the cocoon opens!.");
        mech_cocoon_integrity_set(mech, 0);
        mech_event_cancel(mech, EVENT_OOD);
        mech_event_schedule(mech, EVENT_FALL, mech_fall_event, FALL_TICK, -1);
      }
    }
  }

  /* Better setup some glancing stuff. There will be 3 ways to get hit..
   * Glancing_Blows = 0: NO Glancing. ROLL>=BTH=Normal
   * Glancing_Blows = 1: MaxTech Glancing. ROLL=BTH=Glance, ROLL>BTH=Normal
   * Glancing_Blows = 2: Exile Glancing. ROLL=BTH-1=Glance, ROLL>=BTH=Normal
   * We need to do a little handling here. The rest happens over it
   * mech_hit_resolve
   */
  RbaseToHit = baseToHit;
  if (btech_context_glancing_blow_mode(mech_context(mech)) == 2)
    RbaseToHit = baseToHit - 1; /* only time we modify it */

  if (!isarty) {
    MechFireBroadcast(mech, ishex ? nullptr : target, mapx, mapy, mech_map,
                      &MechWeapons[weapindx].name[3],
                      (roll >= RbaseToHit) && range_ok);
  }
  /* Tell our target they were just shot at... */
  if (target) {
    if (mech_los_check(target, mech, mech_position_x(mech),
                       mech_position_y(mech), range))
      mech_printf(target, MECHALL, "%s has fired a %s at you!",
                  mech_to_mech_display_id(target, mech).text,
                  &MechWeapons[weapindx].name[3]);
    else
      mech_printf(
          target, MECHALL, "Something has fired a %s at you from bearing %d!",
          &MechWeapons[weapindx].name[3],
          FindBearing(mech_position_real_x(target),
                      mech_position_real_y(target), mech_position_real_x(mech),
                      mech_position_real_y(mech)));
  }

  /* Time to decide if we've really hit them and wot to do */
  mech_fired_recently_set(mech, true);

  if (!ishex) /* only record against actual targets */
    mech_shots_fired_increment(mech);

  if (isarty) {
    artillery_shoot(mech, mapx, mapy, weapindx,
                    mech_critical_ammo_mode(mech, section, critical),
                    baseToHit <= roll);
  } else if (range_ok) {
    if (IsMissile(weapindx)) {
      mech_hit_resolve(mech, weapindx, section, critical, target, mapx, mapy,
                       LOS, type, modifier, (roll >= RbaseToHit) && range_ok,
                       baseToHit, wGattlingShots, tIsSwarmAttack, roll);
    } else {
      if (roll >= RbaseToHit) {
        mech_hit_resolve(mech, weapindx, section, critical, target, mapx, mapy,
                         LOS, type, modifier, 1, RbaseToHit, wGattlingShots,
                         tIsSwarmAttack, roll);
      } else {
        int tTryClear = 1;

        if (target) {
          if ((mech_class(target) == CLASS_BSUIT) &&
              (mech_swarm_target(target) > -1) &&
              (altTarget = btech_context_get_mech(mech_context(mech),
                                                  mech_swarm_target(target)))) {

            baseToHit = mech_normal_to_hit_calculate(
                mech, mech_map, section, critical, weapindx, range, altTarget,
                indirectFire, &c3Ref);

            if (roll >= baseToHit) {
              mech_notify(altTarget, MECHALL, "The shot hits you instead!");
              mech_los_broadcast(
                  altTarget, "manages to get in the way of the shot instead!");
              mech_hit_resolve(mech, weapindx, section, critical, altTarget,
                               mapx, mapy, LOS, type, modifier, 1, baseToHit,
                               wGattlingShots, tIsSwarmAttack, roll);

              tTryClear = 0;
            } else {
              if (battle_map_hex_elevation(mech_map, mech_position_x(target),
                                           mech_position_y(target)) <
                  (mech_position_z(target) - 2))
                tTryClear = 0;
            }
          }
        }

        if (tTryClear) {
          int tempDamage = mech_hit_damage_determine(
              mech, section, critical, target, mapx, mapy, weapindx,
              wGattlingShots, MechWeapons[weapindx].damage,
              mech_critical_ammo_mode(mech, section, critical), type, modifier,
              1);

          mech_terrain_possibly_ignite_or_clear(
              mech, weapindx, mech_critical_ammo_mode(mech, section, critical),
              tempDamage, mapx, mapy, 0);
        }
      }
    }
  }

  /* Recycle the weapon */
  mech_set_recycle_part(mech, section, critical,
                        WEAPON_TICK * btech_context_weapon_recycle_time(
                                          mech_context(mech), weapindx));

  /****************************************
   * START: Set the heat after firing
   ****************************************/
  if (type == HEAT)
    mech_weapon_heat_add(mech, (float)modifier);

  if (mech_critical_fire_mode(mech, section, critical) & GATTLING_MODE) {
    mech_weapon_heat_add(mech, (float)wGattlingShots);
  } else if (MechWeapons[weapindx].special & RAC) {
    if (mech_critical_fire_mode(mech, section, critical) & RAC_TWOSHOT_MODE)
      wRACHeat = 2;
    else if (mech_critical_fire_mode(mech, section, critical) &
             RAC_FOURSHOT_MODE)
      wRACHeat = 4;
    else if (mech_critical_fire_mode(mech, section, critical) &
             RAC_SIXSHOT_MODE)
      wRACHeat = 6;
    else
      wRACHeat = 1;

    mech_weapon_heat_add(mech, (float)(MechWeapons[weapindx].heat * wRACHeat));

    if (type == HEAT)
      mech_weapon_heat_add(mech, (float)(modifier * wRACHeat));
  } else {
    mech_weapon_heat_add(mech, (float)MechWeapons[weapindx].heat);

    if (IsEnergy(weapindx))
      mech_weapon_heat_add(mech, (float)mech_weapon_critical_heat_modifier(
                                     mech, section, critical));

    if ((mech_critical_fire_mode(mech, section, critical) & ULTRA_MODE) ||
        (mech_critical_fire_mode(mech, section, critical) & RFAC_MODE)) {

      if (type == HEAT)
        mech_weapon_heat_add(mech, (float)modifier);

      mech_weapon_heat_add(mech, (float)MechWeapons[weapindx].heat);
    }
  }

  /****************************************
   * END: Set the heat after firing
   ****************************************/

  /* Decrement Ammunition */
  mech_ammunition_decrement(mech, weapindx, section, critical, ammoLoc,
                            ammoCrit, ammoLoc1, ammoCrit1, wGattlingShots);

  /* Special for Heavy Gauss Rifles */
  if ((MechWeapons[weapindx].special & HVYGAUSS) &&
      (mech_class(mech) == CLASS_MECH)) {
    if (fabs(mech_current_speed(mech)) > 0.0) {
      mech_notify(mech, MECHALL,
                  "You realize that moving while firing this weapon may not be "
                  "a good idea after all.");
      if (mech_tonnage(mech) <= 35)
        wHGRPSkillMod = 2;
      else if (mech_tonnage(mech) <= 55)
        wHGRPSkillMod = 1;
      else if (mech_tonnage(mech) <= 75)
        wHGRPSkillMod = 0;
      else
        wHGRPSkillMod = -1;
      if (!MadePilotSkillRoll(mech, wHGRPSkillMod)) {
        mech_notify(mech, MECHALL,
                    "The weapon's recoil knocks you to the ground!");
        mech_los_broadcast(mech, tprintf("topples over from the %s's recoil!",
                                         &MechWeapons[weapindx].name[3]));
        mech_fall(mech, 1, 0);
      }
    }
  }
}
