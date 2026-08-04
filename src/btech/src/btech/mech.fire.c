/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *
 *  Copyright (c) 2001-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech.h"
#include "btech_event.h"
#include "macros.h"
#include "map.h"
#include "map.terrain.h"
#include "mech.events.h"
#include "mech.lifecycle.h"
#include "mech.notify.h"
#include "mux/network/mux_event.h"
#include "mux/server/platform.h"
#include "p.glue.h"
#include "p.map.obj.h"
#include "p.mech.damage.h"
#include "p.mech.fire.h"
#include "p.mech.hitloc.h"
#include "p.mech.notify.h"
#include "p.mech.utils.h"

#define VEHICLEBURN_TICK 60
#define VEHICLE_EXTINGUISH_TICK 120

static void inferno_end_event(MuxEvent *e) {
  MECH *mech = (MECH *)e->data;

  MechCritStatus(mech) &= ~JELLIED;
  mech_notify(mech, MECHALL,
              "You feel suddenly far cooler as the fires finally die.");
}

void inferno_burn(MECH *mech, int time) {
  int l;

  if (!(MechCritStatus(mech) & JELLIED)) {
    MechCritStatus(mech) |= JELLIED;
    mech_event_schedule(mech, EVENT_BURN, inferno_end_event, time, 0);
    return;
  }

  l = mux_event_last_type_data(mech->xcode.context->events, EVENT_BURN,
                               (void *)mech) +
      time;
  mux_event_remove_type_data(mech->xcode.context->events, EVENT_BURN,
                             (void *)mech);
  mech_event_schedule(mech, EVENT_BURN, inferno_end_event, l, 0);
}

static void vehicle_burn_event(MuxEvent *objEvent) {
  MECH *objMech = (MECH *)objEvent->data; /* get the mech */
  long wLoc = (long)objEvent->data2;      /* and now the loc to damage */
  int wDamRoll;
  char strLocName[30];

  if (!objMech)
    return;
  wDamRoll = btech_random_range(objMech->xcode.context, 1, 6);

  ArmorStringFromIndex(wLoc, strLocName, MechType(objMech), MechMove(objMech));

  if (!GetSectInt(objMech, wLoc)) /* if our loc is gone, no damage to do */
    return;

  mech_printf(objMech, MECHALL,
              "[fg=red bold]Your %s takes damage from the fire![reset]",
              strLocName);
  DamageMech(objMech, objMech, 0, -1, wLoc, 0, 0, wDamRoll, 0, 0, 0, -1, 0, 1);

  /*
   * Only continue the event if the damage was greater than one
   */
  if ((wDamRoll > 1) && (GetSectInt(objMech, wLoc)))
    mech_event_schedule(objMech, EVENT_VEHICLEBURN, vehicle_burn_event,
                        VEHICLEBURN_TICK, wLoc);
  else {
    if (GetSectInt(objMech, wLoc))
      mech_printf(objMech, MECHALL,
                  "The fire burning on your %s finally goes out.", strLocName);
    if (!mech_event_count(objMech, EVENT_VEHICLEBURN))
      MechLOSBroadcast(objMech, "is no longer engulfed in flames.");
  }
}

void vehicle_start_burn(MECH *objMech, MECH *objAttacker) {
  long wIter;
  long wDamage = 0;
  char strLocName[30];

  if (!objAttacker)
    objAttacker = objMech;

  mech_notify(objMech, MECHALL, "You catch on fire!");
  MechLOSBroadcast(objMech, "catches on fire!");

  for (wIter = 0; wIter < NUM_SECTIONS; wIter++) {
    if (GetSectInt(objMech, wIter) &&
        !mech_event_count_data(objMech, EVENT_VEHICLEBURN, wIter)) {
      wDamage = btech_random_range(objMech->xcode.context, 1, 6);
      ArmorStringFromIndex(wIter, strLocName, MechType(objMech),
                           MechMove(objMech));
      mech_printf(objMech, MECHALL, "Your %s catches on fire!", strLocName);

      DamageMech(objMech, objAttacker, 0, -1, wIter, 0, 0, wDamage, 0, 0, 0, -1,
                 0, 1);
      mech_event_schedule(objMech, EVENT_VEHICLEBURN, vehicle_burn_event,
                          VEHICLEBURN_TICK, wIter);
    }
  }
}

void vehicle_extinquish_fire_event(MuxEvent *e) {
  MECH *objMech = (MECH *)e->data;

  if (!objMech)
    return;

  if (!mech_event_count(objMech, EVENT_VEHICLEBURN))
    return;

  mech_event_cancel(objMech, EVENT_VEHICLEBURN);

  mech_notify(objMech, MECHALL, "You manage to dowse the fire.");
  MechLOSBroadcast(objMech, "is no longer engulfed in flames.");
}

void vehicle_extinquish_fire(DbRef player, MECH *mech, char *buffer) {
  cch(MECH_USUALS);

  DOCHECK_CONTEXT(mech->xcode.context, Started(mech),
                  "Your tank is started! You can not extinguish the "
                  "flames while your tank is started!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  !mech_event_count(mech, EVENT_VEHICLEBURN),
                  "This unit is not on fire!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_event_count(mech, EVENT_VEHICLE_EXTINGUISH),
                  "You're already trying to put out the fire!");

  mech_notify(mech, MECHALL, "You begin to extinguish the fires!");

  mech_event_schedule(mech, EVENT_VEHICLE_EXTINGUISH,
                      vehicle_extinquish_fire_event, VEHICLE_EXTINGUISH_TICK,
                      0);
}

/*
 *  Mechs entering level 2 water, or proning in level 1 water should
 *  extinguish any inferno currently burning.
 */
void water_extinguish_inferno(MECH *mech) {
  int elev = MechElevation(mech);
  MAP *map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  if (!InWater(mech) || MechType(mech) != CLASS_MECH || !Jellied(mech) ||
      (elev == -1 && !Fallen(mech)))
    return;

  mux_event_remove_type_data(mech->xcode.context->events, EVENT_BURN,
                             (void *)mech);
  MechCritStatus(mech) &= ~JELLIED;

  mech_notify(mech, MECHALL, "The flames extinguish in a roar of steam!");
  MechLOSBroadcast(
      mech, "is surrounded by a plume of steam as the flames extinguish.");

  /* According to FASA, the inferno jelly should keep on burning on the
   * water hex. We'll just add some steam (smoke) instead. */
  add_decoration(map, MechX(mech), MechY(mech), TYPE_SMOKE, SMOKE, 120);
}

void checkVehicleInFire(MECH *objMech, int fromHexFire) {
  int wRoll = btech_random_roll(objMech->xcode.context);
  int wIter;
  int wDamage = 0;

  switch (MechMove(objMech)) {
  case MOVE_WHEEL:
  case MOVE_VTOL:
    wRoll += 2;
    break;
  case MOVE_HOVER:
    wRoll += 4;
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
    if (MechType(objMech) == CLASS_VTOL) {
      /*
       * VTOLs _should_ make a pskill or go up one level... not right now tho
       */
    } else {
      mech_notify(objMech, MECHALL,
                  "[fg=red bold]The fire damages your motive system![reset]");
      DoMotiveSystemHit(objMech, 0);
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
      wDamage = btech_random_range(objMech->xcode.context, 1, 6);

      if (GetSectInt(objMech, wIter))
        DamageMech(objMech, objMech, 0, -1, wIter, 0, 0, wDamage, 0, 0, 0, -1,
                   0, 1);
    }
    break;

  default:
    vehicle_start_burn(objMech, objMech);
    break;
  }
}
