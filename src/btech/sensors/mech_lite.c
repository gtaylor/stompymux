
/* Implements searchlight and illumination effects. */

#include "btconfig.h"
#include "equipment_types.h"
#include "map_los_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_condition_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_lite_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_sensor_state_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"

/* If the target is in the front arc, and Line of Sight is not blocked
 * (by terrain, water hexes or more than 2 'points' of wood) and in
 * range, the target is lit.
 */
static int mech_lites_target(Mech *mech, Mech *target) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (!mech_searchlight_active(mech))
    return 0;
  if (mech_range_to(mech, target) > LITE_RANGE)
    return 0;
  if (!(in_weapon_arc(mech, mech_position_real_x(target),
                      mech_position_real_y(target)) &
        FORWARDARC))
    return 0;
  if (battle_map_unit_los_is_blocked(map, mech, target) ||
      battle_map_unit_los_wood_count(map, mech, target) > 2 ||
      battle_map_unit_los_water_count(map, mech, target) != 0)
    return 0;
  return 1;
}

void cause_lite(Mech *mech, Mech *temp_mech) {
  if (mech_condition_summary(temp_mech).illuminated)
    return;
  if (mech_lites_target(mech, temp_mech)) {
    mech_illumination_set(temp_mech, true);
    if (mech_searchlight_warning_enabled(temp_mech))
      mech_notify(temp_mech, MECHALL, "You are being illuminated!");
  }
}

void end_lite_check(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  Mech *t;
  int i;

  if (!mech_condition_summary(mech).illuminated)
    return;
  if (!map)
    return;
  for (i = 0; i < battle_map_unit_count(map); i++) {
    if (i == mech_map_slot(mech))
      continue;
    t = btech_context_find_object(mech_context(mech),
                                  battle_map_unit_dbref(map, i));
    if (!t)
      continue;
    if (mech_lites_target(t, mech))
      return;
  }
  mech_illumination_set(mech, false);
  if (mech_searchlight_warning_enabled(mech))
    mech_notify(mech, MECHALL, "You are no longer being illuminated.");
}
