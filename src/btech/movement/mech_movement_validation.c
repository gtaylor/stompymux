#include "mech_movement_validation_api.h"

#include "btech/context.h"
#include "btech_channel.h"
#include "map_terrain.h"
#include "mech_crew_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_startup_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"

BattleMap *mech_movement_map_validate(Mech *mech) {
  BtechContext *context = mech_context(mech);
  DbRef map_dbref = mech_map_dbref(mech);
  DbRef pilot = mech_pilot_dbref(mech);
  BattleMap *map = btech_context_get_map(context, map_dbref);

  if (!map && pilot >= 0)
    map = ValidMap(context, pilot, map_dbref);

  bool position_is_valid =
      map &&
      battle_map_coordinate_is_valid(map, mech_position_x(mech),
                                     mech_position_y(mech)) &&
      battle_map_coordinate_is_valid(map, mech_position_previous_x(mech),
                                     mech_position_previous_y(mech));
  if (position_is_valid)
    return map;

  mech_notify(mech, MECHALL,
              map ? "You are at an invalid map location! Map index reset!"
                  : "You are on an invalid map! Map index reset!");
  mech_cocoon_integrity_set(mech, 0);
  if (mech_is_jumping(mech)) {
    char empty_command[] = "";
    mech_land(pilot, mech, empty_command);
  }
  char empty_command[] = "";
  mech_shutdown(pilot, mech, empty_command);
  btech_channel_send(context, BTECH_CHANNEL_MECH_ERRORS,
                     "move_mech:invalid map:Mech: %ld Index: %ld",
                     mech_dbref(mech), map_dbref);
  mech_map_dbref_set(mech, -1);
  return nullptr;
}
