/* Implements BattleTech combat mechanics for unit fire. */

#include <stdint.h>

#include "btech/context.h"
#include "btech_event.h"
#include "equipment_types.h"
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

static constexpr int VEHICLEBURN_TICK = 60;
static constexpr int VEHICLE_EXTINGUISH_TICK = 120;

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

static void vehicle_burn_event(MuxEvent *obj_event) {
  Mech *obj_mech = (Mech *)obj_event->data; /* get the mech */
  const int W_LOC = (int)(intptr_t)obj_event->data2;
  int w_dam_roll;
  char str_loc_name[30];

  if (!obj_mech)
    return;
  w_dam_roll = btech_random_range_int(mech_context(obj_mech), 1, 6);

  armor_string_from_index(W_LOC, str_loc_name, mech_class(obj_mech),
                          mech_movement_type(obj_mech));

  if (!mech_section_internal(obj_mech, W_LOC))
    return;

  mech_printf(obj_mech, MECHALL,
              "[fg=red bold]Your %s takes damage from the fire![reset]",
              str_loc_name);
  mech_damage_apply(&(MechDamageRequest){.target = obj_mech,
                                         .attacker = obj_mech,
                                         .line_of_sight = false,
                                         .attack_pilot = -1,
                                         .hit_location = W_LOC,
                                         .rear = false,
                                         .critical = false,
                                         .armor_damage = w_dam_roll,
                                         .internal_damage = 0,
                                         .transfer = MECH_DAMAGE_NORMAL,
                                         .cause = 0,
                                         .base_to_hit = 0,
                                         .weapon_index = -1,
                                         .ammunition_mode = 0,
                                         .ignore_swarmers = true});

  /*
   * Only continue the event if the damage was greater than one
   */
  if ((w_dam_roll > 1) && mech_section_internal(obj_mech, W_LOC)) {
    mech_event_schedule(obj_mech, EVENT_VEHICLEBURN, vehicle_burn_event,
                        VEHICLEBURN_TICK, W_LOC);
  } else {
    if (mech_section_internal(obj_mech, W_LOC))
      mech_printf(obj_mech, MECHALL,
                  "The fire burning on your %s finally goes out.",
                  str_loc_name);
    if (!mech_event_count(obj_mech, EVENT_VEHICLEBURN))
      mech_los_broadcast(obj_mech, "is no longer engulfed in flames.");
  }
}

void vehicle_fire_start(Mech *obj_mech, Mech *obj_attacker) {
  int w_damage = 0;
  char str_loc_name[30];

  if (!obj_attacker)
    obj_attacker = obj_mech;

  mech_notify(obj_mech, MECHALL, "You catch on fire!");
  mech_los_broadcast(obj_mech, "catches on fire!");

  for (int w_iter = 0; w_iter < NUM_SECTIONS; w_iter++) {
    if (mech_section_internal(obj_mech, w_iter) &&
        !mech_event_count_data(obj_mech, EVENT_VEHICLEBURN, w_iter)) {
      w_damage = btech_random_range_int(mech_context(obj_mech), 1, 6);
      armor_string_from_index(w_iter, str_loc_name, mech_class(obj_mech),
                              mech_movement_type(obj_mech));
      mech_printf(obj_mech, MECHALL, "Your %s catches on fire!", str_loc_name);

      mech_damage_apply(&(MechDamageRequest){.target = obj_mech,
                                             .attacker = obj_attacker,
                                             .line_of_sight = false,
                                             .attack_pilot = -1,
                                             .hit_location = w_iter,
                                             .rear = false,
                                             .critical = false,
                                             .armor_damage = w_damage,
                                             .internal_damage = 0,
                                             .transfer = MECH_DAMAGE_NORMAL,
                                             .cause = 0,
                                             .base_to_hit = 0,
                                             .weapon_index = -1,
                                             .ammunition_mode = 0,
                                             .ignore_swarmers = true});
      mech_event_schedule(obj_mech, EVENT_VEHICLEBURN, vehicle_burn_event,
                          VEHICLEBURN_TICK, w_iter);
    }
  }
}

void vehicle_fire_extinguish_event(MuxEvent *e) {
  Mech *obj_mech = (Mech *)e->data;

  if (!obj_mech)
    return;

  if (!mech_event_count(obj_mech, EVENT_VEHICLEBURN))
    return;

  mech_event_cancel(obj_mech, EVENT_VEHICLEBURN);

  mech_notify(obj_mech, MECHALL, "You manage to dowse the fire.");
  mech_los_broadcast(obj_mech, "is no longer engulfed in flames.");
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
  add_decoration(&(MapDecorationRequest){
      .map = map,
      .position = {.x = mech_position_x(mech), .y = mech_position_y(mech)},
      .type = TYPE_SMOKE,
      .terrain_marker = SMOKE,
      .duration = 120,
  });
}

void vehicle_fire_check(Mech *obj_mech, int from_hex_fire) {
  int w_roll = btech_random_roll(mech_context(obj_mech));
  int w_iter;
  int w_damage = 0;

  switch (mech_movement_type(obj_mech)) {
  case MOVE_WHEEL:
  case MOVE_VTOL:
    w_roll += 2;
    break;
  case MOVE_HOVER:
    w_roll += 4;
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

  if (w_roll < 8) /* don't do jack if it's < 8 */
    return;

  if (from_hex_fire)
    mech_notify(
        obj_mech, MECHALL,
        "[fg=red bold]You drive through a wall of searing flames![reset]");
  else
    mech_notify(obj_mech, MECHALL,
                "[fg=red bold]The fires surround your vehicle![reset]");

  switch (w_roll) {
  case 8: /* roll once on the motive system chart */
  case 9:
    if (mech_class(obj_mech) == CLASS_VTOL) {
      /*
       * VTOLs _should_ make a pskill or go up one level... not right now tho
       */
    } else {
      mech_notify(obj_mech, MECHALL,
                  "[fg=red bold]The fire damages your motive system![reset]");
      mech_motive_system_hit(obj_mech, 0);
    }
    break;

  case 10:
  case 11:
    /*
     * Do 1d6 damage to each loc
     */
    mech_notify(
        obj_mech, MECHALL,
        "[fg=red bold]The fire sweeps across your unit damaging it![reset]");

    for (w_iter = 0; w_iter < NUM_SECTIONS; w_iter++) {
      w_damage = btech_random_range_int(mech_context(obj_mech), 1, 6);

      if (mech_section_internal(obj_mech, w_iter)) {
        mech_damage_apply(&(MechDamageRequest){.target = obj_mech,
                                               .attacker = obj_mech,
                                               .line_of_sight = false,
                                               .attack_pilot = -1,
                                               .hit_location = w_iter,
                                               .rear = false,
                                               .critical = false,
                                               .armor_damage = w_damage,
                                               .internal_damage = 0,
                                               .transfer = MECH_DAMAGE_NORMAL,
                                               .cause = 0,
                                               .base_to_hit = 0,
                                               .weapon_index = -1,
                                               .ammunition_mode = 0,
                                               .ignore_swarmers = true});
      }
    }
    break;

  default:
    vehicle_fire_start(obj_mech, obj_mech);
    break;
  }
}
