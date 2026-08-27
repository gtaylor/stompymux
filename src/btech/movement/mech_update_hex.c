/* Implements BattleTech movement mechanics for unit update hex. */
#include "btech/context.h"
#include "btechstats_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_fire_api.h"
#include "mech_hex_transition_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "section_types.h"
/*
 * Check to see what happens to the unit now that its entered a new hex
 */
void mech_hex_entry_resolve(const MechHexEntryRequest *request) {
  Mech *mech = request->mech;
  BattleMap *mech_map = request->map;
  float deltax = request->delta.x;
  const float DELTAY = request->delta.y;
  const int LAST_Z = request->previous_z;
  int elevation;
  int lastelevation;
  int oldterrain;
  int ot;
  int done = 0;
  BtechContext *context = mech_context(mech);
  bool skid_cliff = btech_context_uses_skid_cliff_rules(context);
  bool roll_on_backwalk = btech_context_uses_roll_on_backwalk(context);
  bool new_terrain = btech_context_uses_new_terrain_rules(context);
  bool advanced_vehicle_fire =
      btech_context_uses_advanced_vehicle_fire(context);
  /* Recording the old elevation and terrain */
  /*! \todo {Wasn't lastelevation passed as an argument 'last_z' ?} */
  ot = oldterrain = (unsigned char)map_terrain_get(
      mech_map, mech_position_previous_x(mech), mech_position_previous_y(mech));
  if ((mech_movement_type(mech) == MOVE_HOVER) &&
      (oldterrain == BATTLE_TERRAIN_WATER || oldterrain == BATTLE_TERRAIN_ICE ||
       ((oldterrain == BATTLE_TERRAIN_BRIDGE) && (LAST_Z == 0)))) {
    lastelevation = elevation = 0;
  } else {
    lastelevation =
        battle_map_hex_elevation(mech_map, mech_position_previous_x(mech),
                                 mech_position_previous_y(mech));
    elevation = mech_position_surface_elevation(mech);
    if (mech_movement_type(mech) == MOVE_HOVER && elevation < 0)
      elevation = 0;
    if (ot == BATTLE_TERRAIN_ICE && mech_position_z(mech) >= 0) {
      lastelevation = 0;
    }
    /*	if(mech_position_z(mech) < le)
                    le = mech_position_z(mech);
    */
  }
  switch (mech_movement_type(mech)) {
  case MOVE_BIPED:
  case MOVE_QUAD: {
    HexTransitionResult result = mech_hex_transition_resolve(
        &(HexMechTransitionInput){.mech = mech,
                                  .map = mech_map,
                                  .delta_x = deltax,
                                  .delta_y = DELTAY,
                                  .elevation = elevation,
                                  .last_elevation = lastelevation,
                                  .old_terrain = oldterrain});
    if (result.stop)
      return;
    done = result.done;
    break;
  }
  case MOVE_TRACK:
  case MOVE_WHEEL:
  case MOVE_HULL:
  case MOVE_FOIL:
  case MOVE_SUB:
  case MOVE_HOVER:
  case MOVE_VTOL:
  case MOVE_FLY:
    if (mech_vehicle_hex_transition_resolve(&(HexVehicleTransitionInput){
            .mech = mech,
            .delta_x = deltax,
            .delta_y = DELTAY,
            .elevation = elevation,
            .last_elevation = lastelevation,
            .old_terrain = oldterrain,
            .skid_cliff = skid_cliff,
            .roll_on_backwalk = roll_on_backwalk,
            .new_terrain = new_terrain,
        })) {
      return;
    }
    break;
  case MOVE_NONE:
    break;
  }
  if (!done) {
    mine_field_trigger(mech, MINE_STEP);
    if (advanced_vehicle_fire && (mech_class(mech) == CLASS_VEH_GROUND) &&
        (mech_position_terrain(mech) == BATTLE_TERRAIN_FIRE))
      vehicle_fire_check(mech, 1);
  }
  mark_for_los_update(mech);
}
