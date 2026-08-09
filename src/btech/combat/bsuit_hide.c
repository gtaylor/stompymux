/* Implements BattleTech combat mechanics for battle armor hide. */

#include <math.h>
#include <string.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "map.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

/*! \todo {The Bsuit code needs an overhaul} */

/* 2 battlesuit-specific attacks:
   - attackleg
   - swarm
 */

static int mech_hidden_turns(const Mech *mech) {
  int turns = mech_class(mech) == CLASS_MW      ? 1
              : mech_class(mech) == CLASS_BSUIT ? 3
              : mech_class(mech) == CLASS_VTOL  ? 4
                                                : 5;
  return turns * ((mech_technology_flags_secondary(mech) & CAMO_TECH) ? 1 : 2);
}

/* Stops everyone who's swarming this poor guy */

static void mech_hide_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  Mech *t;
  BtechContext *context = mech_context(mech);
  BattleMap *map = btech_context_get_map(context, mech_map_dbref(mech));
  int fail = 0, i;
  long tic = (long)e->data2;

  if (!map)
    return;

  for (i = 0; i < battle_map_unit_count(map); i++) {
    const DbRef unit = battle_map_unit_dbref(map, i);
    if (unit <= 0)
      continue;
    if (!(t = btech_context_get_mech(context, unit)))
      continue;
    if (mech_is_clairvoyant(t) || mech_is_observer(t) || mech_is_invisible(t))
      continue;
    if (mech_team(t) == mech_team(mech))
      continue;
    if (!mech_is_started(t))
      continue;
    if (mech_is_destroyed(t))
      continue;
    if (mech_los_check(t, mech, mech_position_x(mech), mech_position_y(mech),
                       mech_range_to(t, mech)))
      fail = 1;
  }

  if (mech_height_above_surface(mech))
    fail = 1;

  if (fail) {
    mech_notify(mech, MECHALL,
                "Your spidey sense tingles, telling you this isn't going to "
                "work......");
    return;
  } else if (tic < (mech_hidden_turns(mech) * HIDE_TICK)) {
    tic++;
    mech_event_schedule(mech, EVENT_HIDE, mech_hide_event, 1, tic);
  } else if (!fail) {
    mech_notify(mech, MECHALL, "You are now hidden!");
    mech_hidden_set(mech, true);
  }
  return;
}

void bsuit_hide(DbRef player, void *data, const char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  BattleMap *map = btech_context_find_object(context, mech_map_dbref(mech));
  int terrain;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (((mech_technology_flags_secondary(mech) & CAMO_TECH) ||
       is_wizard(btech_context_database(context), player))
          ? 0
          : mech_class(mech) != CLASS_BSUIT && mech_class(mech) != CLASS_MW) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You aren't capable of such curious things.");
    return;
  }

  if (!map) {
    mech_notify(mech, MECHALL, "You are not on a map!");
    return;
  }

  if (mech_is_jumping(mech) || mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Hide where? Up here?");
    return;
  }
  if (fabsf(mech_current_speed(mech)) > MP1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Come to a complete stop first.");
    return;
  }
  if (mech_event_count(mech, EVENT_HIDE)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are looking for cover already!");
    return;
  }
  if (mech_movement_type(mech) == MOVE_VTOL && !mech_is_landed(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You must be landed!");
    return;
  }

  if (mech_condition_summary(mech).swarm_target > 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Hide where? Not while on that!");
    return;
  }

  terrain =
      map_real_terrain_get(map, mech_position_x(mech), mech_position_y(mech));

  if (terrain == BATTLE_TERRAIN_LIGHT_FOREST ||
      terrain == BATTLE_TERRAIN_HEAVY_FOREST) {
    mech_notify(mech, MECHALL, "You start to hide amongst the trees...");
  } else if (terrain == BATTLE_TERRAIN_MOUNTAINS) {
    mech_notify(mech, MECHALL,
                "You start to hide behind some rocky outcroppings...");
  } else if (terrain == BATTLE_TERRAIN_ROUGH) {
    mech_notify(mech, MECHALL,
                "You find some boulders to try to hide behind...");
  } else if (terrain == BATTLE_TERRAIN_BUILDING &&
             mech_class(mech) == CLASS_BSUIT) {
    mech_notify(mech, MECHALL,
                "You break into a building and look for a spot to hide...");
  } else {
    mech_notify(mech, MECHALL, "You begin to hide in this terrain...");
    mech_notify(mech, MECHALL,
                "... then realize that just isn't going to work!");
    return;
  }

  mech_event_schedule(mech, EVENT_HIDE, mech_hide_event, 1, 0);
}
