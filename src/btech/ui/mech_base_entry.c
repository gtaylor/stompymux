#include "autopilot.h"
#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "eject_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_obj_api.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_maps_api.h"
#include "mech_move_api.h"
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
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "mux/world/access.h"
#include "mux/world/move.h"
#include "registry_api.h"

#include "section_types.h"
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static void mech_enter_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  Mech *tmpm = nullptr;
  MapObject *mapo;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  BattleMap *newmap;
  const intptr_t TARGET_VALUE = e->secondary.integer;
  char target;
  int x;
  int y;
  int obj_x;
  int obj_y;
  LuaLockInvocation lock;

  if (TARGET_VALUE < CHAR_MIN || TARGET_VALUE > CHAR_MAX)
    return;
  target = (char)TARGET_VALUE;
  mapo = find_entrance_by_xy(map, mech_position_x(mech), mech_position_y(mech));
  if (!mapo)
    return;
  if (!mech_is_started(mech) || mech_pilot_is_unconscious(mech) ||
      mech_is_jumping(mech) ||
      (mech_class(mech) == CLASS_MECH &&
       (mech_is_fallen(mech) || mech_event_count(mech, EVENT_STAND))) ||
      mech_is_out_of_control(mech) ||
      (fabsf(mech_current_speed(mech)) * 5.0F >=
           mech_cargo_maximum_speed(mech, mech_maximum_speed(mech)) &&
       fabsf(mech_cargo_maximum_speed(mech, mech_maximum_speed(mech))) >=
           MP1) ||
      (mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0))
    return;
  newmap = btech_context_get_map(mech_context(mech), mapo->obj);
  if (!newmap)
    return;
  MapEntranceResult entrance = find_entrance(newmap, target);
  if (!entrance.found)
    return;
  x = entrance.position.x;
  y = entrance.position.y;
  LuaLockResult *lock_result = checked_storage_allocate(sizeof(*lock_result));

  if (!lock_test(btech_context_evaluation(mech_context(mech)), mech_dbref(mech),
                 mech_dbref(mech), mech_dbref(mech), newmap->mynum,
                 LUA_LOCK_ENTER, false, &lock, lock_result) &&
      (battle_map_build_is_safe(newmap) || newmap->cf >= (newmap->cfmax / 2))) {
    const char *msg = lock_result->has_enactor_message
                          ? lock_result->enactor_message
                          : "The hangar is locked.";
    mech_notify(mech, MECHALL, msg);
    goto cleanup;
  }

  if (mech_class(mech) == CLASS_MW &&
      !is_in_character(mech_context(mech)->database, mapo->obj)) {
    bsuit_swarmers_stop(
        btech_context_find_object(mech_context(mech), mech_map_dbref(mech)),
        mech, 1);
    mech_printf(mech, MECHALL, "You enter %s.",
                structure_name(mech_context(mech)->database, mapo).text);
    mech_los_broadcastf(mech, "has entered %s at %d,%d.",
                        structure_name(mech_context(mech)->database, mapo).text,
                        mech_position_x(mech), mech_position_y(mech));
    mark_for_los_update(mech);
    enter_mw_bay(mech, mapo->obj);
    goto cleanup;
  }
  if (mech_carried_dbref(mech) > 0)
    tmpm = btech_context_get_mech(mech_context(mech), mech_carried_dbref(mech));
  Mech *const UNITS[] = {mech, tmpm};
  const MechMapSetBatchRequest MAP_PLACEMENT = {
      .mechs = UNITS, .count = tmpm ? 2 : 1, .map = mapo->obj};
  if (mech_map_index_preflight_batch(&MAP_PLACEMENT) != MECH_MAP_SET_OK) {
    mech_notify(mech, MECHALL, "Unable to enter: unit placement failed.");
    goto cleanup;
  }
  obj_x = mech_position_x(mech);
  obj_y = mech_position_y(mech);
  const ObjectMovementRequest MOVEMENTS[] = {
      {.evaluation = btech_context_evaluation(mech_context(mech)),
       .object = mech_dbref(mech),
       .destination = mapo->obj,
       .cause = 1},
      {.evaluation = btech_context_evaluation(mech_context(mech)),
       .object = tmpm ? mech_dbref(tmpm) : NOTHING,
       .destination = mapo->obj,
       .cause = 1},
  };
  if (!move_via_teleport_batch(&(ObjectTeleportBatchRequest){
          .movements = MOVEMENTS, .count = tmpm ? 2 : 1})) {
    mech_notify(mech, MECHALL, "Unable to enter: teleportation was denied.");
    goto cleanup;
  }
  bsuit_swarmers_stop(
      btech_context_find_object(mech_context(mech), mech_map_dbref(mech)), mech,
      1);
  mech_printf(mech, MECHALL, "You enter %s.",
              structure_name(mech_context(mech)->database, mapo).text);
  mech_los_broadcastf(mech, "has entered %s at %d,%d.",
                      structure_name(mech_context(mech)->database, mapo).text,
                      mech_position_x(mech), mech_position_y(mech));
  mark_for_los_update(mech);
  if (mech_map_index_set_batch(&MAP_PLACEMENT) != MECH_MAP_SET_OK ||
      !mech_position_set(
          &(MechPositionSetRequest){.mech = mech, .x = x, .y = y})) {
    mech_notify(mech, MECHALL, "Unable to enter: unit placement failed.");
    goto cleanup;
  }
  mech_los_broadcastf(mech, "has entered %s at %d,%d.",
                      structure_name(mech_context(mech)->database, mapo).text,
                      obj_x, obj_y);
  if (tmpm) {
    mech_los_broadcastf(tmpm, "has entered %s at %d,%d.",
                        structure_name(mech_context(mech)->database, mapo).text,
                        obj_x, obj_y);
  }
  if (tmpm) {
    if (!mech_position_set(
            &(MechPositionSetRequest){.mech = tmpm, .x = x, .y = y})) {
      mech_notify(tmpm, MECHALL, "Unable to enter: unit placement failed.");
      goto cleanup;
    }
  }
  auto_cal_mapindex(mech_context(mech), mech);

cleanup:
  free_buf(lock_result);
}

void mech_enterbase(DbRef player, Mech *mech, char *buffer) {
  BattleMap *map;
  BattleMap *newmap;
  MapObject *mapo;
  char target;
  char *tmpc;
  char *args[2];
  int argc;

  char fail_mesg[SBUF_SIZE];
  LuaLockInvocation lock;

  argc = mech_parseattributes(buffer, args, 2);
  if (argc > 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid arguments to command!");
    return;
  }
  tmpc = argc > 0 ? args[0] : nullptr;
  if (tmpc != nullptr && *tmpc && *checked_string_suffix(tmpc, 1) == '\0')
    target = ascii_to_lower(*tmpc);
  else
    target = 0;
  if (!common_checks(player, mech, MECH_USUAL))
    return;
  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  /* For now, no dir checks */
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "While in mid-jump? No way.");
    return;
  }
  if (mech_class(mech) == CLASS_MECH &&
      (mech_is_fallen(mech) || mech_event_count(mech, EVENT_STAND))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Crawl inside? I think not. Stand first.");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "While in mid-flight? No way.");
    return;
  }
  if (mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You lack fuel to maneuver in!");
    return;
  }
  if (mech_is_flying_type(mech) && !mech_is_landed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You need to land before you can enter the hangar.");
    return;
  }
  if (mech_is_dropship(mech)) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "Heh, you're trying to be funny, right, a DropShip entering hangar?");
    return;
  }
  if (fabsf(mech_current_speed(mech)) * 5.0F >=
          mech_cargo_maximum_speed(mech, mech_maximum_speed(mech)) &&
      fabsf(mech_cargo_maximum_speed(mech, mech_maximum_speed(mech))) >= MP1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are moving too fast to enter the hangar!");
    return;
  }
  mapo = find_entrance_by_xy(map, mech_position_x(mech), mech_position_y(mech));
  if (!mapo) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You see nothing to enter here!");
    return;
  }
  /* Wow, *gasp*, we got something to enter */
  newmap = btech_context_find_object(mech_context(mech), mapo->obj);
  if (!newmap) {
    mech_notify(mech, MECHALL, "You sense wrongness in fabric of space..");
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS,
        "Error: No map existing for mapindex #%d (@ %d,%d of #%ld)",
        (int)mapo->obj, mapo->x, mapo->y, mech_map_dbref(mech));
    return;
  }
  MapEntranceResult entrance = find_entrance(newmap, target);
  if (!entrance.found) {
    mech_notify(mech, MECHALL, "You sense wrongness in fabric of space..");
    btech_channel_send(
        mech_context(mech), BTECH_CHANNEL_MECH_ERRORS,
        "Error: No entrance existing for mapindex #%d (@ %d,%d of #%ld)",
        (int)mapo->obj, mapo->x, mapo->y, mech_map_dbref(mech));
    return;
  }
  LuaLockResult *lock_result = checked_storage_allocate(sizeof(*lock_result));
  if (!lock_test(btech_context_evaluation(mech_context(mech)), player, player,
                 mech_dbref(mech), newmap->mynum, LUA_LOCK_ENTER, false, &lock,
                 lock_result) &&
      (battle_map_build_is_safe(newmap) || newmap->cf >= (newmap->cfmax / 2))) {
    /* Trigger FAIL & AFAIL */
    memset(fail_mesg, 0, sizeof(fail_mesg));
    (void)snprintf(fail_mesg, SBUF_SIZE, "The hangar is locked.");

    notify_lock_failure(&(LockFailureNotification){
        .evaluation = btech_context_evaluation(mech_context(mech)),
        .invocation = &lock,
        .result = lock_result,
        .enactor_default = fail_mesg,
        .event = LUA_EVENT_FAIL});

    goto cleanup;
  }

  if (mech_event_count(mech, EVENT_ENTER_HANGAR)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are already entering the hangar!");
    goto cleanup;
  }
  /* XXX Check for other mechs in the hex possibly doing this as well (ick) */
  hex_los_broadcast(map, mech_position_x(mech), mech_position_y(mech),
                    "The doors at $h start to open..");
  mech_event_schedule(mech, EVENT_ENTER_HANGAR, mech_enter_event, 18,
                      (long)target);

cleanup:
  free_buf(lock_result);
}
