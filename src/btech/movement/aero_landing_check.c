#include "aero_move_api.h"

#include "btech/context.h"
#include "command_handlers_api.h"
#include "map_coordinates.h"
#include "mech_electronics_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"

void aero_checklz(DbRef player, Mech *mech, char *buffer) {
  char *arguments[3];
  int x;
  int y;

  if (!common_checks(player, mech, MECH_USUAL))
    return;

  const int ARGUMENT_COUNT = mech_parseattributes(buffer, arguments, 3);
  if (ARGUMENT_COUNT == 2) {
    if (!parse_int_checked(arguments[0], &x) ||
        !parse_int_checked(arguments[1], &y)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid coordinates!");
      return;
    }
    if (!mech_is_observer(mech)) {
      float real_x;
      float real_y;
      const int TACTICAL_RANGE = mech_tactical_range(mech);
      map_coord_to_real_coord(x, y, &real_x, &real_y);
      if (map_real_range(&(MapRealSegment){
              .start = {.x = mech_position_real_x(mech),
                        .y = mech_position_real_y(mech)},
              .end = {.x = real_x, .y = real_y},
          }) > (float)TACTICAL_RANGE) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Out of range!");
        return;
      }
    }
  } else if (ARGUMENT_COUNT == 0) {
    x = mech_position_x(mech);
    y = mech_position_y(mech);
  } else {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of parameters!");
    return;
  }

  int issue = aero_landing_zone_check(mech, x, y);
  if (!issue) {
    mech_printf(mech, MECHALL,
                "The hex (%d,%d) looks good enough for a landing.", x, y);
    return;
  }
  mech_printf(mech, MECHALL,
              "The hex (%d,%d) doesn't look good for landing: %s.", x, y,
              aero_landing_reason(issue - 1));
}
