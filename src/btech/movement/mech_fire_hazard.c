#include "mech_update_api.h"

#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "section_types.h"

bool mech_fire_hazard_resolve(Mech *mech) {
  if (mech_position_terrain(mech) != BATTLE_TERRAIN_FIRE)
    return false;
  if (mech_is_aerospace_unit(mech) || mech_movement_type(mech) == MOVE_VTOL ||
      mech_is_destroyed(mech))
    return false;

  if (mech_class(mech) != CLASS_MW)
    return false;

  mech_notify(mech, MECHALL, "You feel a tad bit too warm..");
  mech_notify(mech, MECHALL, "You faint.");
  mech_destroy(mech, mech, 0, KILL_TYPE_HEAT);
  return true;
}
