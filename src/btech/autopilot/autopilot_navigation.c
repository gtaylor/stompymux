#include "autopilot_ai_internal.h"

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
  BattleMap *map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  location_simulation_initialize(&t, target_mech);
  while ((flt_time = artillery_round_flight_time(MechFX(mech), MechFY(mech),
                                                 t.fx, t.fy)) > now) {
    if (!crashed)
      if (ai_crash(map, target_mech, &t))
        crashed = 1;
    now++;
  }
  /* Fire at t.x, t.y */
  if (MechTargX(mech) != t.x || MechTargY(mech) != t.y)
    mech_settarget(player, mech, tprintf("%d %d", t.x, t.y));
  mech_fireweapon(player, mech, tprintf("%d", index));
  return 0;
}

void mech_snipe(DbRef player, Mech *mech, char *buffer) {
  char *args[3];
  DbRef d;
  Mech *target_mech;

  DOCHECK_CONTEXT(mech->xcode.context,
                  !is_wizard(mech->xcode.context->database, player),
                  "Permission denied.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 3) != 2,
                  "Please supply target ID _and_ weapon(s) to use");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (d = FindTargetDBREFFromMapNumber(mech, args[0])) <= 0,
                  "Invalid target!");
  target_mech = btech_context_get_mech(mech->xcode.context, d);
  multi_weap_sel(mech, player, args[1], 1, mech_snipe_func, target_mech);
}
