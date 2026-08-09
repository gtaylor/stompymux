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
#include "mux/support/formatting.h"
#include "registry_api.h"

void ai_init(Autopilot *a, Mech *m) {

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

static int mech_snipe_func(Mech *mech, DbRef player, int index, int high,
                           void *context) {
  /* Simulate mech movements until flight_time <= now */
  int now = 0, crashed = 0;
  int flt_time;
  LocationSimulation t;
  Mech *target_mech = context;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  location_simulation_initialize(&t, target_mech);
  while ((flt_time = artillery_round_flight_time(mech_position_real_x(mech),
                                                 mech_position_real_y(mech),
                                                 t.fx, t.fy)) > now) {
    if (!crashed)
      if (ai_crash(map, target_mech, &t))
        crashed = 1;
    now++;
  }
  /* Fire at t.x, t.y */
  if (mech_target_hex_x(mech) != t.x || mech_target_hex_y(mech) != t.y)
    mech_set_target(player, mech, tprintf("%d %d", t.x, t.y));
  mech_fireweapon(player, mech, tprintf("%d", index));
  return 0;
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
  if ((d = FindTargetDBREFFromMapNumber(mech, args[0])) <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid target!");
    return;
  }
  target_mech = btech_context_get_mech(mech_context(mech), d);
  multi_weap_sel(mech, player, args[1], 1, mech_snipe_func, target_mech);
}
