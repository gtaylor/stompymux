/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Mon Jan  6 15:57:35 1997 fingon
 * Last modified: Mon Jun  8 19:49:25 1998 fingon
 *
 */

enum { BOMB_GRAVITY = 1 };

#include <stdio.h>
#include <stdlib.h>
#include <strings.h>

#include "artillery_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "econ_cmds_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "math.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mycool.h"
#include "mymath.h"
#include "registry_api.h"

typedef enum BombKind {
  BOMB_KIND_STANDARD,
  BOMB_KIND_INFERNO,
  BOMB_KIND_CLUSTER,
} BombKind;

typedef struct BombInfo {
  const char *name;
  int aff;
  BombKind type;
  int weight;
} BombInfo;

typedef struct BombShot {
  int x;
  int y;
  int type;
  BattleMap *map;
} BombShot;

static const BombInfo bombs[] = {{"10_Inferno", 10, BOMB_KIND_INFERNO, 30},
                                 {"10_Cluster", 10, BOMB_KIND_CLUSTER, 30},
                                 {"10_Standard", 10, BOMB_KIND_STANDARD, 130},
                                 {"50_Inferno", 50, BOMB_KIND_INFERNO, 130},
                                 {"50_Cluster", 50, BOMB_KIND_CLUSTER, 130},
                                 {"50_Standard", 50, BOMB_KIND_STANDARD, 130},
                                 {"100_Inferno", 100, BOMB_KIND_INFERNO, 250},
                                 {"100_Cluster", 100, BOMB_KIND_CLUSTER, 250},
                                 {"100_Standard", 100, BOMB_KIND_STANDARD, 250},
                                 {nullptr, 0, BOMB_KIND_STANDARD, 0}};

int bomb_weight(int i) { return bombs[i].weight; }

const char *bomb_name(int i) { return bombs[i].name; }

static void bomb_list(Mech *mech, DbRef player) {
  int bc = 0, fb;
  int i, j, k;
  char location[20];
  static const char *const types[] = {"Standard", "Inferno", "Cluster",
                                      nullptr};
  CoolMenu *c = nullptr;

  cool_menu_add_line(&c);
  cool_menu_add_centered(
      &c, tprintf("Bomb payload for %s:", mech_display_id(mech).text));
  cool_menu_add_line(&c);
  for (i = 0; i < NUM_SECTIONS; i++) {
    fb = 1;
    for (j = 0; j < NUM_CRITICALS; j++)
      if (equipment_is_bomb((k = mech_critical_part_type(mech, i, j)))) {
        k = bomb_from_equipment_index(k);
        if (fb) {
          ArmorStringFromIndex(i, location, mech_class(mech),
                               mech_movement_type(mech));
          fb = 0;
        }
        if (!bc) {
          cool_menu_add_text(&c, tprintf("#  %-20s %-5s %-5s %s", "Location",
                                         "Weight", "Power", "Type"));
        }
        cool_menu_add_text(&c, tprintf("%-2d %-20s %5d %5d %s", bc + 1,
                                       location, bombs[k].weight / 10,
                                       bombs[k].aff, types[bombs[k].type]));
        bc++;
      }
  }
  if (!bc)
    cool_menu_add_centered(&c, "No bombs installed.");
  cool_menu_add_line(&c);
  ShowCoolMenu(btech_context_evaluation(mech_context(mech)), player, c);
  KillCoolMenu(c);
}

static float bomb_calculate_destination(Mech *mech, short *x, short *y) {
  /* Present location */
  float fx = mech_position_real_x(mech);
  float fy = mech_position_real_y(mech);
  float fz = mech_position_real_z(mech) / ZSCALE;
  float zspd = mech_motion_vector_z(mech) / ZSCALE;
  float t, ot;

  ot = t = (zspd + sqrt(zspd * zspd + 2 * BOMB_GRAVITY * fz)) / BOMB_GRAVITY;
  t = (float)t / MOVE_TICK;
  fx = fx + mech_motion_vector_x(mech) * t;
  fy = fy + mech_motion_vector_y(mech) * t;
  RealCoordToMapCoord(x, y, fx, fy);
  return ot;
}

static void bomb_aim(Mech *mech, DbRef player) {
  float t; /* The time of impact */
  char toi[LBUF_SIZE];
  short x, y;

  t = bomb_calculate_destination(mech, &x, &y);
  snprintf(toi, LBUF_SIZE, "%.1f second%s", t,
           (t >= 2.0 || t < 1.0) ? "" : "s");
  mech_printf(mech, MECHALL,
              "Estimated bomb flight time %s, estimated landing hex %d,%d.",
              toi, x, y);
}

static void bomb_hit_hexes(BattleMap *map, int x, int y, int hitnb,
                           bool iscluster, int aff_d, int aff_h, char *tomsg,
                           char *otmsg, char *tomsg1, char *otmsg1) {
  blast_hit_hexes(map, aff_d, iscluster ? 2 : 10, aff_h, x, y, tomsg, otmsg,
                  tomsg1, otmsg1, 0, 4, 1, 1, hitnb);
}

static void bomb_hit(BombShot *s) {
  switch (bombs[s->type].type) {
  case BOMB_KIND_STANDARD:
    HexLOSBroadcast(s->map, s->x, s->y, "A blast rocks the area around $H!");
    bomb_hit_hexes(
        s->map, s->x, s->y, 1, 0,
        bombs[s->type].type == BOMB_KIND_INFERNO ? bombs[s->type].aff / 2
                                                 : bombs[s->type].aff,
        bombs[s->type].type == BOMB_KIND_INFERNO ? bombs[s->type].aff : 0,
        "You receive a direct hit!", "receives a direct hit!",
        "You are hit by shrapnel!", "is hit by shrapnel!");
    break;
  case BOMB_KIND_INFERNO:
    HexLOSBroadcast(
        s->map, s->x, s->y,
        "A fiery blast occurs in $H, spraying flaming gel everywhere!");
    bomb_hit_hexes(
        s->map, s->x, s->y, 1, 0,
        bombs[s->type].type == BOMB_KIND_INFERNO ? bombs[s->type].aff / 2
                                                 : bombs[s->type].aff,
        bombs[s->type].type == BOMB_KIND_INFERNO ? bombs[s->type].aff : 0,
        "You receive a direct hit!", "receives a direct hit!",
        "You are hit by the globs of flaming gel!", "is hit by the globs!");
    break;
  case BOMB_KIND_CLUSTER:
    HexLOSBroadcast(
        s->map, s->x, s->y,
        "A bomb drops rain of small bomblets in $H's surroundings!");
    bomb_hit_hexes(
        s->map, s->x, s->y, 1, 1,
        bombs[s->type].type == BOMB_KIND_INFERNO ? bombs[s->type].aff / 2
                                                 : bombs[s->type].aff,
        bombs[s->type].type == BOMB_KIND_INFERNO ? bombs[s->type].aff : 0,
        "You are hit by ton of small munitions!",
        "is hit by many small munitions!",
        "You are hit by some of the small munitions!",
        "is hit by some small munitions!");
    break;
  }
}

static void bomb_hit_event(MuxEvent *e) {
  BombShot *s = e->data;

  bomb_hit(s);
  free(s);
}

static void bomb_simulate_flight(Mech *mech, BattleMap *map, short *x, short *y,
                                 float t) {
  float fx = mech_position_real_x(mech);
  float fy = mech_position_real_y(mech);
  float fz = mech_position_real_z(mech);

  float delx, dely;
  float dx, dy;
  int i;
  short tx, ty;

  if (t < 1.0)
    return;
  MapCoordToRealCoord(*x, *y, &dx, &dy);
  delx = (dx - fx) / t;
  dely = (dy - fy) / t;
  for (i = 1; i < t; i++) {
    fx = fx + delx;
    fy = fx + dely;
    fz = (float)fz - BOMB_GRAVITY;
    RealCoordToMapCoord(&tx, &ty, fx, fy);
    if (!battle_map_coordinate_is_valid(map, tx, ty))
      continue;
    if (battle_map_hex_elevation(map, tx, ty) > (fz / ZSCALE)) {
      *x = tx;
      *y = ty;
    }
  }
}

static void bomb_drop(Mech *mech, DbRef player, int bn) {
  int bc = 0;
  int i, j, k;
  int lloc = 0, lpos = 0;
  float t;
  short x, y;
  int ob;
  int di;
  float dir;
  BombShot *s;
  BattleMap *map;

  if (bn < 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Negative bomb number? Gimme a break.");
    return;
  }
  bn--;
  for (i = 0; i < NUM_SECTIONS; i++)
    for (j = 0; j < NUM_CRITICALS; j++)
      if (equipment_is_bomb((k = mech_critical_part_type(mech, i, j))) &&
          !mech_critical_is_destroyed(mech, i, j)) {
        if (bc == bn) {
          lloc = i;
          lpos = j;
        }
        bc++;
      }
  if (!bc) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "No bombs installed.");
    return;
  }
  if (!(map =
            btech_context_get_map(mech_context(mech), mech_map_dbref(mech)))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're on invalid map!");
    return;
  }
  if (bn < 0 || bn >= bc) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "No bomb with such number installed! (See BOMB LIST)");
    return;
  }
  mech_los_broadcast(mech,
                     "detaches a small object that starts falling down..");
  k = bomb_from_equipment_index(mech_critical_part_type(mech, lloc, lpos));
  mech_notify(mech, MECHALL, "The ship trembles as you detach a bomb..");
  t = bomb_calculate_destination(mech, &x, &y);
  ob = (int)t / 10;
  if (MadePilotSkillRoll(mech, 4 + ob) || t < 2.0)
    mech_notify(mech, MECHALL,
                "Despite the slight problems, you keep the craft stable enough "
                "to drop the bomb right on target..");
  else {
    mech_notify(mech, MECHALL,
                "The ship's lurches slightly, dropping the bomb off target!");
    ob = 6 * (1 + ob); /* Max distance missed  */
    ob = MAX(1, (btech_random_range(mech_context(mech), 1, ob)) / 2);
    di = btech_random_range(mech_context(mech), 0, 359);
    dir = di * TWOPIOVER360;
    x = x + ob * cos(dir);
    y = y + ob * sin(dir);
  }
  bomb_simulate_flight(mech, map, &x, &y, t);
  if (!battle_map_coordinate_is_valid(map, x, y))
    return;
  mech_critical_part_type_set(mech, lloc, lpos, 0);
  Create(s, BombShot, 1);
  s->x = x;
  s->y = y;
  s->type = k;
  s->map = map;
  mech_cargo_weight_recalculate(mech);
  btech_context_event_schedule(mech_context(mech), s, EVENT_DHIT,
                               bomb_hit_event, MAX(1, t), 0);
}

void mech_bomb(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[3];
  int argc;
  int bn;

  if (!common_checks(player, mech, MECH_USUALSO))
    return;
  if (!(argc = mech_parseattributes(buffer, args, 3))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "(At least) one option required.");
    return;
  }
  if (argc > 2) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Too many arguments!");
    return;
  }
  if (!strcasecmp(args[0], "list")) {
    bomb_list(mech, player);
    return;
  }
  if (mech_is_landed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "The craft is landed!");
    return;
  }
  if (!strcasecmp(args[0], "aim")) {
    bomb_aim(mech, player);
    return;
  }
  if (strcasecmp(args[0], "drop")) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid argument to BOMB!");
    return;
  }
  if (argc < 2) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "The BOMB commands needs to know WHICH bomb to drop!");
    return;
  }
  if ((!((bn) = atoi(args[1])) && strcmp((args[1]), "0"))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid bomb number!");
    return;
  }
  bomb_drop(mech, player, bn);
}
