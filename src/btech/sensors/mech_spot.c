/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2001-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2001 Thomas Wouters
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map_terrain.h"
#include "mech_bth_api.h"
#include "mech_combat_api.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/server/platform.h"
#include "registry_api.h"

int IsArtyMech(Mech *mech) {
  int weapnum, section, critical, weaptype = -2;

  for (weapnum = 0; weaptype != -1; weapnum++) {
    weaptype = FindWeaponNumberOnMech(mech, weapnum, &section, &critical);
    if (IsArtillery(weaptype))
      return 1;
  }
  return 0;
}

static void mech_check_range(MuxEvent *e) {
  Mech *spotter = (Mech *)e->data2, *mech = (Mech *)e->data;
  float range;

  if (!mech)
    return;

  if (MechSpotter(mech) == -1)
    return;

  if (!spotter) {
    mech_notify(mech, MECHALL, "You have lost link with your spotter!");
    MechSpotter(mech) = -1;
    return;
  }
  range = FlMechRange(fl, mech, spotter);
  if (range > 2 * MechRadioRange(spotter) || MechSpotter(spotter) == -1 ||
      spotter->mapindex != mech->mapindex) {
    mech_notify(mech, MECHALL, "You have lost link with your spotter!");
    MechSpotter(mech) = -1;
    return;
  }
  mech_event_schedule(mech, EVENT_SPOT_CHECK, mech_check_range, SPOT_TICK,
                      (intptr_t)spotter);
}

static void mech_spot_event(MuxEvent *e) {
  Mech *target, *mech = (Mech *)e->data;
  struct MechSpotData *sd = (struct MechSpotData *)e->data2;

  target = (Mech *)sd->target;

  if (MechFX(mech) != sd->mechFX && MechFY(mech) != sd->mechFY &&
      MechFX(target) != sd->tarFX && MechFY(target) != sd->tarFY) {
    mech_notify(target, MECHALL,
                "The data link was not established due to movement!");
    mech_notify(mech, MECHALL,
                "The data link was not established due to movement!");
    free((void *)e->data2);
    return;
  }
  mech_printf(target, MECHALL, "Data link established with %s.",
              mech_to_mech_display_id(target, mech).text);
  mech_printf(mech, MECHALL,
              "Data link established with %s, you now have a forward observer.",
              mech_to_mech_display_id(target, mech).text);
  MechSpotter(mech) = target->mynum;
  mech_event_schedule(mech, EVENT_SPOT_CHECK, mech_check_range, SPOT_TICK,
                      (intptr_t)target);
  free((void *)e->data2);
}

void ClearFireAdjustments(BattleMap *map, DbRef mech) {
  int i;
  Mech *m;

  for (i = 0; i < map->first_free; i++)
    if (map->mechsOnMap[i] >= 0) {
      if (!(m = btech_context_get_mech(map->xcode.context, map->mechsOnMap[i])))
        continue;
      if (m->mynum == mech)
        continue;
      if (MechSpotter(m) == mech)
        MechFireAdjustment(m) = 0;
    }
}

void mech_spot(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  char *args[5];
  char targetID[3];
  int argc;
  int LOS = 1;
  DbRef targetref;
  float range;
  struct MechSpotData *dat;
  BattleMap *mech_map;

  cch(MECH_USUALO);
  mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  argc = mech_parseattributes(buffer, args, 5);
#ifdef BT_MOVEMENT_MODES
  DOCHECK_CONTEXT(mech->xcode.context, mech_move_mode_locked(mech),
                  "You cannot spot while using a special movement mode.");
#endif
  DOCHECK_CONTEXT(mech->xcode.context, argc != 1,
                  "You may only use mech ID's to set spotter!");
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) == CLASS_MW,
                  "Spot ? You ? What with, your pretty blue eyes ? Hah!");
  targetID[0] = args[0][0];
  targetID[1] = args[0][1];
  targetID[2] = 0;
  targetref = FindTargetDBREFFromMapNumber(mech, targetID);
  if (!strcmp(args[0], "-")) {
    if (MechSpotter(mech) == mech->mynum) {
      mech_notify(mech, MECHALL, "You spot no longer.");
      ClearFireAdjustments(mech_map, mech->mynum);
    } else
      mech_notify(mech, MECHALL, "You disable the datalink to spotter.");
    MechSpotter(mech) = -1;
    return;
  }
  if (!strcasecmp(targetID, mech_id(mech, false).text)) {
    DOCHECK_CONTEXT(mech->xcode.context, MechFullNoRecycle(mech, CHECK_BOTH),
                    "You have weapons recycling!");
    MechSpotter(mech) = mech->mynum;
    mech_notify(mech, MECHALL, "You are now set as a spotter.");
    return;
  }
  target = btech_context_get_mech(mech->xcode.context, targetref);
  if (target)
    LOS = InLineOfSight(mech, target, MechX(target), MechY(target),
                        FlMechRange(mech_map, mech, target));
  DOCHECK_CONTEXT(mech->xcode.context,
                  !target || (targetref == -1) ||
                      MechTeam(target) != MechTeam(mech),
                  "That target does not exist!");

  DOCHECK_CONTEXT(
      mech->xcode.context, MechType(target) == CLASS_MW,
      "Spot ? That puny being ?! What with, those clear brown eyes ? Hah!");
  DOCHECK_CONTEXT(mech->xcode.context, MechSpotter(target) != target->mynum,
                  "That 'mech is not set up as spotter!");

  if (IsArtyMech(mech) && !LOS) {
    mech_notify(target, MECHALL,
                "Someone is trying to establish a data link with you!");
    mech_notify(mech, MECHALL,
                "You attempt to establish a data link..... please stand by.");
    range = FlMechRange(mech_map, mech, target);
    if (range > 2 * MechRadioRange(target)) {
      mech_notify(mech, MECHALL, "That target is our of data link range!");
      return;
    }
    Create(dat, struct MechSpotData, 1);
    dat->mechFY = MechFY(mech);
    dat->mechFX = MechFX(mech);
    dat->tarFX = MechFX(target);
    dat->tarFY = MechFY(target);
    dat->target = (Mech *)target;
    // NOLINTNEXTLINE(clang-analyzer-unix.Malloc)
    mech_event_schedule(mech, EVENT_SPOT_LOCK, mech_spot_event,
                        WEAPON_TICK * ((int)range / 10 + 5), (intptr_t)dat);
    return;
  } else
    DOCHECK_CONTEXT(mech->xcode.context, !LOS,
                    "You do not have LOS to that target!")
  MechSpotter(mech) = targetref;
  MechFireAdjustment(mech) = 0;
  mech_printf(mech, MECHALL, "%s set as spotter.",
              mech_to_mech_display_id(mech, target).text);
}

int FireSpot(DbRef player, Mech *mech, BattleMap *mech_map, int weaponnum,
             int weapontype, int sight, int section, int critical) {
  /* Nim 9/11/96 */

  float spot_range, range;
  float enemyX, enemyY, enemyZ = 0;
  int LOS, mapx = 0, mapy = 0;
  Mech *target = NULL, *spotter;
  int spotTerrain;
  int found_target = 0;

  /* No spotter or not IDF weapon lets get outta here */
  if (MechSpotter(mech) == -1 || !(MechWeapons[weapontype].special & IDF))
    return 0;

  spotter = btech_context_get_mech(mech->xcode.context, MechSpotter(mech));
  DOCHECKMP1(!spotter, "There is no spotter avilable to IDF with!");

  if (!(MechSpotter(spotter) == spotter->mynum)) {
    mech_notify(mech, MECH_PILOT, "You do not have a spotter!");
    MechSpotter(mech) = -1;
    return 1;
  }
  DOCHECKMP1(Uncon(spotter), "Your spotter is unconscious!");
  DOCHECKMP1(Blinded(spotter), "Your spotter can't see a thing!");

  /* Is the spotter set to a Mech or to a Hex? */
  if (MechTarget(spotter) != -1) {
    target = btech_context_get_mech(mech->xcode.context, MechTarget(spotter));
    DOCHECKMP1(!target, "Your spotter has invalid target!");
    mapx = MechX(target);
    mapy = MechY(target);
    spot_range = FaMechRange(spotter, target);
    LOS = InLineOfSight(spotter, target, mapx, mapy, spot_range);
    DOCHECKMP1(!LOS, "You spotter does not have a target in LOS!");
    range = FaMechRange(mech, target);
    DOCHECK0_CONTEXT(mech->xcode.context, InWater(target) && !(InWater(mech)),
                     "You can't fire into water with that weapon from here.");

    spotTerrain =
        IsArtillery(weapontype)
            ? 2
            : (1 + AddTerrainMod(spotter, target, mech_map, spot_range, 0) +
               AttackMovementMods(spotter) +
               ((mech_event_count(spotter, EVENT_LOCK) &&
                 MechTargComp(spotter) != TARGCOMP_MULTI)
                    ? 2
                    : 0));
    DOCHECK1_CONTEXT(mech->xcode.context, IsArtillery(weapontype) && target,
                     "You can only target hexes with this kind of artillery.");
    if (!sight) {
      AccumulateSpotXP(MechPilot(spotter), spotter, target);
      AccumulateArtyXP(MechPilot(mech), mech, target);
    }
    FireWeapon(mech, mech_map, target, 0, weapontype, weaponnum, section,
               critical, MechFX(target), MechFY(target), mapx, mapy, range,
               spotTerrain, sight, 2);
    return 1;
  }
  if (!(MechTargX(spotter) >= 0 && MechTargY(spotter) >= 0)) {
    notify(btech_context_evaluation(mech->xcode.context), player,
           "Your spotter has no target set!");
    return 1;
  }
  if (!IsArtillery(weapontype))
    if ((target = find_mech_in_hex(mech, mech_map, MechTargX(spotter),
                                   MechTargY(spotter), 0))) {
      enemyX = MechFX(target);
      enemyY = MechFY(target);
      enemyZ = MechFZ(target);
      mapx = MechX(target);
      mapy = MechY(target);
      found_target = 1;
    }
  if (!found_target) {
    target = (Mech *)NULL;
    mapx = MechTargX(spotter);
    mapy = MechTargY(spotter);
    enemyZ = ZSCALE * MechTargZ(spotter);
    MapCoordToRealCoord(mapx, mapy, &enemyX, &enemyY);
  }
  spot_range = FindRange(MechFX(spotter), MechFY(spotter), MechFZ(spotter),
                         enemyX, enemyY, enemyZ);
  LOS = InLineOfSight(spotter, target, mapx, mapy, spot_range);
  DOCHECK0_CONTEXT(mech->xcode.context, !LOS,
                   "That target is not in your spotters line of sight!");
  range = FindRange(MechFX(mech), MechFY(mech), MechFZ(mech), enemyX, enemyY,
                    enemyZ);
  spotTerrain = IsArtillery(weapontype)
                    ? 2
                    : (1 + AttackMovementMods(spotter) +
                       ((mech_event_count(spotter, EVENT_LOCK) &&
                         MechTargComp(spotter) != TARGCOMP_MULTI)
                            ? 2
                            : 0));
  FireWeapon(mech, mech_map, target, 0, weapontype, weaponnum, section,
             critical, enemyX, enemyY, mapx, mapy, range, spotTerrain, sight,
             2);
  return 1;
}
