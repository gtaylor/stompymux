/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 */

#include <math.h>
#include <stddef.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "eject_api.h"
#include "legacy_macros.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_restrict_api.h"
#include "mech_startup_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "mux/world/access.h"
#include "mux/world/move.h"
#include "registry_api.h"

void mech_createbays(DbRef player, void *data, char *buffer) {
  char *args[NUM_BAYS + 1];
  int argc;
  DbRef it;
  int i;
  Mech *ds = (Mech *)data;
  BattleMap *map;

  DOCHECK_CONTEXT(ds->xcode.context,
                  (argc = mech_parseattributes(buffer, args, NUM_BAYS + 1)) ==
                      (NUM_BAYS + 1),
                  "Invalid number of arguments!");
  for (i = 0; i < argc; i++) {
    it = match_thing(&btech_context_command(ds->xcode.context)->match, player,
                     args[i]);
    DOCHECK_CONTEXT(ds->xcode.context, it == NOTHING,
                    tprintf("Argument %d is invalid.", i + 1));
    DOCHECK_CONTEXT(ds->xcode.context,
                    !btech_context_is_map(ds->xcode.context, it),
                    tprintf("Argument %d is not a map.", i + 1));
    map = btech_context_find_object(ds->xcode.context, it);
    AeroBay(ds, i) = it;
    map->onmap = ds->mynum;
  }
  for (i = argc; i < NUM_BAYS; i++)
    AeroBay(ds, i) = -1;
  notify_printf(btech_context_evaluation(ds->xcode.context), player,
                "%d bay(s) set up!", argc);
}

extern const int dirs[6][2];

static const int dir2loc[6] = {DS_NOSE, DS_RWING,  DS_RRWING,
                               DS_AFT,  DS_LRWING, DS_LWING};

int Find_DS_Bay_Number(Mech *ds, int dir) {
  int bayn = 0;
  int i, j;

  for (i = 0; i <= dir; i++) {
    for (j = 0; j < NUM_CRITICALS; j++)
      if (GetPartType(ds, dir2loc[i % 6], j) == I2Special(DS_MECHDOOR) ||
          GetPartType(ds, dir2loc[i % 6], j) == I2Special(DS_AERODOOR))
        break;
    if (j != NUM_CRITICALS) {
      if (i == dir)
        return bayn;
      bayn++;
    }
  }
  return -1;
}

int Find_DS_Bay_Dir(Mech *ds, int num) {
  int i;

  for (i = 0; i < 6; i++)
    if (Find_DS_Bay_Number(ds, i) == num)
      return i;
  return -1;
}

#define KLUDGE(fx, tx) ((((fx) % 2) && !((tx) % 2)) ? -1 : 0)

int Find_DS_Bay_In_MechHex(Mech *seer, Mech *ds, long *bayn) {
  int i;
  int t = DSBearMod(ds);

  for (i = t; i < (t + 6); i++) {

    if (((MechX(ds) + dirs[i % 6][0]) == MechX(seer)) &&
        ((MechY(ds) + dirs[i % 6][1] +
          KLUDGE(MechX(ds), MechX(ds) + dirs[i % 6][0])) == MechY(seer))) {
      if ((*bayn = Find_DS_Bay_Number(ds, ((i - t + 6) % 6))) >= 0)
        return 1;
      return 0;
    }
  }
  return 0;
}

static int Find_Single_DS_In_MechHex(Mech *mech, long *ref, long *bayn) {
  BattleMap *map =
      btech_context_find_object(mech->xcode.context, mech->mapindex);
  int loop;
  Mech *tempMech;
  int count = 0;

  *ref = 0;
  if (!map)
    return 0;
  for (loop = 0; loop < map->first_free; loop++)
    if (map->mechsOnMap[loop] >= 0) {
      if (!(tempMech = btech_context_get_mech(mech->xcode.context,
                                              map->mechsOnMap[loop])))
        continue;
      if (!IsDS(tempMech))
        continue;
      if (!Landed(tempMech))
        continue; /* This might break midflight-aero-DS-docking. But aeros are
                     broken anyway. */
      if (Find_DS_Bay_In_MechHex(mech, tempMech, bayn)) {
        if (count++)
          *ref = -1;
        else
          *ref = tempMech->mynum;
      }
    }
  return count;
}

static void mech_enterbay_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data, *ds, *tmpm = NULL;
  long ref = (long)e->data2;
  long bayn;
  int x = 5, y = 5;
  BattleMap *tmpmap;

  if (!Started(mech) || Uncon(mech) || Jumping(mech) ||
      (MechType(mech) == CLASS_MECH &&
       (Fallen(mech) || mech_event_count(mech, EVENT_STAND))) ||
      OODing(mech) ||
      (fabs(MechSpeed(mech)) * 5 >= MMaxSpeed(mech) &&
       fabs(MMaxSpeed(mech)) >= MP1) ||
      (MechType(mech) == CLASS_VTOL && AeroFuel(mech) <= 0))
    return;
  tmpmap = btech_context_get_map(mech->xcode.context, ref);
  if (!(ds = btech_context_get_mech(mech->xcode.context, tmpmap->onmap)))
    return;
  if (!Find_DS_Bay_In_MechHex(mech, ds, &bayn))
    return;
  /* whee */
  ref = AeroBay(ds, bayn);
  StopBSuitSwarmers(
      btech_context_find_object(mech->xcode.context, mech->mapindex), mech, 1);
  mech_notify(mech, MECHALL, "You enter the bay.");
  mech_los_broadcast(mech, tprintf("has entered %s at %d,%d.",
                                   mech_display_id(ds).text, MechX(mech),
                                   MechY(mech)));
  MarkForLOSUpdate(mech);
  if (MechType(mech) == CLASS_MW &&
      !is_in_character(mech->xcode.context->database, ref)) {
    enter_mw_bay(mech, ref);
    return;
  }
  if (MechCarrying(mech) > 0)
    tmpm = btech_context_get_mech(mech->xcode.context, MechCarrying(mech));
  mech_Rsetmapindex(GOD, (void *)mech, tprintf("%ld", ref));
  mech_Rsetxy(GOD, (void *)mech, tprintf("%d %d", x, y));
  mech_los_broadcast(mech, "has entered the bay.");
  move_via_teleport(btech_context_evaluation(mech->xcode.context), mech->mynum,
                    ref, 1, 0);
  if (tmpm) {
    mech_Rsetmapindex(GOD, (void *)tmpm, tprintf("%ld", ref));
    mech_Rsetxy(GOD, (void *)tmpm, tprintf("%d %d", x, y));
    move_via_teleport(btech_context_evaluation(mech->xcode.context),
                      tmpm->mynum, ref, 1, 0);
  }
}

static int DS_Bay_Is_Open(Mech *mech, Mech *ds, DbRef bayref) {
  int i, j;

  for (i = 0; i < NUM_BAYS; i++)
    if (AeroBay(ds, i) > 0)
      if (AeroBay(ds, i) == bayref) {
        j = Find_DS_Bay_Dir(ds, i);
        for (i = 0; i < NUM_CRITICALS; i++) {
          if (((is_aero(mech) &&
                GetPartType(ds, dir2loc[j], i) == I2Special(DS_AERODOOR)) ||
               (!is_aero(mech) &&
                GetPartType(ds, dir2loc[j], i) == I2Special(DS_MECHDOOR))) &&
              !PartIsDestroyed(ds, dir2loc[j], i))
            return 1;
        }
        return 0;
      }
  return 0;
}

static int DS_Bay_Is_EnterOK(Mech *mech, Mech *ds, DbRef bayref) {
  int i;

  for (i = 0; i < NUM_BAYS; i++)
    if (AeroBay(ds, i) > 0)
      if (AeroBay(ds, i) == bayref)
        return mux_event_count_type_data2(ds->xcode.context->events,
                                          EVENT_ENTER_HANGAR,
                                          (void *)bayref) > 0
                   ? 0
                   : 1;
  return 0;
}

/* ID / Number, both optional (this _will_ be painful) */

void mech_enterbay(DbRef player, void *data, char *buffer) {
  char *args[3];
  int argc;
  DbRef ref = -1, bayn = -1;
  Mech *mech = data, *ds;
  BattleMap *map;
  LuaLockInvocation lock;
  LuaLockResult lock_result;

  cch(MECH_USUAL);
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) == CLASS_VTOL && AeroFuel(mech) <= 0,
                  "You lack fuel to maneuver in!");
  DOCHECK_CONTEXT(mech->xcode.context, Jumping(mech),
                  "While in mid-jump? No way.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) == CLASS_MECH &&
                      (Fallen(mech) || mech_event_count(mech, EVENT_STAND)),
                  "Crawl inside? I think not. Stand first.");
  DOCHECK_CONTEXT(mech->xcode.context, OODing(mech),
                  "While in mid-flight? No way.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (argc = mech_parseattributes(buffer, args, 2)) == 2,
                  "Hmm, invalid number of arguments?");
  if (argc > 0)
    DOCHECK_CONTEXT(mech->xcode.context,
                    (ref = FindTargetDBREFFromMapNumber(mech, args[0])) <= 0,
                    "Invalid target!");
  if (ref < 0) {
    DOCHECK_CONTEXT(mech->xcode.context,
                    !Find_Single_DS_In_MechHex(mech, &ref, &bayn),
                    "No DS bay found in your hex!");
    DOCHECK_CONTEXT(mech->xcode.context, ref < 0,
                    "Multiple enterable things found ; use the id for "
                    "specifying which you want.");
    DOCHECK_CONTEXT(mech->xcode.context,
                    !(ds = btech_context_get_mech(mech->xcode.context, ref)),
                    "You sense wrongness in fabric of space.");
  } else {
    DOCHECK_CONTEXT(mech->xcode.context,
                    !(ds = btech_context_get_mech(mech->xcode.context, ref)),
                    "You sense wrongness in fabric of space.");
    DOCHECK_CONTEXT(mech->xcode.context,
                    !Find_DS_Bay_In_MechHex(mech, ds, &bayn),
                    "You see no bays in your hex.");
  }
  DOCHECK_CONTEXT(mech->xcode.context,
                  IsDS(mech) && !(MechSpecials2(mech) & CARRIER_TECH),
                  "Your craft can't enter bays.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  !DS_Bay_Is_Open(mech, ds, AeroBay(ds, bayn)),
                  "The door has been jammed!");
  DOCHECK_CONTEXT(mech->xcode.context, IsDS(mech),
                  "Your unit is a bit too large to fit in there.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (fabs((float)(MechSpeed(mech) - MechSpeed(ds)))) > MP1,
                  "Speed difference's too large to enter!");
  DOCHECK_CONTEXT(mech->xcode.context, MechZ(ds) != MechZ(mech),
                  "Get to same elevation before thinking about entering!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  fabs(MechVerticalSpeed(mech) - MechVerticalSpeed(ds)) > 10,
                  "Vertical speed difference is too great to enter safely!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) == CLASS_MECH && !MechIsQuad(mech) &&
                      (IsMechLegLess(mech)),
                  "Without legs? Are you kidding?");
  ref = AeroBay(ds, bayn);
  map = btech_context_get_map(mech->xcode.context, ref);

  DOCHECK_CONTEXT(mech->xcode.context, !map,
                  "You sense wrongness in fabric of space.");

  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_event_count(mech, EVENT_ENTER_HANGAR),
                  "You are already entering the hangar!");
  if (!lock_test(btech_context_evaluation(mech->xcode.context), player, player,
                 mech->mynum, ref, LUA_LOCK_ENTER,
                 LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock, &lock_result)) {
    char *msg = lock_result.has_enactor_message
                    ? lock_result.enactor_message
                    : "You are unable to enter the bay!";
    notify(btech_context_evaluation(mech->xcode.context), player, msg);
    return;
  }
  DOCHECK_CONTEXT(mech->xcode.context,
                  !DS_Bay_Is_EnterOK(mech, ds, AeroBay(ds, bayn)),
                  "Someone else is using the door at the moment.");
  DOCHECK_CONTEXT(
      mech->xcode.context,
      !(map = btech_context_get_map(mech->xcode.context, mech->mapindex)),
      "You sense a wrongness in fabric of space.");
  HexLOSBroadcast(map, MechX(mech), MechY(mech),
                  "The bay doors at $h start to open..");
  mech_event_schedule(mech, EVENT_ENTER_HANGAR, mech_enterbay_event, 12, ref);
}

static void DS_Place(Mech *ds, Mech *mech, int frombay) {
  int i;
  int nx, ny;
  BattleMap *mech_map;

  for (i = 0; i < NUM_BAYS; i++)
    if (AeroBay(ds, i) == frombay)
      break;
  if (i == NUM_BAYS || !(mech_map = btech_context_get_map(mech->xcode.context,
                                                          mech->mapindex))) {
    /* i _should_ be set, otherwise things are deeply disturbing */
    mech_notify(mech, MECHALL, "Reality collapse imminent.");
    return;
  }
  i = Find_DS_Bay_Dir(ds, i);
  nx = dirs[(DSBearMod(ds) + i) % 6][0] + MechX(ds);
  ny = dirs[(DSBearMod(ds) + i) % 6][1] + MechY(ds) + KLUDGE(MechX(ds), nx);
  nx = BOUNDED(0, nx, mech_map->map_width - 1);
  ny = BOUNDED(0, ny, mech_map->map_height - 1);

  /* snippage from mech_Rsetxy */
  MechX(mech) = nx;
  MechLastX(mech) = nx;
  MechY(mech) = ny;
  MechLastY(mech) = ny;
  MechZ(mech) = MechZ(ds);
  MechElev(mech) = MechElev(ds);
  MapCoordToRealCoord(MechX(mech), MechY(mech), &MechFX(mech), &MechFY(mech));
  MechTerrain(mech) = map_terrain_get(mech_map, MechX(mech), MechY(mech));
}

static int Leave_DS_Bay(BattleMap *map, Mech *ds, Mech *mech, DbRef frombay) {
  Mech *car = NULL;

  StopBSuitSwarmers(
      btech_context_find_object(mech->xcode.context, mech->mapindex), mech, 1);
  mech_los_broadcast(mech, "has left the bay.");
  /* We escape confines of the bay to open air/land! */
  mech_Rsetmapindex(GOD, (void *)mech, tprintf("%ld", ds->mapindex));
  if (MechCarrying(mech) > 0)
    car = btech_context_get_mech(mech->xcode.context, MechCarrying(mech));
  if (car)
    mech_Rsetmapindex(GOD, (void *)car, tprintf("%ld", ds->mapindex));
  DOCHECKMA0(mech->mapindex == map->mynum,
             "Fatal error: Unable to find the map 'ship is on.");
  move_via_teleport(btech_context_evaluation(mech->xcode.context), mech->mynum,
                    mech->mapindex, 1, 0);
  if (car)
    move_via_teleport(btech_context_evaluation(mech->xcode.context), car->mynum,
                      mech->mapindex, 1, 0);
  mech_notify(mech, MECHALL, "You have left the bay.");
  DS_Place(ds, mech, frombay);
  if (car)
    MirrorPosition(mech, car, 0);
  mech_los_broadcast_unit(mech, ds, "has left %s's bay.");
  mech_notify(ds, MECHALL,
              tprintf("%s has left the bay.", mech_display_id(mech).text));
  mech_continue_flying(mech);
  if (is_in_character(mech->xcode.context->database, mech->mynum) &&
      game_object_location(mech->xcode.context->database, MechPilot(mech)) !=
          mech->mynum) {
    mech_notify(
        mech, MECHALL,
        "[fg=red bold blink inverse]INTRUDER ALERT! INTRUDER ALERT![reset]");
    mech_notify(mech, MECHALL,
                "[fg=red bold blink]Automatic self-destruct sequence "
                "initiated.[reset]");
    mech_shutdown(GOD, (void *)mech, "");
  }
  return 1;
}

int Leave_DS(BattleMap *map, Mech *mech) {
  Mech *car;

  DOCHECKMA0(!(car = btech_context_get_mech(mech->xcode.context, map->onmap)),
             "Invalid : No parent object?");
  DOCHECKMA0(!DS_Bay_Is_Open(mech, car, map->mynum),
             "The door has been jammed!");
  DOCHECKMA0(!Landed(car) && !FlyingT(mech), "The 'ship is still airborne!");
  DOCHECKMA0(is_zombie(car->xcode.context->database, car->mynum),
             "You don't feel leaving right now would be prudent..");
  return Leave_DS_Bay(map, car, mech, map->mynum);
}
