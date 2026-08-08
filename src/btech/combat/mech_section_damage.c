#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2000 Thomas Wouters
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "eject_api.h"
#include "environment_damage_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_ammodump_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_pickup_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "section_types.h"
void mech_weapon_destroy(Mech *wounded, int hitloc, int type, int startCrit,
                         int numcrits, int totalcrits) {
  int i;
  char sum = totalcrits;
  char destroyed = numcrits;
  int checkloc;
  int newcrit;
  int split;
  int disable = 0; // Hax for later destroying all crits or disabling

  for (i = startCrit; i < NUM_CRITICALS; i++) {
    if (mech_critical_part_type(wounded, hitloc, i) == type) {
      if (mech_critical_is_damaged(wounded, hitloc, i)) {
        if (disable)
          mech_critical_fire_mode_add(wounded, hitloc, i, DISABLED_MODE);
        else
          mech_critical_destroy(wounded, hitloc, i);
      } else if (destroyed) {
        if (disable)
          mech_critical_fire_mode_add(wounded, hitloc, i, DISABLED_MODE);
        else
          mech_critical_destroy(wounded, hitloc, i);
        destroyed--;
      } else {
        mech_critical_fire_mode_add(wounded, hitloc, i, BROKEN_MODE);
      }
      sum--;
      //                      if(sum == totalcrits)
      if (!sum)
        return;
    }
  }
  // if we've gotten here, then sum != total crits, but we've run outta crits in
  // this location, so it must be a split crit.
  if (mech_class(wounded) != CLASS_MECH)
    return; // sanity check
  if (GetSplitData(wounded, hitloc, startCrit, &checkloc, &newcrit, &split))
    mech_weapon_destroy(wounded, checkloc, split, newcrit, destroyed, sum);
}

int mech_weapon_count_in_section(Mech *mech, int loc) {
  int i;
  int j, sec, cri;
  int count = 0;

  j = FindWeaponNumberOnMech(mech, 1, &sec, &cri);
  for (i = 2; j != -1; i++) {
    if (sec == loc)
      count++;
    j = FindWeaponNumberOnMech(mech, i, &sec, &cri);
  }
  return count;
}

int mech_weapon_index_in_section(Mech *mech, int loc, int num) {
  int i;
  int j, sec, cri;
  int count = 0;

  j = FindWeaponNumberOnMech(mech, 1, &sec, &cri);
  for (i = 2; j != -1; i++) {
    if (sec == loc) {
      count++;
      if (count == num)
        return j;
    }
    j = FindWeaponNumberOnMech(mech, i, &sec, &cri);
  }
  return -1;
}

void mech_weapon_destroy_random(Mech *mech, int hitloc) {
  /* Look for hit locations.. */
  int i = mech_weapon_count_in_section(mech, hitloc);
  int a, b;
  int firstCrit;

  if (!i)
    return;
  a = btech_random_range(mech_context(mech), 1, i);
  b = mech_weapon_index_in_section(mech, hitloc, a);
  if (b < 0)
    return;

  firstCrit = FindFirstWeaponCrit(
      mech, hitloc, -1, 0, weapon_equipment_index(b), GetWeaponCrits(mech, b));

  mech_weapon_destroy(mech, hitloc, weapon_equipment_index(b), firstCrit, 1,
                      GetWeaponCrits(mech, b));
  mech_printf(mech, MECHALL, "[fg=red bold]Your %s is destroyed![reset]",
              &MechWeapons[b].name[3]);
}

void mech_heat_sink_destroy(Mech *mech, int hitloc) {
  /* This can be done easily, or this can be done painfully. */
  /* Let's try the painful way, it's more fun that way. */
  int num;
  int i = special_equipment_index(HEAT_SINK);

  if (FindObj(mech, hitloc, i)) {
    num = mech_heat_sink_critical_size(mech);
    mech_weapon_destroy(mech, hitloc, i, 0, 1, num);
    mech_heat_sink_count_remove(mech, MAX(num, 2));
    mech_notify(mech, MECHALL,
                "The computer shows a heatsink died due to the impact.");
  }
}

void mech_section_destroy(Mech *wounded, Mech *attacker, int LOS, int hitloc) {
  char locname[30] = {0};
  char msgbuf[MBUF_SIZE] = {0};
  int i;
  int tKillMech;
  int tIsLeg = ((hitloc == RLEG || hitloc == LLEG) ||
                ((hitloc == RARM || hitloc == LARM) && mech_is_quad(wounded)));
  DbRef wounded_pilot = mech_pilot_dbref(wounded);
  Mech *ttarget;

  /* Prevent the rare occurance of a section getting destroyed twice */
  if (mech_section_is_destroyed(wounded, hitloc)) {
    fprintf(stderr, "Double-desting section %d on mech #%ld\n", hitloc,
            mech_dbref(wounded));
    if (mech_is_dropship(wounded))
      return;
    for (i = 0; i < NUM_SECTIONS; i++)
      if (mech_section_original_internal(wounded, i) &&
          mech_section_internal(wounded, i))
        return;
    if (btech_context_event_data_count(mech_context(wounded), EVENT_NUKEMECH,
                                       (intptr_t)wounded)) {
      fprintf(stderr, "And nuke event already existed.\n");
      return;
    }
    discard_mw(wounded);
    return;
  }
  /* Ouch. They got toasted */
  mech_section_armor_set(wounded, hitloc, 0);
  mech_section_internal_set(wounded, hitloc, 0);
  mech_section_rear_armor_set(wounded, hitloc, 0);
  mech_section_specials_clear(wounded, hitloc);

  /* uncycle the section <in the case of an arm/leg that was kicking getting
   * blown */
  mech_set_recycle_limb(wounded, hitloc, 0);

  /* drop off what we were carrying, since we really can't pick it up with one
   * arm */
  if ((hitloc == RARM || hitloc == LARM)) {
    if (mech_carried_dbref(wounded) > 0) {
      if ((ttarget = btech_context_get_mech(mech_context(wounded),
                                            mech_carried_dbref(wounded)))) {
        mech_notify(ttarget, MECHALL, "Your tow lines go suddenly slack!");
        mech_dropoff(GOD, wounded, "");
      }
    }
  }

  /* Tell the attacker about it... */
  if (attacker) {
    ArmorStringFromIndex(hitloc, locname, mech_class(wounded),
                         mech_movement_type(wounded));
    if (LOS >= 0)
      mech_printf(wounded, MECHALL, "Your %s has been destroyed!", locname);
    snprintf(msgbuf, sizeof(msgbuf), "'s %s has been destroyed!", locname);
    mech_los_broadcast(wounded, msgbuf);
  }

  /* Destroy everything in the loc */
  mech_parts_destroy(attacker, wounded, hitloc, 0, 0);
  mech_ecm_check(wounded);
  /* Stop lateral if we're a quad */
  if (mech_class(wounded) == CLASS_MECH && mech_is_quad(wounded))
    if (mech_lateral_movement(wounded) && tIsLeg)
      mech_lateral_movement_set(wounded, 0);
  /* Check to see if we should destroy the unit */
  if (mech_class(wounded) == CLASS_BSUIT) {
    if (bsuit_member_count(wounded) > 0)
      goto skip_nuke;
    else if (!mech_is_destroyed(wounded))
      mech_destroy(wounded, attacker, 1, KILL_TYPE_NORMAL);
  } else {
    for (i = 0; i < NUM_SECTIONS; i++)
      if (mech_section_original_internal(wounded, i) &&
          mech_section_internal(wounded, i))
        goto skip_nuke;
  }

  /* Ensure the template's timely demise */
  if (is_in_character(btech_context_database(mech_context(wounded)),
                      mech_dbref(wounded))) {
    mech_communications_clear(wounded);

    /* There's a 25% chance of bsuit pilots living through it */
    if ((mech_class(wounded) == CLASS_BSUIT) &&
        (btech_random_range(mech_context(wounded), 1, 100) <= 25) &&
        wounded_pilot)
      autoeject(wounded_pilot, wounded, 1);
    else
      mech_contents_kill_if_in_character(wounded);
    /* Schedule removal of the template */
    if (!mech_is_dropship(wounded))
      discard_mw(wounded);
  }

  /* We've done everything we should do... */
  return;
skip_nuke:

  /* Add 4 MW damage if it's a MW loosing a location */
  if (mech_class(wounded) == CLASS_MW) {
    mwlethaldam(wounded, attacker, 4);
  }

  /* If it's a MW or a mech, let's see if there's additional stuff we need to do
   */
  if (mech_class(wounded) == CLASS_MW || mech_class(wounded) == CLASS_MECH) {
    if (hitloc == LTORSO)
      mech_section_destroy(wounded, attacker, LOS, LARM);
    else if (hitloc == RTORSO)
      mech_section_destroy(wounded, attacker, LOS, RARM);
    else if (hitloc == CTORSO || hitloc == HEAD) {
      if (!mech_is_destroyed(wounded)) {
        if (hitloc == HEAD) {
          if (attacker && mech_aim_section(attacker) == HEAD) {
            mech_destroy(wounded, attacker, 1, KILL_TYPE_HEAD_TARGET);
          } else {
            mech_destroy(wounded, attacker, 1, KILL_TYPE_BEHEADED);
          }
        } else {
          mech_destroy(wounded, attacker, 1, KILL_TYPE_NORMAL);
        }
      }
      /* If it's the head or a MW's CT, kill the contents if IC */
      if (hitloc == HEAD ||
          ((mech_class(wounded) == CLASS_MW) && (hitloc == CTORSO))) {
        if (is_in_character(btech_context_database(mech_context(wounded)),
                            mech_dbref(wounded))) {
          mech_communications_clear(wounded);
          mech_contents_kill_if_in_character(wounded);
        }
      }

      if (mech_class(wounded) == CLASS_MW)
        discard_mw(wounded);
    }

    return;
  }

  /* If we're an aero... */
  if (mech_is_aerospace_unit(wounded)) {
    /* FIXME: Could this be the invincible aero bug? */
    /* Aero handling is trivial ; No destruction whatsoever, for now. */
    /* With one exception.. */
    if (hitloc == COCKPIT && mech_class(wounded) == CLASS_AERO) {
      if (!mech_is_destroyed(wounded))
        mech_destroy(wounded, attacker, 0, KILL_TYPE_COCKPIT);
      mech_communications_clear(wounded);
      mech_contents_kill_if_in_character(wounded);
    }
    return;
  }

  /* Last check to see if we destroy the unit... vehicle stuff */
  if (mech_hit_location_transfer(wounded, 0) < 0)
    tKillMech = 1;
  else
    tKillMech = 0;
  switch (mech_class(wounded)) {
  case CLASS_BSUIT:
    tKillMech = 0;
    break;
  case CLASS_VEH_GROUND:
    if (hitloc == TURRET) {
      tKillMech = 0;
      mech_turret_auto_turn_set(wounded, false);
    } else
      tKillMech = 1;
    break;
  case CLASS_VTOL:
    if (hitloc == ROTOR) {
      tKillMech = 0;
      mech_vtol_crash_start(wounded);
    } else
      tKillMech = 1;
    break;
  default:
    break;
  }

  if (tKillMech) {
    if (!mech_is_destroyed(wounded))
      mech_destroy(wounded, attacker, 1, KILL_TYPE_NORMAL);
  }
}

char *mech_armor_status_set_value(Mech *mech, char *sectstr, char *typestr,
                                  char *valuestr) {
  int index, type, value;

  if (!sectstr || !*sectstr)
    return "#-1 INVALID SECTION";
  index = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                 sectstr);
  if (index == -1 || !mech_section_original_internal(mech, index))
    return "#-1 INVALID SECTION";
  if ((value = atoi(valuestr)) < 0 || value > 255)
    return "#-1 INVALID ARMORVALUE";
  switch (type = atoi(typestr)) {
  case 0:
    mech_section_armor_set(mech, index, value);
    break;
  case 1:
    mech_section_internal_set(mech, index, value);
    break;
  case 2:
    mech_section_rear_armor_set(mech, index, value);
    break;
  default:
    return "#-1 INVALID ARMORTYPE";
  }
  return "1";
}

int mech_damage_apply_clusters(DbRef player, Mech *mech, int totaldam,
                               int clustersize, int direction, int iscritical,
                               char *mechmsg, char *mechbroadcast) {

  int hitloc = 1, this_time, isrear = 0, dummy = 0;
  int *dummy1 = &dummy, *dummy2 = &dummy;

  if (direction < 8) {
    hitloc = direction;
  } else if (direction < 16) {
    hitloc = direction - 8;
    isrear = 1;
  } else if (direction > 21) {
    return 0;
  }

  if (mechmsg && *mechmsg)
    mech_notify(mech, MECHALL, mechmsg);
  if (mechbroadcast && *mechbroadcast)
    mech_los_broadcast(mech, mechbroadcast);
  while (totaldam) {
    if (direction > 18)
      isrear = 1;
    if (direction > 15)
      hitloc =
          mech_hit_location(mech, ((direction - 1) & 3) + 1, dummy1, dummy2);
    this_time = MIN(clustersize, totaldam);
    DamageMech(mech, mech, 0, -1, hitloc, isrear, iscritical, this_time, 0, 0,
               0, -1, 0, 1);
    totaldam -= this_time;
  }
  return 1;
}

void mech_damage(DbRef player, Mech *mech, char *buffer) {
  char *args[5];
  int damage, clustersize;
  int isrear, iscritical;

  if (mech_parseattributes(buffer, args, 5) != 4) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid arguments!");
    return;
  }
  if ((!((damage) = atoi(args[0])) && strcmp((args[0]), "0"))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid damage!");
    return;
  }
  if ((!((clustersize) = atoi(args[1])) && strcmp((args[1]), "0"))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid cluster size!");
    return;
  }
  if ((!((isrear) = atoi(args[2])) && strcmp((args[2]), "0"))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid isrear flag!");
    return;
  }
  if ((!((iscritical) = atoi(args[3])) && strcmp((args[3]), "0"))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid iscritical flag!");
    return;
  }
  if (damage <= 0 || damage > 1000) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid damage!");
    return;
  }
  if (clustersize <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid cluster size!");
    return;
  }
  if (clustersize > damage) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "Invalid cluster size! (must be smaller than damage amount, but > 0)");
    return;
  }
  if (mech_class(mech) == CLASS_MW) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "No MW killings!");
    return;
  }
  mech_missile_apply_hits(mech, mech, -1, -1, isrear, iscritical, 0, -1, -1,
                          clustersize, damage / clustersize, 1, 0, 0, 0);
}

void mech_damage_section(DbRef player, Mech *mech, char *buffer) {
  char *args[5];
  int damage, isrear, iscritical, section;

  /* ARGS: <SECTION> <DAMAGE> <ISREAR> <ISCRITICAL> */

  if (mech_parseattributes(buffer, args, 5) != 4) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid Arguments: <SECTION> <DAMAGE> <ISREAR> <ISCRITICAL>");
    return;
  }

  section = ArmorSectionFromString(mech_class(mech), mech_movement_type(mech),
                                   args[0]);

  if (section == -1) {
    invalid_section(player, mech);
    return;
  }

  if ((!((damage) = atoi(args[1])) && strcmp((args[1]), "0"))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid damage (Arg 2) amount! (Must be a number!)");
    return;
  }
  if ((!((isrear) = atoi(args[2])) && strcmp((args[2]), "0"))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Isrear value (Arg 3) Invalid! (1 or 0)");
    return;
  }
  if ((!((iscritical) = atoi(args[3])) && strcmp((args[3]), "0"))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Iscritical value (Arg 4) Invalid! (1 or 0)");
    return;
  }
  if (damage <= 0 || damage > 1000) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid damage (Arg 2 amount! (Must be >0 or <1000)");
    return;
  }
  DamageMech(mech, mech, 0, -1, section, isrear, iscritical, damage, 0, 0, 0,
             -1, 0, 1);
}
