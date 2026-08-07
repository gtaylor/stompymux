/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

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
#include "btmux_build_config.h"
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
#include "mymath.h"
#include "random.h"
#include "registry_api.h"
#include "template_api.h"
#include "weapon_settings.h"

#ifdef BT_PART_WEIGHTS
/* From template.c */
extern const int internalsweight[];
extern const int cargoweight[];
#endif

#ifdef BT_MOVEMENT_MODES
#include "failures.h"
#endif

static inline int mech_weapon_recycle_time(const Mech *mech, int weapon_index) {
  return btech_weapon_settings_recycle_time(
      &mech->xcode.context->weapon_settings, weapon_index);
}

static inline int mech_weapon_battle_value(const Mech *mech, int weapon_index) {
  return btech_weapon_settings_battle_value(
      &mech->xcode.context->weapon_settings, weapon_index);
}

enum { BTECH_BV_SKILL_LIMIT = 8 };
extern float skillmul[BTECH_BV_SKILL_LIMIT][BTECH_BV_SKILL_LIMIT];

static inline int battle_value_skill_index(int skill) {
  return skill < 0                           ? 0
         : skill >= BTECH_BV_SKILL_LIMIT - 1 ? BTECH_BV_SKILL_LIMIT - 1
                                             : skill;
}

/* TODO: We can use M_PI if exists, otherwise define something reasonable.  */
static inline float degrees_to_radians(float degrees) {
  return degrees * (3.14159265F / 180.0F);
}

static inline float radians_to_degrees(float radians) {
  return radians * (180.0F / 3.14159265F);
}
