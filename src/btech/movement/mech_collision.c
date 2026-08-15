#include "mech_hex_transition_api.h"

#include <stdlib.h>

#include "btech/context.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_update_api.h"
#include "registry_api.h"
#include "section_types.h"

static bool mech_moves_over_water(const Mech *mech) {
  int movement_type = mech_movement_type(mech);
  return (movement_type == MOVE_HOVER || mech_class(mech) == CLASS_MW ||
          movement_type == MOVE_FOIL || movement_type == MOVE_HULL) != 0;
}

static int mech_elevation_change_limit(const Mech *mech) {
  if (mech_class(mech) == CLASS_MW)
    return 3;
  int movement_type = mech_movement_type(mech);
  return movement_type == MOVE_BIPED || movement_type == MOVE_QUAD ? 2 : 1;
}

int bridge_w_elevation(Mech *mech) {
  (void)mech;
  return -1;
}

void bridge_set_elevation(Mech *mech) {
  int upper_elevation = mech_upper_surface_elevation(mech);
  if (mech_position_z(mech) <
      upper_elevation - mech_elevation_change_limit(mech)) {
    mech_position_z_set(
        mech, mech_moves_over_water(mech) ? 0 : bridge_w_elevation(mech));
    return;
  }
  mech_position_z_set(mech, upper_elevation);
}

int collision_check(const MovementCollisionCheck *check) {
  Mech *mech = check->mech;
  MovementCollisionMode mode = check->mode;
  int last_elevation = check->previous_elevation;
  int last_terrain = check->previous_terrain;
  BtechContext *context = mech_context(mech);
  BattleMap *map = btech_context_get_map(context, mech_map_dbref(mech));
  bool over_water = mech_moves_over_water(mech);
  int elevation_limit = mech_elevation_change_limit(mech);

  if (over_water && last_elevation < 0)
    last_elevation = 0;
  int elevation = mech_position_surface_elevation(mech);
  char terrain = mech_real_terrain_get(mech);
  if (terrain == BATTLE_TERRAIN_ICE && last_elevation >= 0)
    elevation = 0;
  if (last_elevation < mech_upper_surface_elevation(mech) - elevation_limit &&
      last_terrain == BATTLE_TERRAIN_BRIDGE)
    last_elevation = over_water ? 0 : bridge_w_elevation(mech);
  if (elevation < 0 && over_water)
    elevation = 0;

  switch (mode) {
  case JUMP:
    if (terrain == BATTLE_TERRAIN_BRIDGE)
      return mech_position_z(mech) < 0 ||
             mech_position_z(mech) == elevation - 1;
    return mech_position_z(mech) < elevation;
  case WALK_DROP:
    return last_elevation - elevation > elevation_limit;
  case WALK_WALL:
    if (mech_movement_type(mech) == MOVE_HOVER &&
        terrain == BATTLE_TERRAIN_BRIDGE &&
        (last_terrain == BATTLE_TERRAIN_ICE ||
         last_terrain == BATTLE_TERRAIN_WATER ||
         (last_elevation == 0 && last_terrain == BATTLE_TERRAIN_BRIDGE)))
      return 0;
    return elevation - last_elevation > elevation_limit;
  case WALK_BACK:
    if (mech_movement_type(mech) != MOVE_TRACK &&
        mech_class(mech) != CLASS_VTOL)
      return mech_current_speed(mech) < 0 ? abs(last_elevation - elevation) : 0;
    [[fallthrough]];
  case HIT_UNDER_BRIDGE:
    return last_terrain == BATTLE_TERRAIN_BRIDGE &&
           terrain == BATTLE_TERRAIN_BRIDGE && last_elevation == 0 && map &&
           battle_map_hex_elevation(map, mech_position_previous_x(mech),
                                    mech_position_previous_y(mech)) != 0 &&
           battle_map_hex_elevation(map, mech_position_x(mech),
                                    mech_position_y(mech)) == 1;
  }
  return 0;
}
