/* Defines BattleTech unit data and interfaces for unit utils internal. */

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "aero_bomb_api.h"
#include "autopilot.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "ds_bay_api.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_consistency_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_restrict_api.h"
#include "mech_startup_api.h"
#include "mech_status_api.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "missile_hit_registry.h"
#include "mux/commands/action_messages.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/world/move.h"
#include "random.h"
#include "registry_api.h"
#include "template_api.h"
#include "weapon_settings.h"

int mech_weapon_recycle_time(const Mech *mech, int weapon_index);
int mech_weapon_battle_value(const Mech *mech, int weapon_index);

enum { BTECH_BV_SKILL_LIMIT = 8 };
float battle_value_skill_multiplier(int gunnery, int piloting);

static inline int battle_value_skill_index(int skill) {
  if (skill < 0)
    return 0;
  if (skill >= BTECH_BV_SKILL_LIMIT - 1)
    return BTECH_BV_SKILL_LIMIT - 1;
  return skill;
}

static inline float degrees_to_radians(float degrees) {
  return degrees * ((float)M_PI / 180.0F);
}

static inline float radians_to_degrees(float radians) {
  return radians * (180.0F / 3.14159265F);
}
