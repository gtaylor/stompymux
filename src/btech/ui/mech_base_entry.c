#include "autopilot.h"
#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "eject_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_obj_api.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/commands/action_messages.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/world/access.h"
#include "mux/world/move.h"
#include "registry_api.h"

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

static void mech_enter_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data, *tmpm = nullptr;
  MapObject *mapo;
  BattleMap *map =
                btech_context_get_map(mech_context(mech), mech_map_dbref(mech)),
            *newmap;
  long target = (long)e->data2;
  int x, y;
  int obj_x, obj_y;
  LuaLockInvocation lock;
  LuaLockResult lock_result;

  if (!(mapo = find_entrance_by_xy(map, mech_position_x(mech),
                                   mech_position_y(mech))))
    return;
  if (!mech_is_started(mech) || mech_pilot_is_unconscious(mech) ||
      mech_is_jumping(mech) ||
      (mech_class(mech) == CLASS_MECH &&
       (mech_is_fallen(mech) || mech_event_count(mech, EVENT_STAND))) ||
      mech_is_out_of_control(mech) ||
      (fabs(mech_current_speed(mech)) * 5 >=
           mech_cargo_maximum_speed(mech, mech_maximum_speed(mech)) &&
       fabs(mech_cargo_maximum_speed(mech, mech_maximum_speed(mech))) >= MP1) ||
      (mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0))
    return;
  if (!(newmap = btech_context_get_map(mech_context(mech), mapo->obj)))
    return;
  if (!find_entrance(newmap, target, &x, &y))
    return;

  if (!lock_test(btech_context_evaluation(mech_context(mech)), mech_dbref(mech),
                 mech_dbref(mech), mech_dbref(mech), newmap->mynum,
                 LUA_LOCK_ENTER, LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock,
                 &lock_result) &&
      (BuildIsSafe(newmap) || newmap->cf >= (newmap->cfmax / 2))) {
    char *msg = lock_result.has_enactor_message ? lock_result.enactor_message
                                                : "The hangar is locked.";
    mech_notify(mech, MECHALL, msg);
    return;
  }

  bsuit_swarmers_stop(
      btech_context_find_object(mech_context(mech), mech_map_dbref(mech)), mech,
      1);
  mech_printf(mech, MECHALL, "You enter %s.",
              structure_name(mech_context(mech)->database, mapo).text);
  mech_los_broadcast(
      mech, tprintf("has entered %s at %d,%d.",
                    structure_name(mech_context(mech)->database, mapo).text,
                    mech_position_x(mech), mech_position_y(mech)));
  MarkForLOSUpdate(mech);
  if (mech_class(mech) == CLASS_MW &&
      !is_in_character(mech_context(mech)->database, mapo->obj)) {
    enter_mw_bay(mech, mapo->obj);
    return;
  }
  if (mech_carried_dbref(mech) > 0)
    tmpm = btech_context_get_mech(mech_context(mech), mech_carried_dbref(mech));
  obj_x = mech_position_x(mech);
  obj_y = mech_position_y(mech);
  mech_Rsetmapindex(GOD, (void *)mech, tprintf("%d", (int)mapo->obj));
  mech_Rsetxy(GOD, (void *)mech, tprintf("%d %d", x, y));
  mech_los_broadcast(
      mech, tprintf("has entered %s at %d,%d.",
                    structure_name(mech_context(mech)->database, mapo).text,
                    obj_x, obj_y));
  if (tmpm)
    mech_los_broadcast(
        tmpm, tprintf("has entered %s at %d,%d.",
                      structure_name(mech_context(mech)->database, mapo).text,
                      obj_x, obj_y));
  move_via_teleport(btech_context_evaluation(mech_context(mech)),
                    mech_dbref(mech), mapo->obj, 1, 0);
  if (tmpm) {
    mech_Rsetmapindex(GOD, (void *)tmpm, tprintf("%d", (int)mapo->obj));
    mech_Rsetxy(GOD, (void *)tmpm, tprintf("%d %d", x, y));
    move_via_teleport(btech_context_evaluation(mech_context(mech)),
                      mech_dbref(tmpm), mapo->obj, 1, 0);
  }
  auto_cal_mapindex(mech_context(mech), mech);
}

void mech_enterbase(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *map, *newmap;
  int x, y;
  MapObject *mapo;
  char target, *tmpc;
  char *args[2];
  int argc;

  char fail_mesg[SBUF_SIZE];
  LuaLockInvocation lock;
  LuaLockResult lock_result;

  argc = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(mech_context(mech), argc > 1,
                  "Invalid arguments to command!");
  tmpc = args[0];
  if (argc > 0 && *tmpc && !(*(tmpc + 1)))
    target = tolower(*tmpc);
  else
    target = 0;
  cch(MECH_USUAL);
  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  /* For now, no dir checks */
  DOCHECK_CONTEXT(mech_context(mech), mech_is_jumping(mech),
                  "While in mid-jump? No way.");
  DOCHECK_CONTEXT(
      mech_context(mech),
      mech_class(mech) == CLASS_MECH &&
          (mech_is_fallen(mech) || mech_event_count(mech, EVENT_STAND)),
      "Crawl inside? I think not. Stand first.");
  DOCHECK_CONTEXT(mech_context(mech), mech_is_out_of_control(mech),
                  "While in mid-flight? No way.");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0,
                  "You lack fuel to maneuver in!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_is_flying_type(mech) && !mech_is_landed(mech),
                  "You need to land before you can enter the hangar.");
  DOCHECK_CONTEXT(
      mech_context(mech), mech_is_dropship(mech),
      "Heh, you're trying to be funny, right, a DropShip entering hangar?");
  DOCHECK_CONTEXT(
      mech_context(mech),
      fabs(mech_current_speed(mech)) * 5 >=
              mech_cargo_maximum_speed(mech, mech_maximum_speed(mech)) &&
          fabs(mech_cargo_maximum_speed(mech, mech_maximum_speed(mech))) >= MP1,
      "You are moving too fast to enter the hangar!");
  DOCHECK_CONTEXT(mech_context(mech),
                  !(mapo = find_entrance_by_xy(map, mech_position_x(mech),
                                               mech_position_y(mech))),
                  "You see nothing to enter here!");
  /* Wow, *gasp*, we got something to enter */
  if (!(newmap = btech_context_find_object(mech_context(mech), mapo->obj))) {
    mech_notify(mech, MECHALL, "You sense wrongness in fabric of space..");
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Error: No map existing for mapindex #%d (@ %d,%d of #%ld)",
                (int)mapo->obj, mapo->x, mapo->y, mech_map_dbref(mech)));
    return;
  }
  if (!find_entrance(newmap, target, &x, &y)) {
    mech_notify(mech, MECHALL, "You sense wrongness in fabric of space..");
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf(
            "Error: No entrance existing for mapindex #%d (@ %d,%d of #%ld)",
            (int)mapo->obj, mapo->x, mapo->y, mech_map_dbref(mech)));
    return;
  }

  if (!lock_test(btech_context_evaluation(mech_context(mech)), player, player,
                 mech_dbref(mech), newmap->mynum, LUA_LOCK_ENTER,
                 LUA_LOCK_OPERATION_BTECH_ENTER, false, &lock, &lock_result) &&
      (BuildIsSafe(newmap) || newmap->cf >= (newmap->cfmax / 2))) {

    /* Trigger FAIL & AFAIL */
    memset(fail_mesg, 0, sizeof(fail_mesg));
    snprintf(fail_mesg, SBUF_SIZE, "The hangar is locked.");

    notify_lock_failure(btech_context_evaluation(mech_context(mech)), &lock,
                        &lock_result, fail_mesg, nullptr, LUA_EVENT_FAIL);

    return;
  }

  DOCHECK_CONTEXT(mech_context(mech),
                  mech_event_count(mech, EVENT_ENTER_HANGAR),
                  "You are already entering the hangar!");
  /* XXX Check for other mechs in the hex possibly doing this as well (ick) */
  HexLOSBroadcast(map, mech_position_x(mech), mech_position_y(mech),
                  "The doors at $h start to open..");
  mech_event_schedule(mech, EVENT_ENTER_HANGAR, mech_enter_event, 18,
                      (long)target);
}
