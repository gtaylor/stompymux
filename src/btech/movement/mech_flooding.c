/* Implements BattleTech movement mechanics for unit flooding. */

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "aero_move_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "environment_damage_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_position_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_stagger.h"
#include "mech_status_types.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"
/* Flooding code. Once we're in water, this is checked
   now and then (basically when DamageMech'ed and/or
   depth changes and/or we fall) */

static bool mech_is_in_water(Mech *mech) {
  return battle_terrain_is_water(mech_real_terrain_get(mech)) &&
         mech_position_z(mech) < 0;
}

void mech_flood_section(Mech *mech, int loc, int lev) {
  char locbuff[32];
  MechConditionSummary condition = mech_condition_summary(mech);

  if (condition.combat_safe)
    return;

  if ((mech_section_armor(mech, loc) &&
       (mech_section_rear_armor(mech, loc) ||
        !mech_section_original_rear_armor(mech, loc))) ||
      !mech_section_internal(mech, loc))
    return;
  if (!mech_is_in_water(mech))
    return;
  if (lev >= 0)
    return;
  /* No armor, and in water. */
  if (lev == -1 &&
      (!mech_is_fallen(mech) && loc != LLEG && loc != RLEG &&
       (mech_movement_type(mech) != MOVE_QUAD || (loc != LARM && loc != RARM))))
    return;
  if (mech_class(mech) != CLASS_MECH)
    return;

  if (mech_section_is_flooded(mech, loc))
    return;

  /* Woo, valid target. */
  ArmorStringFromIndex(loc, locbuff, mech_class(mech),
                       mech_movement_type(mech));
  mech_printf(
      mech, MECHALL,
      "[fg=red bold]Water floods into your %s disabling everything that was "
      "there![reset]",
      locbuff);
  mech_los_broadcast(
      mech, tprintf("has a gaping hole in %s, and water pours in!", locbuff));

  mech_section_flooded_set(mech, loc, true);
  mech_parts_destroy(mech, mech, loc, 1, 1);
}

void mech_flood(Mech *mech) {
  int i;
  int elev = mech_position_surface_elevation(mech);
  MechConditionSummary condition = mech_condition_summary(mech);

  if (!mech_is_in_water(mech))
    return;

  /* Waterproof Tech - no flooding if we have this */
  if (mech_technology_flags_secondary(mech) & WATERPROOF_TECH)
    return;

  if (mech_class(mech) == CLASS_BSUIT) {

    if (condition.swarm_target > 0)
      return;

    mech_notify(mech, MECHALL,
                "You somehow find yourself in water and realize this may "
                "really really suck...");
    mech_notify(mech, MECHALL,
                "Everything gets very dark as water starts to fill your suit "
                "and you sink towards the bottom!");

    mech_los_broadcast(
        mech, "shudders, splashes in the water for a second, then goes limp "
              "and sinks to the bottom.");

    mech_contents_kill_if_in_character(mech);
    mech_destroy(mech, mech, 0, KILL_TYPE_FLOOD);
    return;
  }

  if (mech_class(mech) != CLASS_MECH)
    return;

  if (mech_position_z(mech) >= 0)
    return;

  for (i = 0; i < NUM_SECTIONS; i++)
    mech_flood_section(mech, i, elev);
}
