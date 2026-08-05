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
#include "ds_bay_api.h"
#include "eject_api.h"
#include "equipment_types.h"
#include "legacy_macros.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
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
#include "section_types.h"

void mech_createbays(DbRef player, void *data, char *buffer) {
  char *args[NUM_BAYS + 1];
  int argc;
  DbRef it;
  int i;
  Mech *ds = (Mech *)data;
  BattleMap *map;
  BtechContext *context = mech_context(ds);

  DOCHECK_CONTEXT(context,
                  (argc = mech_parseattributes(buffer, args, NUM_BAYS + 1)) ==
                      (NUM_BAYS + 1),
                  "Invalid number of arguments!");
  for (i = 0; i < argc; i++) {
    it = match_thing(&btech_context_command(context)->match, player, args[i]);
    DOCHECK_CONTEXT(context, it == NOTHING,
                    tprintf("Argument %d is invalid.", i + 1));
    DOCHECK_CONTEXT(context, !btech_context_is_map(context, it),
                    tprintf("Argument %d is not a map.", i + 1));
    map = btech_context_find_object(context, it);
    mech_bay_dbref_set(ds, i, it);
    battle_map_parent_dbref_set(map, mech_dbref(ds));
  }
  for (i = argc; i < NUM_BAYS; i++)
    mech_bay_dbref_set(ds, i, -1);
  notify_printf(btech_context_evaluation(context), player, "%d bay(s) set up!",
                argc);
}

extern const int dirs[6][2];

static const int dir2loc[6] = {DS_NOSE, DS_RWING,  DS_RRWING,
                               DS_AFT,  DS_LRWING, DS_LWING};

int dropship_bay_number(Mech *ds, int dir) {
  int bayn = 0;
  int i, j;

  for (i = 0; i <= dir; i++) {
    for (j = 0; j < NUM_CRITICALS; j++)
      if (mech_critical_part_type(ds, dir2loc[i % 6], j) ==
              I2Special(DS_MECHDOOR) ||
          mech_critical_part_type(ds, dir2loc[i % 6], j) ==
              I2Special(DS_AERODOOR))
        break;
    if (j != NUM_CRITICALS) {
      if (i == dir)
        return bayn;
      bayn++;
    }
  }
  return -1;
}

int dropship_bay_direction(Mech *ds, int num) {
  int i;

  for (i = 0; i < 6; i++)
    if (dropship_bay_number(ds, i) == num)
      return i;
  return -1;
}

static int dropship_hex_row_adjustment(int from_x, int to_x) {
  return (from_x % 2 && !(to_x % 2)) ? -1 : 0;
}

int dropship_bay_in_adjacent_hex(Mech *seer, Mech *ds, long *bayn) {
  int i;
  int t = mech_dropship_bearing_sector(ds);

  for (i = t; i < (t + 6); i++) {

    int bay_x = mech_position_x(ds) + dirs[i % 6][0];
    int bay_y = mech_position_y(ds) + dirs[i % 6][1] +
                dropship_hex_row_adjustment(mech_position_x(ds), bay_x);
    if (bay_x == mech_position_x(seer) && bay_y == mech_position_y(seer)) {
      if ((*bayn = dropship_bay_number(ds, ((i - t + 6) % 6))) >= 0)
        return 1;
      return 0;
    }
  }
  return 0;
}

static int dropship_find_single_adjacent_bay(Mech *mech, long *ref,
                                             long *bayn) {
  BattleMap *map =
      btech_context_find_object(mech_context(mech), mech_map_dbref(mech));
  int loop;
  Mech *tempMech;
  int count = 0;

  *ref = 0;
  if (!map)
    return 0;
  for (loop = 0; loop < battle_map_unit_count(map); loop++)
    if (battle_map_unit_dbref(map, loop) >= 0) {
      if (!(tempMech = btech_context_get_mech(
                mech_context(mech), battle_map_unit_dbref(map, loop))))
        continue;
      if (!mech_is_dropship(tempMech))
        continue;
      if (!mech_is_landed(tempMech))
        continue; /* This might break midflight-aero-DS-docking. But aeros are
                     broken anyway. */
      if (dropship_bay_in_adjacent_hex(mech, tempMech, bayn)) {
        if (count++)
          *ref = -1;
        else
          *ref = mech_dbref(tempMech);
      }
    }
  return count;
}

static void mech_enterbay_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data, *ds, *tmpm = nullptr;
  long ref = (long)e->data2;
  long bayn;
  int x = 5, y = 5;
  BattleMap *tmpmap;
  BtechContext *context = mech_context(mech);

  if (!mech_is_started(mech) || mech_pilot_is_unconscious(mech) ||
      mech_is_jumping(mech) ||
      (mech_class(mech) == CLASS_MECH &&
       (mech_is_fallen(mech) || mech_event_count(mech, EVENT_STAND))) ||
      mech_is_out_of_control(mech) ||
      (fabs(mech_current_speed(mech)) * 5 >=
           mech_effective_maximum_speed(mech) &&
       fabs(mech_effective_maximum_speed(mech)) >= MP1) ||
      (mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0))
    return;
  tmpmap = btech_context_get_map(context, ref);
  if (!(ds = btech_context_get_mech(context, battle_map_parent_dbref(tmpmap))))
    return;
  if (!dropship_bay_in_adjacent_hex(mech, ds, &bayn))
    return;
  /* whee */
  ref = mech_bay_dbref(ds, bayn);
  StopBSuitSwarmers(btech_context_find_object(context, mech_map_dbref(mech)),
                    mech, 1);
  mech_notify(mech, MECHALL, "You enter the bay.");
  mech_los_broadcast(
      mech, tprintf("has entered %s at %d,%d.", mech_display_id(ds).text,
                    mech_position_x(mech), mech_position_y(mech)));
  MarkForLOSUpdate(mech);
  if (mech_class(mech) == CLASS_MW &&
      !is_in_character(btech_context_database(context), ref)) {
    enter_mw_bay(mech, ref);
    return;
  }
  if (mech_carried_dbref(mech) > 0)
    tmpm = btech_context_get_mech(context, mech_carried_dbref(mech));
  mech_Rsetmapindex(GOD, (void *)mech, tprintf("%ld", ref));
  mech_Rsetxy(GOD, (void *)mech, tprintf("%d %d", x, y));
  mech_los_broadcast(mech, "has entered the bay.");
  move_via_teleport(btech_context_evaluation(context), mech_dbref(mech), ref, 1,
                    0);
  if (tmpm) {
    mech_Rsetmapindex(GOD, (void *)tmpm, tprintf("%ld", ref));
    mech_Rsetxy(GOD, (void *)tmpm, tprintf("%d %d", x, y));
    move_via_teleport(btech_context_evaluation(context), mech_dbref(tmpm), ref,
                      1, 0);
  }
}

static int dropship_bay_is_open(Mech *mech, Mech *ds, DbRef bayref) {
  int i, j;

  for (i = 0; i < NUM_BAYS; i++)
    if (mech_bay_dbref(ds, i) > 0)
      if (mech_bay_dbref(ds, i) == bayref) {
        j = dropship_bay_direction(ds, i);
        for (i = 0; i < NUM_CRITICALS; i++) {
          if (((mech_is_aerospace_unit(mech) &&
                mech_critical_part_type(ds, dir2loc[j], i) ==
                    I2Special(DS_AERODOOR)) ||
               (!mech_is_aerospace_unit(mech) &&
                mech_critical_part_type(ds, dir2loc[j], i) ==
                    I2Special(DS_MECHDOOR))) &&
              !mech_critical_is_destroyed(ds, dir2loc[j], i))
            return 1;
        }
        return 0;
      }
  return 0;
}

static int dropship_bay_is_enterable(Mech *mech, Mech *ds, DbRef bayref) {
  int i;

  for (i = 0; i < NUM_BAYS; i++)
    if (mech_bay_dbref(ds, i) > 0)
      if (mech_bay_dbref(ds, i) == bayref)
        return btech_context_event_data_count(mech_context(ds),
                                              EVENT_ENTER_HANGAR, bayref) > 0
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
  BtechContext *context = mech_context(mech);

  cch(MECH_USUAL);
  DOCHECK_CONTEXT(context,
                  mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0,
                  "You lack fuel to maneuver in!");
  DOCHECK_CONTEXT(context, mech_is_jumping(mech), "While in mid-jump? No way.");
  DOCHECK_CONTEXT(
      context,
      mech_class(mech) == CLASS_MECH &&
          (mech_is_fallen(mech) || mech_event_count(mech, EVENT_STAND)),
      "Crawl inside? I think not. Stand first.");
  DOCHECK_CONTEXT(context, mech_is_out_of_control(mech),
                  "While in mid-flight? No way.");
  DOCHECK_CONTEXT(context, (argc = mech_parseattributes(buffer, args, 2)) == 2,
                  "Hmm, invalid number of arguments?");
  if (argc > 0)
    DOCHECK_CONTEXT(context,
                    (ref = FindTargetDBREFFromMapNumber(mech, args[0])) <= 0,
                    "Invalid target!");
  if (ref < 0) {
    DOCHECK_CONTEXT(context,
                    !dropship_find_single_adjacent_bay(mech, &ref, &bayn),
                    "No DS bay found in your hex!");
    DOCHECK_CONTEXT(context, ref < 0,
                    "Multiple enterable things found ; use the id for "
                    "specifying which you want.");
    DOCHECK_CONTEXT(context, !(ds = btech_context_get_mech(context, ref)),
                    "You sense wrongness in fabric of space.");
  } else {
    DOCHECK_CONTEXT(context, !(ds = btech_context_get_mech(context, ref)),
                    "You sense wrongness in fabric of space.");
    DOCHECK_CONTEXT(context, !dropship_bay_in_adjacent_hex(mech, ds, &bayn),
                    "You see no bays in your hex.");
  }
  DOCHECK_CONTEXT(context,
                  mech_is_dropship(mech) &&
                      !(mech_technology_flags_secondary(mech) & CARRIER_TECH),
                  "Your craft can't enter bays.");
  DOCHECK_CONTEXT(context,
                  !dropship_bay_is_open(mech, ds, mech_bay_dbref(ds, bayn)),
                  "The door has been jammed!");
  DOCHECK_CONTEXT(context, mech_is_dropship(mech),
                  "Your unit is a bit too large to fit in there.");
  DOCHECK_CONTEXT(
      context, fabsf(mech_current_speed(mech) - mech_current_speed(ds)) > MP1,
      "Speed difference's too large to enter!");
  DOCHECK_CONTEXT(context, mech_position_z(ds) != mech_position_z(mech),
                  "Get to same elevation before thinking about entering!");
  DOCHECK_CONTEXT(
      context, fabsf(mech_vertical_speed(mech) - mech_vertical_speed(ds)) > 10,
      "Vertical speed difference is too great to enter safely!");
  DOCHECK_CONTEXT(context,
                  mech_class(mech) == CLASS_MECH &&
                      mech_movement_type(mech) != MOVE_QUAD &&
                      (IsMechLegLess(mech)),
                  "Without legs? Are you kidding?");
  ref = mech_bay_dbref(ds, bayn);
  map = btech_context_get_map(context, ref);

  DOCHECK_CONTEXT(context, !map, "You sense wrongness in fabric of space.");

  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_ENTER_HANGAR),
                  "You are already entering the hangar!");
  if (!lock_test(btech_context_evaluation(context), player, player,
                 mech_dbref(mech), ref, LUA_LOCK_ENTER,
                 LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock, &lock_result)) {
    char *msg = lock_result.has_enactor_message
                    ? lock_result.enactor_message
                    : "You are unable to enter the bay!";
    notify(btech_context_evaluation(context), player, msg);
    return;
  }
  DOCHECK_CONTEXT(
      context, !dropship_bay_is_enterable(mech, ds, mech_bay_dbref(ds, bayn)),
      "Someone else is using the door at the moment.");
  DOCHECK_CONTEXT(context,
                  !(map = btech_context_get_map(context, mech_map_dbref(mech))),
                  "You sense a wrongness in fabric of space.");
  HexLOSBroadcast(map, mech_position_x(mech), mech_position_y(mech),
                  "The bay doors at $h start to open..");
  mech_event_schedule(mech, EVENT_ENTER_HANGAR, mech_enterbay_event, 12, ref);
}

static void dropship_place_departing_unit(Mech *ds, Mech *mech, int frombay) {
  int i;
  int nx, ny;
  BattleMap *mech_map;

  for (i = 0; i < NUM_BAYS; i++)
    if (mech_bay_dbref(ds, i) == frombay)
      break;
  if (i == NUM_BAYS || !(mech_map = btech_context_get_map(
                             mech_context(mech), mech_map_dbref(mech)))) {
    /* i _should_ be set, otherwise things are deeply disturbing */
    mech_notify(mech, MECHALL, "Reality collapse imminent.");
    return;
  }
  i = dropship_bay_direction(ds, i);
  nx =
      dirs[(mech_dropship_bearing_sector(ds) + i) % 6][0] + mech_position_x(ds);
  ny = dirs[(mech_dropship_bearing_sector(ds) + i) % 6][1] +
       mech_position_y(ds) +
       dropship_hex_row_adjustment(mech_position_x(ds), nx);
  nx = BOUNDED(0, nx, battle_map_width(mech_map) - 1);
  ny = BOUNDED(0, ny, battle_map_height(mech_map) - 1);

  /* snippage from mech_Rsetxy */
  mech_position_xy_set(mech, nx, ny);
  mech_position_hex_z_set(mech, mech_position_z(ds));
  mech_position_elevation_set(mech, mech_position_elevation(ds));
  float real_x, real_y;
  MapCoordToRealCoord(nx, ny, &real_x, &real_y);
  mech_position_real_xy_set(mech, real_x, real_y);
  mech_position_terrain_set(mech, map_terrain_get(mech_map, nx, ny));
}

static int dropship_leave_bay(BattleMap *map, Mech *ds, Mech *mech,
                              DbRef frombay) {
  Mech *car = nullptr;
  BtechContext *context = mech_context(mech);

  StopBSuitSwarmers(btech_context_find_object(context, mech_map_dbref(mech)),
                    mech, 1);
  mech_los_broadcast(mech, "has left the bay.");
  /* We escape confines of the bay to open air/land! */
  mech_Rsetmapindex(GOD, (void *)mech, tprintf("%ld", mech_map_dbref(ds)));
  if (mech_carried_dbref(mech) > 0)
    car = btech_context_get_mech(context, mech_carried_dbref(mech));
  if (car)
    mech_Rsetmapindex(GOD, (void *)car, tprintf("%ld", mech_map_dbref(ds)));
  DOCHECKMA0(mech_map_dbref(mech) == battle_map_dbref(map),
             "Fatal error: Unable to find the map 'ship is on.");
  move_via_teleport(btech_context_evaluation(context), mech_dbref(mech),
                    mech_map_dbref(mech), 1, 0);
  if (car)
    move_via_teleport(btech_context_evaluation(context), mech_dbref(car),
                      mech_map_dbref(mech), 1, 0);
  mech_notify(mech, MECHALL, "You have left the bay.");
  dropship_place_departing_unit(ds, mech, frombay);
  if (car) {
    mech_position_mirror(car, mech, 0);
    MarkForLOSUpdate(car);
    mech_flood(car);
  }
  mech_los_broadcast_unit(mech, ds, "has left %s's bay.");
  mech_notify(ds, MECHALL,
              tprintf("%s has left the bay.", mech_display_id(mech).text));
  mech_continue_flying(mech);
  if (is_in_character(btech_context_database(context), mech_dbref(mech)) &&
      game_object_location(btech_context_database(context),
                           mech_pilot_dbref(mech)) != mech_dbref(mech)) {
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

int dropship_leave(BattleMap *map, Mech *mech) {
  Mech *car;

  DOCHECKMA0(!(car = btech_context_get_mech(mech_context(mech),
                                            battle_map_parent_dbref(map))),
             "Invalid : No parent object?");
  DOCHECKMA0(!dropship_bay_is_open(mech, car, battle_map_dbref(map)),
             "The door has been jammed!");
  DOCHECKMA0(!mech_is_landed(car) && !mech_is_flying_type(mech),
             "The 'ship is still airborne!");
  DOCHECKMA0(
      is_zombie(btech_context_database(mech_context(car)), mech_dbref(car)),
      "You don't feel leaving right now would be prudent..");
  return dropship_leave_bay(map, car, mech, battle_map_dbref(map));
}
