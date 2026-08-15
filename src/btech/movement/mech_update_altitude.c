/* Implements BattleTech movement mechanics for unit update altitude. */

#include "equipment_types.h"
#include "mech_update_api.h"

#include <math.h>

#include "aero_move_api.h"
#include "btconfig.h"
#include "btech_channel.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_crew_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "section_types.h"

void mech_naval_altitude_check(Mech *mech, int previous_z) {
  char terrain = mech_real_terrain_get(mech);

  if (!battle_terrain_is_water(terrain)) {
    mech_movement_stop(mech);
    mech_vertical_speed_set(mech, 0.0F);
    mech_heading_set(mech, 0);
    mech_desired_heading_set(mech, 0);
    return;
  }
  if (!previous_z && mech_position_z(mech) &&
      mech_position_elevation(mech) > 1) {
    mark_for_los_update(mech);
    mech_position_hex_z_set(mech, 0);
    mech_los_broadcast(mech, "dives!");
    mech_position_hex_z_set(mech, -1);
  }
  if (mech_position_real_z(mech) > 0.0F) {
    if (mech_vertical_speed(mech) > 0 && !mech_position_z(mech) &&
        previous_z < 0) {
      mech_notify(mech, MECHALL,
                  "Your sub has reached surface and stops rising.");
      mech_los_broadcastf(mech, "surfaces at %d,%d!", mech_position_x(mech),
                          mech_position_y(mech));
      /* Possible show-up message? */
    }
    mech_position_z_set(mech, 0);
    if (mech_vertical_speed(mech) > 0)
      mech_vertical_speed_set(mech, 0.0F);
    return;
  }
  if (mech_position_z(mech) <= mech_lower_surface_elevation(mech)) {
    int z = mech_lower_surface_elevation(mech) + 1;
    if (z > 0)
      z = 0;
    mech_position_hex_z_set(mech, z);
    if (mech_position_surface_elevation(mech) > 0) {
      btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS,
                         "Oddity: #%ld managed to wind up on '%c' (%d elev.)",
                         mech_dbref(mech), mech_position_terrain(mech),
                         mech_position_elevation(mech));
    }
    const int MECH_Z = mech_position_z(mech);
    mech_position_real_z_set(mech,
                             (((5.0F * (float)MECH_Z) - 4.0F) * ZSCALE) / 5.0F);
    if (mech_movement_type(mech) == MOVE_SUB && mech_vertical_speed(mech) < 0) {
      mech_vertical_speed_set(mech, 0.0F);
      mech_notify(mech, MECHALL,
                  "The sub has reached bottom and stops diving.");
    }
  }
}

void mech_vtol_altitude_check(Mech *mech) {
  if (battle_terrain_is_water(mech_real_terrain_get(mech)) &&
      mech_position_z(mech) < 0) {
    mech_notify(mech, MECHALL, "You crash your vehicle into the water!");
    mech_notify(mech, MECHALL, "Water pours into the cockpit....glub glub!");
    mech_los_broadcast(mech, "splashes into the water!");
    mech_destroy(mech, mech, false, KILL_TYPE_FLOOD);
    return;
  }

  if (mech_position_z(mech) >= ORBIT_Z && !mech_is_aerospace_unit(mech)) {
    mech_notify(mech, MECHALL,
                "You cannot achieve orbit! Vertical movement halted!");
    mech_position_z_set(mech, ORBIT_Z - 1);
    mech_vertical_speed_set(mech, 0.0F);
    return;
  }

  if (mech_position_z(mech) >= mech_position_surface_elevation(mech))
    return;
  if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE &&
      mech_position_z(mech) != mech_position_surface_elevation(mech) - 1)
    return;
  aero_land(mech_pilot_dbref(mech), mech, "");
  if (mech_is_landed(mech))
    return;
  mech_notify(mech, MECHALL, "CRASH! You smash your toy into the ground!");
  mech_los_broadcast(mech, "crashes into the ground!");
  const float FALL_SPEED = fabsf(mech_vertical_speed(mech) / MP1);
  const int FALL_DISTANCE = 1 + (int)FALL_SPEED;
  mech_fall(mech, FALL_DISTANCE, false);

  mech_position_z_set(mech, mech_position_surface_elevation(mech));
  mech_current_speed_set(mech, 0.0F);
  mech_vertical_speed_set(mech, 0.0F);
  mech_landed_set(mech, true);
}
