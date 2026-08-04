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
#include "legacy_macros.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_ammodump_api.h"
#include "mech_build_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_pickup_api.h"
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
void DestroyWeapon(Mech *wounded, int hitloc, int type, int startCrit,
                   int numcrits, int totalcrits) {
  int i;
  char sum = totalcrits;
  char destroyed = numcrits;
  int checkloc;
  int newcrit;
  int split;
  int disable = 0; // Hax for later destroying all crits or disabling

  for (i = startCrit; i < NUM_CRITICALS; i++) {
    if (GetPartType(wounded, hitloc, i) == type) {
      if (PartIsDamaged(wounded, hitloc, i)) {
        if (disable)
          DisablePart(wounded, hitloc, i);
        else
          DestroyPart(wounded, hitloc, i);
      } else if (destroyed) {
        if (disable)
          DisablePart(wounded, hitloc, i);
        else
          DestroyPart(wounded, hitloc, i);
        destroyed--;
      } else {
        BreakPart(wounded, hitloc, i);
      }
      sum--;
      //                      if(sum == totalcrits)
      if (!sum)
        return;
    }
  }
  // if we've gotten here, then sum != total crits, but we've run outta crits in
  // this location, so it must be a split crit.
  if (MechType(wounded) != CLASS_MECH)
    return; // sanity check
  if (GetSplitData(wounded, hitloc, startCrit, &checkloc, &newcrit, &split))
    DestroyWeapon(wounded, checkloc, split, newcrit, destroyed, sum);
}

int CountWeaponsInLoc(Mech *mech, int loc) {
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

int FindWeaponTypeNumInLoc(Mech *mech, int loc, int num) {
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

void LoseWeapon(Mech *mech, int hitloc) {
  /* Look for hit locations.. */
  int i = CountWeaponsInLoc(mech, hitloc);
  int a, b;
  int firstCrit;

  if (!i)
    return;
  a = btech_random_range(mech->xcode.context, 1, i);
  b = FindWeaponTypeNumInLoc(mech, hitloc, a);
  if (b < 0)
    return;

  firstCrit = FindFirstWeaponCrit(mech, hitloc, -1, 0, I2Weapon(b),
                                  GetWeaponCrits(mech, b));

  DestroyWeapon(mech, hitloc, I2Weapon(b), firstCrit, 1,
                GetWeaponCrits(mech, b));
  mech_printf(mech, MECHALL, "[fg=red bold]Your %s is destroyed![reset]",
              &MechWeapons[b].name[3]);
}

void DestroyHeatSink(Mech *mech, int hitloc) {
  /* This can be done easily, or this can be done painfully. */
  /* Let's try the painful way, it's more fun that way. */
  int num;
  int i = I2Special(HEAT_SINK);

  if (FindObj(mech, hitloc, i)) {
    num = HS_Size(mech);
    DestroyWeapon(mech, hitloc, i, 0, 1, num);
    MechRealNumsinks(mech) -= MAX(num, 2);
    mech_notify(mech, MECHALL,
                "The computer shows a heatsink died due to the impact.");
  }
}

void DestroySection(Mech *wounded, Mech *attacker, int LOS, int hitloc) {
  char locname[30] = {0};
  char msgbuf[MBUF_SIZE] = {0};
  int i, j;
  int tKillMech;
  int tIsLeg = ((hitloc == RLEG || hitloc == LLEG) ||
                ((hitloc == RARM || hitloc == LARM) && (MechIsQuad(wounded))));
  DbRef wounded_pilot = MechPilot(wounded);
  Mech *ttarget;

  /* Prevent the rare occurance of a section getting destroyed twice */
  if (SectIsDestroyed(wounded, hitloc)) {
    fprintf(stderr, "Double-desting section %d on mech #%ld\n", hitloc,
            wounded->mynum);
    if (IsDS(wounded))
      return;
    for (i = 0; i < NUM_SECTIONS; i++)
      if (GetSectOInt(wounded, i) && GetSectInt(wounded, i))
        return;
    if (mux_event_count_type_data(wounded->xcode.context->events,
                                  EVENT_NUKEMECH, (void *)wounded)) {
      fprintf(stderr, "And nuke event already existed.\n");
      return;
    }
    discard_mw(wounded);
    return;
  }
  /* Ouch. They got toasted */
  SetSectArmor(wounded, hitloc, 0);
  SetSectInt(wounded, hitloc, 0);
  SetSectRArmor(wounded, hitloc, 0);
  SetSectDestroyed(wounded, hitloc);
  MechSections(wounded)[hitloc].specials = 0;

  /* uncycle the section <in the case of an arm/leg that was kicking getting
   * blown */
  mech_set_recycle_limb(wounded, hitloc, 0);

  /* drop off what we were carrying, since we really can't pick it up with one
   * arm */
  if ((hitloc == RARM || hitloc == LARM)) {
    if (MechCarrying(wounded) > 0) {
      if ((ttarget = btech_context_get_mech(wounded->xcode.context,
                                            MechCarrying(wounded)))) {
        mech_notify(ttarget, MECHALL, "Your tow lines go suddenly slack!");
        mech_dropoff(GOD, wounded, "");
      }
    }
  }

  /* Tell the attacker about it... */
  if (attacker) {
    ArmorStringFromIndex(hitloc, locname, MechType(wounded), MechMove(wounded));
    if (LOS >= 0)
      mech_printf(wounded, MECHALL, "Your %s has been destroyed!", locname);
    snprintf(msgbuf, sizeof(msgbuf), "'s %s has been destroyed!", locname);
    MechLOSBroadcast(wounded, msgbuf);
  }

  /* Destroy everything in the loc */
  mech_parts_destroy(attacker, wounded, hitloc, 0, 0);
  checkECM(wounded);
  /* Stop lateral if we're a quad */
  if ((MechType(wounded) == CLASS_MECH) && MechIsQuad(wounded))
    if (MechLateral(wounded) && tIsLeg)
      MechLateral(wounded) = 0;
  /* Check to see if we should destroy the unit */
  if (MechType(wounded) == CLASS_BSUIT) {
    if (CountBSuitMembers(wounded) > 0)
      goto skip_nuke;
    else if (!Destroyed(wounded))
      DestroyMech(wounded, attacker, 1, KILL_TYPE_NORMAL);
  } else {
    for (i = 0; i < NUM_SECTIONS; i++)
      if (GetSectOInt(wounded, i) && GetSectInt(wounded, i))
        goto skip_nuke;
  }

  /* Ensure the template's timely demise */
  if (is_in_character(wounded->xcode.context->database, wounded->mynum)) {
    /* Clear the freqs on the unit... */
    for (j = 0; j < FREQS; j++) {
      wounded->freq[j] = 0;
      wounded->freqmodes[j] = 0;
      wounded->chantitle[j][0] = 0;
    }

    /* There's a 25% chance of bsuit pilots living through it */
    if ((MechType(wounded) == CLASS_BSUIT) &&
        (btech_random_range(wounded->xcode.context, 1, 100) <= 25) &&
        wounded_pilot)
      autoeject(wounded_pilot, wounded, 1);
    else
      KillMechContentsIfIC(wounded);
    /* Schedule removal of the template */
    if (!IsDS(wounded))
      discard_mw(wounded);
  }

  /* We've done everything we should do... */
  return;
skip_nuke:

  /* Add 4 MW damage if it's a MW loosing a location */
  if (MechType(wounded) == CLASS_MW) {
    mwlethaldam(wounded, attacker, 4);
  }

  /* If it's a MW or a mech, let's see if there's additional stuff we need to do
   */
  if (MechType(wounded) == CLASS_MW || MechType(wounded) == CLASS_MECH) {
    if (hitloc == LTORSO)
      DestroySection(wounded, attacker, LOS, LARM);
    else if (hitloc == RTORSO)
      DestroySection(wounded, attacker, LOS, RARM);
    else if (hitloc == CTORSO || hitloc == HEAD) {
      if (!Destroyed(wounded)) {
        if (hitloc == HEAD) {
          if (attacker && MechAim(attacker) == HEAD) {
            DestroyMech(wounded, attacker, 1, KILL_TYPE_HEAD_TARGET);
          } else {
            DestroyMech(wounded, attacker, 1, KILL_TYPE_BEHEADED);
          }
        } else {
          DestroyMech(wounded, attacker, 1, KILL_TYPE_NORMAL);
        }
      }
      /* If it's the head or a MW's CT, kill the contents if IC */
      if (hitloc == HEAD ||
          ((MechType(wounded) == CLASS_MW) && (hitloc == CTORSO))) {
        if (is_in_character(wounded->xcode.context->database, wounded->mynum)) {
          for (j = 0; j < FREQS; j++) {
            wounded->freq[j] = 0;
            wounded->freqmodes[j] = 0;
            wounded->chantitle[j][0] = 0;
          }
          KillMechContentsIfIC(wounded);
        }
      }

      if (MechType(wounded) == CLASS_MW)
        discard_mw(wounded);
    }

    return;
  }

  /* If we're an aero... */
  if (is_aero(wounded)) {
    /* FIXME: Could this be the invincible aero bug? */
    /* Aero handling is trivial ; No destruction whatsoever, for now. */
    /* With one exception.. */
    if (hitloc == COCKPIT && MechType(wounded) == CLASS_AERO) {
      if (!Destroyed(wounded))
        DestroyMech(wounded, attacker, 0, KILL_TYPE_COCKPIT);
      for (j = 0; j < FREQS; j++) {
        wounded->freq[j] = 0;
        wounded->freqmodes[j] = 0;
        wounded->chantitle[j][0] = 0;
      }
      KillMechContentsIfIC(wounded);
    }
    return;
  }

  /* Last check to see if we destroy the unit... vehicle stuff */
  if (TransferTarget(wounded, 0) < 0)
    tKillMech = 1;
  else
    tKillMech = 0;
  switch (MechType(wounded)) {
  case CLASS_BSUIT:
    tKillMech = 0;
    break;
  case CLASS_VEH_GROUND:
    if (hitloc == TURRET) {
      tKillMech = 0;
      MechStatus2(wounded) &= ~AUTOTURN_TURRET;
    } else
      tKillMech = 1;
    break;
  case CLASS_VTOL:
    if (hitloc == ROTOR) {
      tKillMech = 0;
      StartVTOLCrash(wounded);
    } else
      tKillMech = 1;
    break;
  }

  if (tKillMech) {
    if (!Destroyed(wounded))
      DestroyMech(wounded, attacker, 1, KILL_TYPE_NORMAL);
  }
}

char *setarmorstatus_func(Mech *mech, char *sectstr, char *typestr,
                          char *valuestr) {
  int index, type, value;

  if (!sectstr || !*sectstr)
    return "#-1 INVALID SECTION";
  index = ArmorSectionFromString(MechType(mech), MechMove(mech), sectstr);
  if (index == -1 || !GetSectOInt(mech, index))
    return "#-1 INVALID SECTION";
  if ((value = atoi(valuestr)) < 0 || value > 255)
    return "#-1 INVALID ARMORVALUE";
  switch (type = atoi(typestr)) {
  case 0:
    SetSectArmor(mech, index, value);
    break;
  case 1:
    SetSectInt(mech, index, value);
    break;
  case 2:
    SetSectRArmor(mech, index, value);
    break;
  default:
    return "#-1 INVALID ARMORTYPE";
  }
  return "1";
}

int dodamage_func(DbRef player, Mech *mech, int totaldam, int clustersize,
                  int direction, int iscritical, char *mechmsg,
                  char *mechbroadcast) {

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
    MechLOSBroadcast(mech, mechbroadcast);
  while (totaldam) {
    if (direction > 18)
      isrear = 1;
    if (direction > 15)
      hitloc = FindHitLocation(mech, ((direction - 1) & 3) + 1, dummy1, dummy2);
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

  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 5) != 4,
                  "Invalid arguments!");
  DOCHECK_CONTEXT(mech->xcode.context, Readnum(damage, args[0]),
                  "Invalid damage!");
  DOCHECK_CONTEXT(mech->xcode.context, Readnum(clustersize, args[1]),
                  "Invalid cluster size!");
  DOCHECK_CONTEXT(mech->xcode.context, Readnum(isrear, args[2]),
                  "Invalid isrear flag!");
  DOCHECK_CONTEXT(mech->xcode.context, Readnum(iscritical, args[3]),
                  "Invalid iscritical flag!");
  DOCHECK_CONTEXT(mech->xcode.context, damage <= 0 || damage > 1000,
                  "Invalid damage!");
  DOCHECK_CONTEXT(mech->xcode.context, clustersize <= 0,
                  "Invalid cluster size!");
  DOCHECK_CONTEXT(
      mech->xcode.context, clustersize > damage,
      "Invalid cluster size! (must be smaller than damage amount, but > 0)");
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) == CLASS_MW,
                  "No MW killings!");
  Missile_Hit(mech, mech, -1, -1, isrear, iscritical, 0, -1, -1, clustersize,
              damage / clustersize, 1, 0, 0, 0);
}

void mech_damage_section(DbRef player, Mech *mech, char *buffer) {
  char *args[5];
  int damage, isrear, iscritical, section;

  /* ARGS: <SECTION> <DAMAGE> <ISREAR> <ISCRITICAL> */

  DOCHECK_CONTEXT(
      mech->xcode.context, mech_parseattributes(buffer, args, 5) != 4,
      "Invalid Arguments: <SECTION> <DAMAGE> <ISREAR> <ISCRITICAL>");

  section = ArmorSectionFromString(MechType(mech), MechMove(mech), args[0]);

  if (section == -1) {
    invalid_section(player, mech);
    return;
  }

  DOCHECK_CONTEXT(mech->xcode.context, Readnum(damage, args[1]),
                  "Invalid damage (Arg 2) amount! (Must be a number!)");
  DOCHECK_CONTEXT(mech->xcode.context, Readnum(isrear, args[2]),
                  "Isrear value (Arg 3) Invalid! (1 or 0)");
  DOCHECK_CONTEXT(mech->xcode.context, Readnum(iscritical, args[3]),
                  "Iscritical value (Arg 4) Invalid! (1 or 0)");
  DOCHECK_CONTEXT(mech->xcode.context, damage <= 0 || damage > 1000,
                  "Invalid damage (Arg 2 amount! (Must be >0 or <1000)");
  DamageMech(mech, mech, 0, -1, section, isrear, iscritical, damage, 0, 0, 0,
             -1, 0, 1);
}
