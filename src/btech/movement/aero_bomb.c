/* Implements BattleTech movement mechanics for aerospace bomb. */

#include "equipment_types.h"
#include "mux/server/platform.h"
#include "mux/support/stringutil.h"
#include <string.h>
static const float BOMB_GRAVITY = 1.0F;

#include <stdio.h>
#include <stdlib.h>
#include <strings.h>

#include "aero_bomb_api.h"
#include "artillery_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "econ_cmds_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
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
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mycool.h"
#include "registry_api.h"
#include <math.h>

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

static const BombInfo BOMBS[] = {{"10_Inferno", 10, BOMB_KIND_INFERNO, 30},
                                 {"10_Cluster", 10, BOMB_KIND_CLUSTER, 30},
                                 {"10_Standard", 10, BOMB_KIND_STANDARD, 130},
                                 {"50_Inferno", 50, BOMB_KIND_INFERNO, 130},
                                 {"50_Cluster", 50, BOMB_KIND_CLUSTER, 130},
                                 {"50_Standard", 50, BOMB_KIND_STANDARD, 130},
                                 {"100_Inferno", 100, BOMB_KIND_INFERNO, 250},
                                 {"100_Cluster", 100, BOMB_KIND_CLUSTER, 250},
                                 {"100_Standard", 100, BOMB_KIND_STANDARD, 250},
                                 {nullptr, 0, BOMB_KIND_STANDARD, 0}};

static const BombInfo *bomb_info(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(BOMBS, 10, sizeof(*BOMBS), (size_t)index);
}

static const char *bomb_kind_name(BombKind kind) {
  switch (kind) {
  case BOMB_KIND_STANDARD:
    return "Standard";
  case BOMB_KIND_INFERNO:
    return "Inferno";
  case BOMB_KIND_CLUSTER:
    return "Cluster";
  }
  abort();
}

static char *bomb_argument(char **arguments, size_t count, size_t index) {
  char **slot = (char **)checked_storage_at((void *)arguments, count,
                                            sizeof(*arguments), index);
  return *slot;
}

int bomb_weight(int i) { return bomb_info(i)->weight; }

const char *bomb_name(int i) { return bomb_info(i)->name; }

static void bomb_list(Mech *mech, DbRef player) {
  int bc = 0;
  int fb;
  int i;
  int j;
  int k;
  char location[20];
  CoolMenu *c = nullptr;

  cool_menu_add_line(&c);
  cool_menu_add_centered(
      &c, tprintf("Bomb payload for %s:", mech_display_id(mech).text));
  cool_menu_add_line(&c);
  for (i = 0; i < NUM_SECTIONS; i++) {
    fb = 1;
    for (j = 0; j < NUM_CRITICALS; j++) {
      k = mech_critical_part_type(mech, i, j);
      if (equipment_is_bomb(k)) {
        k = bomb_from_equipment_index(k);
        if (fb) {
          armor_string_from_index(i, location, mech_class(mech),
                                  mech_movement_type(mech));
          fb = 0;
        }
        if (!bc) {
          cool_menu_add_text(&c, tprintf("#  %-20s %-5s %-5s %s", "Location",
                                         "Weight", "Power", "Type"));
        }
        const BombInfo *bomb = bomb_info(k);
        cool_menu_add_text(&c, tprintf("%-2d %-20s %5d %5d %s", bc + 1,
                                       location, bomb->weight / 10, bomb->aff,
                                       bomb_kind_name(bomb->type)));
        bc++;
      }
    }
  }
  if (!bc)
    cool_menu_add_centered(&c, "No bombs installed.");
  cool_menu_add_line(&c);
  show_cool_menu(btech_context_evaluation(mech_context(mech)), player, c);
  kill_cool_menu(c);
}

static float bomb_calculate_destination(Mech *mech, short *x, short *y) {
  /* Present location */
  float fx = mech_position_real_x(mech);
  float fy = mech_position_real_y(mech);
  float fz = mech_position_real_z(mech) / ZSCALE;
  float zspd = mech_motion_vector_z(mech) / ZSCALE;
  float t;
  float ot;

  ot = t =
      (zspd + sqrtf(zspd * zspd + 2.0F * BOMB_GRAVITY * fz)) / BOMB_GRAVITY;
  t /= (float)MOVE_TICK;
  fx = fx + mech_motion_vector_x(mech) * t;
  fy = fy + mech_motion_vector_y(mech) * t;
  real_coord_to_map_coord(x, y, fx, fy);
  return ot;
}

static void bomb_aim(Mech *mech, DbRef player) {
  float t; /* The time of impact */
  char toi[LBUF_SIZE];
  short x;
  short y;

  t = bomb_calculate_destination(mech, &x, &y);
  (void)snprintf(toi, LBUF_SIZE, "%.1f second%s", (double)t,
                 (t >= 2.0F || t < 1.0F) ? "" : "s");
  mech_printf(mech, MECHALL,
              "Estimated bomb flight time %s, estimated landing hex %d,%d.",
              toi, x, y);
}

static void bomb_hit_hexes(BattleMap *map, int x, int y, int hitnb,
                           bool iscluster, int aff_d, int aff_h,
                           const char *tomsg, const char *otmsg,
                           const char *tomsg1, const char *otmsg1) {
  BlastAreaRequest request = {
      .center =
          {
              .map = map,
              .damage = {.total = aff_d,
                         .hit_size = iscluster ? 2 : 10,
                         .heat = aff_h},
              .impact = {.x = x, .y = y},
              .messages = {.target = tomsg, .observers = otmsg},
              .safety = {.above = 4, .below = 1, .underwater = true},
          },
      .neighbor_messages = {.target = tomsg1, .observers = otmsg1},
      .neighbor_radius = hitnb,
  };
  blast_hit_area(&request);
}

static void bomb_hit(BombShot *s) {
  const BombInfo *bomb = bomb_info(s->type);
  const int DIRECT_DAMAGE =
      bomb->type == BOMB_KIND_INFERNO ? bomb->aff / 2 : bomb->aff;
  const int HEAT_DAMAGE = bomb->type == BOMB_KIND_INFERNO ? bomb->aff : 0;
  switch (bomb->type) {
  case BOMB_KIND_STANDARD:
    hex_los_broadcast(s->map, s->x, s->y, "A blast rocks the area around $H!");
    bomb_hit_hexes(s->map, s->x, s->y, 1, 0, DIRECT_DAMAGE, HEAT_DAMAGE,
                   "You receive a direct hit!", "receives a direct hit!",
                   "You are hit by shrapnel!", "is hit by shrapnel!");
    break;
  case BOMB_KIND_INFERNO:
    hex_los_broadcast(
        s->map, s->x, s->y,
        "A fiery blast occurs in $H, spraying flaming gel everywhere!");
    bomb_hit_hexes(s->map, s->x, s->y, 1, 0, DIRECT_DAMAGE, HEAT_DAMAGE,
                   "You receive a direct hit!", "receives a direct hit!",
                   "You are hit by the globs of flaming gel!",
                   "is hit by the globs!");
    break;
  case BOMB_KIND_CLUSTER:
    hex_los_broadcast(
        s->map, s->x, s->y,
        "A bomb drops rain of small bomblets in $H's surroundings!");
    bomb_hit_hexes(s->map, s->x, s->y, 1, 1, DIRECT_DAMAGE, HEAT_DAMAGE,
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

  float delx;
  float dely;
  float dx;
  float dy;
  int i;
  short tx;
  short ty;

  if (t < 1.0F)
    return;
  map_coord_to_real_coord(*x, *y, &dx, &dy);
  delx = (dx - fx) / t;
  dely = (dy - fy) / t;
  const float FLIGHT_TICKS_FLOAT = ceilf(t);
  const int FLIGHT_TICKS = (int)FLIGHT_TICKS_FLOAT;
  for (i = 1; i < FLIGHT_TICKS; i++) {
    fx = fx + delx;
    fy = fy + dely;
    fz -= BOMB_GRAVITY;
    real_coord_to_map_coord(&tx, &ty, fx, fy);
    if (!battle_map_coordinate_is_valid(map, tx, ty))
      continue;
    const int ELEVATION = battle_map_hex_elevation(map, tx, ty);
    if ((float)ELEVATION > (fz / (float)ZSCALE)) {
      *x = tx;
      *y = ty;
    }
  }
}

typedef struct BombDropRequest {
  Mech *mech;
  DbRef player;
  int bomb_number;
} BombDropRequest;

static void bomb_drop(const BombDropRequest *request) {
  Mech *mech = request->mech;
  const DbRef PLAYER = request->player;
  int bn = request->bomb_number;
  int bc = 0;
  int i;
  int j;
  int k;
  int lloc = 0;
  int lpos = 0;
  float t;
  short x;
  short y;
  int ob;
  int di;
  float dir;
  BombShot *s;
  BattleMap *map;

  if (bn < 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "Negative bomb number? Gimme a break.");
    return;
  }
  bn--;
  for (i = 0; i < NUM_SECTIONS; i++) {
    for (j = 0; j < NUM_CRITICALS; j++) {
      if (equipment_is_bomb(mech_critical_part_type(mech, i, j)) &&
          !mech_critical_is_destroyed(mech, i, j)) {
        if (bc == bn) {
          lloc = i;
          lpos = j;
        }
        bc++;
      }
    }
  }
  if (!bc) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "No bombs installed.");
    return;
  }
  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!map) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "You're on invalid map!");
    return;
  }
  if (bn < 0 || bn >= bc) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), PLAYER,
                 "No bomb with such number installed! (See BOMB LIST)");
    return;
  }
  mech_los_broadcast(mech,
                     "detaches a small object that starts falling down..");
  k = bomb_from_equipment_index(mech_critical_part_type(mech, lloc, lpos));
  mech_notify(mech, MECHALL, "The ship trembles as you detach a bomb..");
  t = bomb_calculate_destination(mech, &x, &y);
  const float IMPACT_TIME_TRUNCATED = truncf(t);
  ob = (int)IMPACT_TIME_TRUNCATED / 10;
  if (made_pilot_skill_roll(mech, 4 + ob) || t < 2.0F) {
    mech_notify(mech, MECHALL,
                "Despite the slight problems, you keep the craft stable enough "
                "to drop the bomb right on target..");
  } else {
    mech_notify(mech, MECHALL,
                "The ship's lurches slightly, dropping the bomb off target!");
    ob = 6 * (1 + ob); /* Max distance missed  */
    ob = max(1, btech_random_range_int(mech_context(mech), 1, ob) / 2);
    di = btech_random_range_int(mech_context(mech), 0, 359);
    dir = (float)di * (float)M_PI / 180.0F;
    const float SCATTERED_X = (float)x + (float)ob * cosf(dir);
    const float SCATTERED_Y = (float)y + (float)ob * sinf(dir);
    const float TRUNCATED_X = truncf(SCATTERED_X);
    const float TRUNCATED_Y = truncf(SCATTERED_Y);
    const int TARGET_X = (int)TRUNCATED_X;
    const int TARGET_Y = (int)TRUNCATED_Y;
    x = clamp_int_to_short(TARGET_X);
    y = clamp_int_to_short(TARGET_Y);
  }
  bomb_simulate_flight(mech, map, &x, &y, t);
  if (!battle_map_coordinate_is_valid(map, x, y))
    return;
  mech_critical_part_type_set(mech, lloc, lpos, 0);
  s = checked_storage_allocate(sizeof(*s));
  s->x = x;
  s->y = y;
  s->type = k;
  s->map = map;
  mech_cargo_weight_recalculate(mech);
  const float DELAY_TRUNCATED = truncf(t);
  const int DELAY = max(1, (int)DELAY_TRUNCATED);
  btech_context_event_schedule(mech_context(mech), s, EVENT_DHIT,
                               bomb_hit_event, DELAY, 0);
}

void mech_bomb(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[3];
  int argc;
  int bn;

  if (!common_checks(player, mech, MECH_USUALSO))
    return;
  argc = mech_parseattributes(buffer, args, 3);
  if (!argc) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "(At least) one option required.");
    return;
  }
  if (argc > 2) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Too many arguments!");
    return;
  }
  if (!strcasecmp(bomb_argument(args, 3, 0), "list")) {
    bomb_list(mech, player);
    return;
  }
  if (mech_is_landed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "The craft is landed!");
    return;
  }
  if (!strcasecmp(bomb_argument(args, 3, 0), "aim")) {
    bomb_aim(mech, player);
    return;
  }
  if (strcasecmp(bomb_argument(args, 3, 0), "drop")) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid argument to BOMB!");
    return;
  }
  if (argc < 2) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "The BOMB commands needs to know WHICH bomb to drop!");
    return;
  }
  if (!parse_int_checked(bomb_argument(args, 3, 1), &bn)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid bomb number!");
    return;
  }
  bomb_drop(&(BombDropRequest){
      .mech = mech,
      .player = player,
      .bomb_number = bn,
  });
}
