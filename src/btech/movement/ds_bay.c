/* Implements BattleTech movement mechanics for dropship bay. */

#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "ds_bay_api.h"
#include "eject_api.h"
#include "equipment_types.h"
#include "map_coordinates.h"
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
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_startup_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/world/access.h"
#include "mux/world/move.h"
#include "registry_api.h"
#include "section_types.h"

static char *bay_argument(char **arguments, size_t count, int index) {
  if (index < 0)
    abort();
  char **slot = (char **)checked_storage_at((void *)arguments, count,
                                            sizeof(*arguments), (size_t)index);
  return *slot;
}

static int dropship_direction_section(int direction) {
  switch (direction % 6) {
  case 0:
    return DS_NOSE;
  case 1:
    return DS_RWING;
  case 2:
    return DS_RRWING;
  case 3:
    return DS_AFT;
  case 4:
    return DS_LRWING;
  case 5:
    return DS_LWING;
  default:
    abort();
  }
}

static int dropship_direction_x(int direction) {
  switch (direction % 6) {
  case 0:
  case 3:
    return 0;
  case 1:
  case 2:
    return 1;
  case 4:
  case 5:
    return -1;
  default:
    abort();
  }
}

static int dropship_direction_y(int direction) {
  switch (direction % 6) {
  case 0:
    return -1;
  case 1:
  case 5:
    return 0;
  case 2:
  case 3:
  case 4:
    return 1;
  default:
    abort();
  }
}

void mech_createbays(DbRef player, void *data, char *buffer) {
  char *args[NUM_BAYS + 1];
  int argc;
  DbRef it;
  int i;
  Mech *ds = (Mech *)data;
  BattleMap *map;
  BtechContext *context = mech_context(ds);

  argc = mech_parseattributes(buffer, args, NUM_BAYS + 1);
  if (argc == NUM_BAYS + 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments!");
    return;
  }
  for (i = 0; i < argc; i++) {
    it = match_thing(&btech_context_command(context)->match, player,
                     bay_argument(args, NUM_BAYS + 1, i));
    if (it == NOTHING) {
      mecha_notifyf(btech_context_evaluation(context), player,
                    "Argument %d is invalid.", i + 1);
      return;
    }
    if (!btech_context_is_map(context, it)) {
      mecha_notifyf(btech_context_evaluation(context), player,
                    "Argument %d is not a map.", i + 1);
      return;
    }
    map = btech_context_find_object(context, it);
    mech_bay_dbref_set(
        &(MechBayAssignment){.mech = ds, .bay = i, .bay_dbref = it});
    battle_map_parent_dbref_set(map, mech_dbref(ds));
  }
  for (i = argc; i < NUM_BAYS; i++)
    mech_bay_dbref_set(
        &(MechBayAssignment){.mech = ds, .bay = i, .bay_dbref = -1});
  notify_printf(btech_context_evaluation(context), player, "%d bay(s) set up!",
                argc);
}

int dropship_bay_number(Mech *ds, int dir) {
  int bayn = 0;
  int i;
  int j;

  for (i = 0; i <= dir; i++) {
    for (j = 0; j < NUM_CRITICALS; j++) {
      if (mech_critical_part_type(ds, dropship_direction_section(i), j) ==
              special_equipment_index(DS_MECHDOOR) ||
          mech_critical_part_type(ds, dropship_direction_section(i), j) ==
              special_equipment_index(DS_AERODOOR))
        break;
    }
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

bool dropship_bay_in_adjacent_hex(Mech *seer, Mech *ds, int *bayn) {
  int i;
  int t = mech_dropship_bearing_sector(ds);

  for (i = t; i < (t + 6); i++) {

    int bay_x = mech_position_x(ds) + dropship_direction_x(i);
    int bay_y = mech_position_y(ds) + dropship_direction_y(i) +
                dropship_hex_row_adjustment(mech_position_x(ds), bay_x);
    if (bay_x == mech_position_x(seer) && bay_y == mech_position_y(seer)) {
      *bayn = dropship_bay_number(ds, ((i - t + 6) % 6));
      return *bayn >= 0;
    }
  }
  return false;
}

static int dropship_find_single_adjacent_bay(Mech *mech, DbRef *ref,
                                             int *bayn) {
  BattleMap *map =
      btech_context_find_object(mech_context(mech), mech_map_dbref(mech));
  int loop;
  Mech *temp_mech;
  int count = 0;

  *ref = 0;
  if (!map)
    return 0;
  for (loop = 0; loop < battle_map_unit_count(map); loop++) {
    if (battle_map_unit_dbref(map, loop) >= 0) {
      temp_mech = btech_context_get_mech(mech_context(mech),
                                         battle_map_unit_dbref(map, loop));
      if (!temp_mech)
        continue;
      if (!mech_is_dropship(temp_mech))
        continue;
      if (!mech_is_landed(temp_mech))
        continue; /* This might break midflight-aero-DS-docking. But aeros are
                     broken anyway. */
      if (dropship_bay_in_adjacent_hex(mech, temp_mech, bayn)) {
        if (count++)
          *ref = -1;
        else
          *ref = mech_dbref(temp_mech);
      }
    }
  }
  return count;
}

static void mech_enterbay_event(MuxEvent *e) {
  char message_buffer[128];
  Mech *mech = (Mech *)e->data;
  Mech *ds;
  Mech *tmpm = nullptr;
  DbRef ref = (DbRef)(intptr_t)e->data2;
  int bayn;
  int x = 5;
  int y = 5;
  BattleMap *tmpmap;
  BtechContext *context = mech_context(mech);

  if (!mech_is_started(mech) || mech_pilot_is_unconscious(mech) ||
      mech_is_jumping(mech) ||
      (mech_class(mech) == CLASS_MECH &&
       (mech_is_fallen(mech) || mech_event_count(mech, EVENT_STAND))) ||
      mech_is_out_of_control(mech) ||
      (fabsf(mech_current_speed(mech)) * 5.0F >=
           mech_effective_maximum_speed(mech) &&
       fabsf(mech_effective_maximum_speed(mech)) >= MP1) ||
      (mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0))
    return;
  tmpmap = btech_context_get_map(context, ref);
  ds = btech_context_get_mech(context, battle_map_parent_dbref(tmpmap));
  if (!ds)
    return;
  if (!dropship_bay_in_adjacent_hex(mech, ds, &bayn))
    return;
  /* whee */
  ref = mech_bay_dbref(ds, bayn);
  if (mech_class(mech) == CLASS_MW &&
      !is_in_character(btech_context_database(context), ref)) {
    bsuit_swarmers_stop(
        btech_context_find_object(context, mech_map_dbref(mech)), mech, 1);
    mech_notify(mech, MECHALL, "You enter the bay.");
    mech_los_broadcastf(mech, "has entered %s at %d,%d.",
                        mech_display_id(ds).text, mech_position_x(mech),
                        mech_position_y(mech));
    mark_for_los_update(mech);
    enter_mw_bay(mech, ref);
    return;
  }
  if (mech_carried_dbref(mech) > 0)
    tmpm = btech_context_get_mech(context, mech_carried_dbref(mech));
  const ObjectMovementRequest MOVEMENTS[] = {
      {.evaluation = btech_context_evaluation(context),
       .object = mech_dbref(mech),
       .destination = ref,
       .cause = 1},
      {.evaluation = btech_context_evaluation(context),
       .object = tmpm ? mech_dbref(tmpm) : NOTHING,
       .destination = ref,
       .cause = 1},
  };
  if (!move_via_teleport_batch(&(ObjectTeleportBatchRequest){
          .movements = MOVEMENTS, .count = tmpm ? 2 : 1})) {
    mech_notify(mech, MECHALL, "Unable to enter: teleportation was denied.");
    return;
  }
  bsuit_swarmers_stop(btech_context_find_object(context, mech_map_dbref(mech)),
                      mech, 1);
  mech_notify(mech, MECHALL, "You enter the bay.");
  mech_los_broadcastf(mech, "has entered %s at %d,%d.",
                      mech_display_id(ds).text, mech_position_x(mech),
                      mech_position_y(mech));
  mark_for_los_update(mech);
  (void)snprintf(message_buffer, sizeof(message_buffer), "%ld", ref);
  mech_rsetmapindex(GOD, (void *)mech, message_buffer);
  (void)snprintf(message_buffer, sizeof(message_buffer), "%d %d", x, y);
  mech_rsetxy(GOD, (void *)mech, message_buffer);
  mech_los_broadcast(mech, "has entered the bay.");
  if (tmpm) {
    (void)snprintf(message_buffer, sizeof(message_buffer), "%ld", ref);
    mech_rsetmapindex(GOD, (void *)tmpm, message_buffer);
    (void)snprintf(message_buffer, sizeof(message_buffer), "%d %d", x, y);
    mech_rsetxy(GOD, (void *)tmpm, message_buffer);
  }
}

static bool dropship_bay_is_open(Mech *mech, Mech *ds, DbRef bayref) {
  int i;
  int j;

  for (i = 0; i < NUM_BAYS; i++) {
    if (mech_bay_dbref(ds, i) > 0) {
      if (mech_bay_dbref(ds, i) == bayref) {
        j = dropship_bay_direction(ds, i);
        for (i = 0; i < NUM_CRITICALS; i++) {
          if (((mech_is_aerospace_unit(mech) &&
                mech_critical_part_type(ds, dropship_direction_section(j), i) ==
                    special_equipment_index(DS_AERODOOR)) ||
               (!mech_is_aerospace_unit(mech) &&
                mech_critical_part_type(ds, dropship_direction_section(j), i) ==
                    special_equipment_index(DS_MECHDOOR))) &&
              !mech_critical_is_destroyed(ds, dropship_direction_section(j), i))
            return true;
        }
        return false;
      }
    }
  }
  return false;
}

typedef struct DropshipBayEntryRequest {
  Mech *dropship;
  DbRef bay;
} DropshipBayEntryRequest;

static bool dropship_bay_is_enterable(const DropshipBayEntryRequest *request) {
  Mech *ds = request->dropship;
  const DbRef BAYREF = request->bay;
  int i;

  for (i = 0; i < NUM_BAYS; i++) {
    if (mech_bay_dbref(ds, i) > 0) {
      if (mech_bay_dbref(ds, i) == BAYREF)
        return (btech_context_event_data_count(mech_context(ds),
                                               EVENT_ENTER_HANGAR, BAYREF) > 0
                    ? 0
                    : 1) != 0;
    }
  }
  return false;
}

/* ID / Number, both optional (this _will_ be painful) */

void mech_enterbay(DbRef player, void *data, char *buffer) {
  char *args[3];
  int argc;
  DbRef ref = -1;
  int bayn = -1;
  Mech *mech = data;
  Mech *ds;
  BattleMap *map;
  LuaLockInvocation lock;
  LuaLockResult lock_result;
  BtechContext *context = mech_context(mech);

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  if (mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You lack fuel to maneuver in!");
    return;
  }
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "While in mid-jump? No way.");
    return;
  }
  if (mech_class(mech) == CLASS_MECH &&
      (mech_is_fallen(mech) || mech_event_count(mech, EVENT_STAND))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Crawl inside? I think not. Stand first.");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "While in mid-flight? No way.");
    return;
  }
  argc = mech_parseattributes(buffer, args, 2);
  if (argc == 2) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Hmm, invalid number of arguments?");
    return;
  }
  if (argc > 0) {
    ref = find_target_dbref_from_map_number(mech, args[0]);
    if (ref <= 0) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Invalid target!");
      return;
    }
  }
  if (ref < 0) {
    if (!dropship_find_single_adjacent_bay(mech, &ref, &bayn)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "No DS bay found in your hex!");
      return;
    }
    if (ref < 0) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Multiple enterable things found ; use the id for "
                   "specifying which you want.");
      return;
    }
    ds = btech_context_get_mech(context, ref);
    if (!ds) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You sense wrongness in fabric of space.");
      return;
    }
  } else {
    ds = btech_context_get_mech(context, ref);
    if (!ds) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You sense wrongness in fabric of space.");
      return;
    }
    if (!dropship_bay_in_adjacent_hex(mech, ds, &bayn)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You see no bays in your hex.");
      return;
    }
  }
  if (mech_is_dropship(mech) &&
      !(mech_technology_flags_secondary(mech) & CARRIER_TECH)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your craft can't enter bays.");
    return;
  }
  if (!dropship_bay_is_open(mech, ds, mech_bay_dbref(ds, bayn))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The door has been jammed!");
    return;
  }
  if (mech_is_dropship(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your unit is a bit too large to fit in there.");
    return;
  }
  if (fabsf(mech_current_speed(mech) - mech_current_speed(ds)) > MP1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Speed difference's too large to enter!");
    return;
  }
  if (mech_position_z(ds) != mech_position_z(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Get to same elevation before thinking about entering!");
    return;
  }
  if (fabsf(mech_vertical_speed(mech) - mech_vertical_speed(ds)) > 10) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Vertical speed difference is too great to enter safely!");
    return;
  }
  if (mech_class(mech) == CLASS_MECH && mech_movement_type(mech) != MOVE_QUAD &&
      (is_mech_leg_less(mech))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Without legs? Are you kidding?");
    return;
  }
  ref = mech_bay_dbref(ds, bayn);
  map = btech_context_get_map(context, ref);

  if (!map) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You sense wrongness in fabric of space.");
    return;
  }

  if (mech_event_count(mech, EVENT_ENTER_HANGAR)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are already entering the hangar!");
    return;
  }
  if (!lock_test(btech_context_evaluation(context), player, player,
                 mech_dbref(mech), ref, LUA_LOCK_ENTER,
                 LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock, &lock_result)) {
    const char *msg = lock_result.has_enactor_message
                          ? lock_result.enactor_message
                          : "You are unable to enter the bay!";
    mecha_notify(btech_context_evaluation(context), player, msg);
    return;
  }
  if (!dropship_bay_is_enterable(&(DropshipBayEntryRequest){
          .dropship = ds,
          .bay = mech_bay_dbref(ds, bayn),
      })) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Someone else is using the door at the moment.");
    return;
  }
  map = btech_context_get_map(context, mech_map_dbref(mech));
  if (!map) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You sense a wrongness in fabric of space.");
    return;
  }
  hex_los_broadcast(map, mech_position_x(mech), mech_position_y(mech),
                    "The bay doors at $h start to open..");
  mech_event_schedule(mech, EVENT_ENTER_HANGAR, mech_enterbay_event, 12, ref);
}

static void dropship_place_departing_unit(Mech *ds, Mech *mech, DbRef frombay) {
  int i;
  int nx;
  int ny;
  BattleMap *mech_map = nullptr;

  for (i = 0; i < NUM_BAYS; i++)
    if (mech_bay_dbref(ds, i) == frombay)
      break;
  if (i != NUM_BAYS)
    mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (i == NUM_BAYS || !mech_map) {
    /* i _should_ be set, otherwise things are deeply disturbing */
    mech_notify(mech, MECHALL, "Reality collapse imminent.");
    return;
  }
  i = dropship_bay_direction(ds, i);
  const int DIRECTION = (mech_dropship_bearing_sector(ds) + i) % 6;
  nx = dropship_direction_x(DIRECTION) + mech_position_x(ds);
  ny = dropship_direction_y(DIRECTION) + mech_position_y(ds) +
       dropship_hex_row_adjustment(mech_position_x(ds), nx);
  nx = bounded(0, nx, battle_map_width(mech_map) - 1);
  ny = bounded(0, ny, battle_map_height(mech_map) - 1);

  /* snippage from mech_Rsetxy */
  mech_position_xy_set(mech, nx, ny);
  mech_position_hex_z_set(mech, mech_position_z(ds));
  float real_x;
  float real_y;
  map_coord_to_real_coord(nx, ny, &real_x, &real_y);
  mech_position_real_xy_set(mech, (MapRealPosition){.x = real_x, .y = real_y});
}

static bool dropship_leave_bay(BattleMap *map, Mech *ds, Mech *mech,
                               DbRef frombay) {
  char message_buffer[128];
  Mech *car = nullptr;
  BtechContext *context = mech_context(mech);
  const DbRef DESTINATION = mech_map_dbref(ds);

  if (mech_carried_dbref(mech) > 0)
    car = btech_context_get_mech(context, mech_carried_dbref(mech));
  if (DESTINATION == battle_map_dbref(map)) {
    mech_notify(mech, MECHALL,
                "Fatal error: Unable to find the map 'ship is on.");
    return false;
  }
  const ObjectMovementRequest MOVEMENTS[] = {
      {.evaluation = btech_context_evaluation(context),
       .object = mech_dbref(mech),
       .destination = DESTINATION,
       .cause = 1},
      {.evaluation = btech_context_evaluation(context),
       .object = car ? mech_dbref(car) : NOTHING,
       .destination = DESTINATION,
       .cause = 1},
  };
  if (!move_via_teleport_batch(&(ObjectTeleportBatchRequest){
          .movements = MOVEMENTS, .count = car ? 2 : 1})) {
    mech_notify(mech, MECHALL,
                "Unable to leave the bay: teleportation was denied.");
    return false;
  }
  bsuit_swarmers_stop(btech_context_find_object(context, mech_map_dbref(mech)),
                      mech, 1);
  mech_los_broadcast(mech, "has left the bay.");
  (void)snprintf(message_buffer, sizeof(message_buffer), "%ld", DESTINATION);
  /* We escape confines of the bay to open air/land! */
  mech_rsetmapindex(GOD, (void *)mech, message_buffer);
  if (car)
    mech_rsetmapindex(GOD, (void *)car, message_buffer);
  mech_notify(mech, MECHALL, "You have left the bay.");
  dropship_place_departing_unit(ds, mech, frombay);
  if (car) {
    mech_position_mirror(car, mech, 0);
    mark_for_los_update(car);
    mech_flood(car);
  }
  mech_los_broadcast_unit(mech, ds, "has left %s's bay.");
  mech_printf(ds, MECHALL, "%s has left the bay.", mech_display_id(mech).text);
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
  return true;
}

bool dropship_leave(BattleMap *map, Mech *mech) {
  Mech *car;

  car =
      btech_context_get_mech(mech_context(mech), battle_map_parent_dbref(map));
  if (!car) {
    mech_notify(mech, MECHALL, "Invalid : No parent object?");
    return false;
  }
  if (!dropship_bay_is_open(mech, car, battle_map_dbref(map))) {
    mech_notify(mech, MECHALL, "The door has been jammed!");
    return false;
  }
  if (!mech_is_landed(car) && !mech_is_flying_type(mech)) {
    mech_notify(mech, MECHALL, "The 'ship is still airborne!");
    return false;
  }
  if (is_zombie(btech_context_database(mech_context(car)), mech_dbref(car))) {
    mech_notify(mech, MECHALL,
                "You don't feel leaving right now would be prudent..");
    return false;
  }
  return dropship_leave_bay(map, car, mech, battle_map_dbref(map));
}
