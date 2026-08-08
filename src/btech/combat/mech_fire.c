/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *
 *  Copyright (c) 2001-2002 Cord Awtry
 *       All rights reserved
 */

#include <stdint.h>

#include "btech_event.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"

#define VEHICLEBURN_TICK 60
#define VEHICLE_EXTINGUISH_TICK 120

static void inferno_end_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  mech_jellied_set(mech, false);
  mech_notify(mech, MECHALL,
              "You feel suddenly far cooler as the fires finally die.");
}

void mech_inferno_burn(Mech *mech, int time) {
  int l;

  if (!mech_is_jellied(mech)) {
    mech_jellied_set(mech, true);
    mech_event_schedule(mech, EVENT_BURN, inferno_end_event, time, 0);
    return;
  }

  l = mech_event_last_delay(mech, EVENT_BURN) + time;
  mech_event_cancel(mech, EVENT_BURN);
  mech_event_schedule(mech, EVENT_BURN, inferno_end_event, l, 0);
}

static void vehicle_burn_event(MuxEvent *objEvent) {
  Mech *objMech = (Mech *)objEvent->data; /* get the mech */
  const int wLoc = (int)(intptr_t)objEvent->data2;
  int wDamRoll;
  char strLocName[30];

  if (!objMech)
    return;
  wDamRoll = btech_random_range_int(mech_context(objMech), 1, 6);

  ArmorStringFromIndex(wLoc, strLocName, mech_class(objMech),
                       mech_movement_type(objMech));

  if (!mech_section_internal(objMech, wLoc))
    return;

  mech_printf(objMech, MECHALL,
              "[fg=red bold]Your %s takes damage from the fire![reset]",
              strLocName);
  DamageMech(objMech, objMech, 0, -1, wLoc, 0, 0, wDamRoll, 0, 0, 0, -1, 0, 1);

  /*
   * Only continue the event if the damage was greater than one
   */
  if ((wDamRoll > 1) && mech_section_internal(objMech, wLoc))
    mech_event_schedule(objMech, EVENT_VEHICLEBURN, vehicle_burn_event,
                        VEHICLEBURN_TICK, wLoc);
  else {
    if (mech_section_internal(objMech, wLoc))
      mech_printf(objMech, MECHALL,
                  "The fire burning on your %s finally goes out.", strLocName);
    if (!mech_event_count(objMech, EVENT_VEHICLEBURN))
      mech_los_broadcast(objMech, "is no longer engulfed in flames.");
  }
}

void vehicle_fire_start(Mech *objMech, Mech *objAttacker) {
  int wDamage = 0;
  char strLocName[30];

  if (!objAttacker)
    objAttacker = objMech;

  mech_notify(objMech, MECHALL, "You catch on fire!");
  mech_los_broadcast(objMech, "catches on fire!");

  for (int wIter = 0; wIter < NUM_SECTIONS; wIter++) {
    if (mech_section_internal(objMech, wIter) &&
        !mech_event_count_data(objMech, EVENT_VEHICLEBURN, wIter)) {
      wDamage = btech_random_range_int(mech_context(objMech), 1, 6);
      ArmorStringFromIndex(wIter, strLocName, mech_class(objMech),
                           mech_movement_type(objMech));
      mech_printf(objMech, MECHALL, "Your %s catches on fire!", strLocName);

      DamageMech(objMech, objAttacker, 0, -1, wIter, 0, 0, wDamage, 0, 0, 0, -1,
                 0, 1);
      mech_event_schedule(objMech, EVENT_VEHICLEBURN, vehicle_burn_event,
                          VEHICLEBURN_TICK, wIter);
    }
  }
}

void vehicle_fire_extinguish_event(MuxEvent *e) {
  Mech *objMech = (Mech *)e->data;

  if (!objMech)
    return;

  if (!mech_event_count(objMech, EVENT_VEHICLEBURN))
    return;

  mech_event_cancel(objMech, EVENT_VEHICLEBURN);

  mech_notify(objMech, MECHALL, "You manage to dowse the fire.");
  mech_los_broadcast(objMech, "is no longer engulfed in flames.");
}

void vehicle_fire_extinguish(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALS))
    return;

  if (mech_is_started(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your tank is started! You can not extinguish the "
                 "flames while your tank is started!");
    return;
  }
  if (!mech_event_count(mech, EVENT_VEHICLEBURN)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit is not on fire!");
    return;
  }
  if (mech_event_count(mech, EVENT_VEHICLE_EXTINGUISH)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're already trying to put out the fire!");
    return;
  }

  mech_notify(mech, MECHALL, "You begin to extinguish the fires!");

  mech_event_schedule(mech, EVENT_VEHICLE_EXTINGUISH,
                      vehicle_fire_extinguish_event, VEHICLE_EXTINGUISH_TICK,
                      0);
}

/*
 *  Mechs entering level 2 water, or proning in level 1 water should
 *  extinguish any inferno currently burning.
 */
void mech_inferno_extinguish_in_water(Mech *mech) {
  int elev = mech_position_elevation(mech);
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (mech_position_terrain(mech) != WATER || mech_class(mech) != CLASS_MECH ||
      !mech_is_jellied(mech) || (elev == -1 && !mech_is_fallen(mech)))
    return;

  mech_event_cancel(mech, EVENT_BURN);
  mech_jellied_set(mech, false);

  mech_notify(mech, MECHALL, "The flames extinguish in a roar of steam!");
  mech_los_broadcast(
      mech, "is surrounded by a plume of steam as the flames extinguish.");

  /* According to FASA, the inferno jelly should keep on burning on the
   * water hex. We'll just add some steam (smoke) instead. */
  add_decoration(map, mech_position_x(mech), mech_position_y(mech), TYPE_SMOKE,
                 SMOKE, 120);
}

void vehicle_fire_check(Mech *objMech, int fromHexFire) {
  int wRoll = btech_random_roll(mech_context(objMech));
  int wIter;
  int wDamage = 0;

  switch (mech_movement_type(objMech)) {
  case MOVE_WHEEL:
  case MOVE_VTOL:
    wRoll += 2;
    break;
  case MOVE_HOVER:
    wRoll += 4;
    break;
  case MOVE_BIPED:
  case MOVE_TRACK:
  case MOVE_HULL:
  case MOVE_FOIL:
  case MOVE_FLY:
  case MOVE_QUAD:
  case MOVE_SUB:
  case MOVE_NONE:
  default:
    break;
  }

  if (wRoll < 8) /* don't do jack if it's < 8 */
    return;

  if (fromHexFire)
    mech_notify(
        objMech, MECHALL,
        "[fg=red bold]You drive through a wall of searing flames![reset]");
  else
    mech_notify(objMech, MECHALL,
                "[fg=red bold]The fires surround your vehicle![reset]");

  switch (wRoll) {
  case 8: /* roll once on the motive system chart */
  case 9:
    if (mech_class(objMech) == CLASS_VTOL) {
      /*
       * VTOLs _should_ make a pskill or go up one level... not right now tho
       */
    } else {
      mech_notify(objMech, MECHALL,
                  "[fg=red bold]The fire damages your motive system![reset]");
      mech_motive_system_hit(objMech, 0);
    }
    break;

  case 10:
  case 11:
    /*
     * Do 1d6 damage to each loc
     */
    mech_notify(
        objMech, MECHALL,
        "[fg=red bold]The fire sweeps across your unit damaging it![reset]");

    for (wIter = 0; wIter < NUM_SECTIONS; wIter++) {
      wDamage = btech_random_range_int(mech_context(objMech), 1, 6);

      if (mech_section_internal(objMech, wIter))
        DamageMech(objMech, objMech, 0, -1, wIter, 0, 0, wDamage, 0, 0, 0, -1,
                   0, 1);
    }
    break;

  default:
    vehicle_fire_start(objMech, objMech);
    break;
  }
}
