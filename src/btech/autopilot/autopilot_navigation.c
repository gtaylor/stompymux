#include "ai_api.h"
#include "ai_simulation_api.h"
#include "autopilot.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "mech_combat_api.h"
#include "mech_identity_api.h"
#include "mech_position_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include <stdio.h>

void ai_init(Autopilot *a, Mech *m [[maybe_unused]]) {

  /* XXX Analyze our unit type ; set basic combat tactic */
  a->auto_cmode = 1; /* CHARGE! */
  a->auto_cdist = 2; /* Attempt to avoid kicking distance */
  a->auto_nervous = 0;
  a->auto_goweight = 44; /* We're mainly concentrating on fighting */
  a->auto_fweight = 55;
  a->speed = 100; /* Reset to full speed */
  a->flags = 0;
  a->target = -1;
}

int artillery_round_flight_time(float fx, float fy, float tx, float ty);

static bool mech_snipe_func(const MultiWeaponSelectionCall *call) {
  char message_buffer[128];
  Mech *mech = call->mech;
  /* Simulate mech movements until flight_time <= now */
  int now = 0;
  int crashed = 0;
  LocationSimulation t;
  Mech *target_mech = call->context;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  location_simulation_initialize(&t, target_mech);
  while (artillery_round_flight_time(mech_position_real_x(mech),
                                     mech_position_real_y(mech), t.fx,
                                     t.fy) > now) {
    if (!crashed)
      if (ai_crash(map, target_mech, &t))
        crashed = 1;
    now++;
  }
  (void)snprintf(message_buffer, sizeof(message_buffer), "%d %d", t.x, t.y);
  /* Fire at t.x, t.y */
  if (mech_target_hex_x(mech) != t.x || mech_target_hex_y(mech) != t.y)
    mech_set_target(call->actor, mech, message_buffer);
  (void)snprintf(message_buffer, sizeof(message_buffer), "%d", call->first);
  mech_fireweapon(call->actor, mech, message_buffer);
  return false;
}

void mech_snipe(DbRef player, Mech *mech, char *buffer) {
  char *args[3];
  DbRef d;
  Mech *target_mech;

  if (!is_wizard(btech_context_database(mech_context(mech)), player)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Permission denied.");
    return;
  }
  if (mech_parseattributes(buffer, args, 3) != 2) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Please supply target ID _and_ weapon(s) to use");
    return;
  }
  d = find_target_dbref_from_map_number(mech, args[0]);
  if (d <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid target!");
    return;
  }
  target_mech = btech_context_get_mech(mech_context(mech), d);
  multi_weapon_select(&(MultiWeaponSelectionRequest){
      .mech = mech,
      .actor = player,
      .selection = args[1],
      .mode = 1,
      .callback = mech_snipe_func,
      .context = target_mech,
  });
}
