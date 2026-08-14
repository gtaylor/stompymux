/* Implements BattleTech combat mechanics for mine. */

/*
   Different types of mines:

   1 = Standard round (infinite explosions, same damage, to everyone in hex)
   2 = Inferno        (single explosion, adds heat instead)
   3 = Command-detonated (single explosion, goes off when hears transmission
   on predefined freq - damages neighbor hexes 1/2)
   4 = Vibra          (single explosion, triggered by weight (setting):
   target<=tons<(target+10) = when stepped on
   (target+10*n)<=tons      = when stepped on n hexes away
   tons<target              = no explosion
 */

#include "mine.h"

#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "artillery_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_bits_api.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/commands/action_messages.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "template_api.h"

/* Different types of mines
 *
 * The Trigger mines are used to let the MUX
 * know if a unit has moved to a certain spot
 *
 * The others are the explosive do damage kind */
static const char *mine_type_name(int type) {
  switch (type) {
  case 0:
    return "Standard";
  case 1:
    return "Inferno";
  case 2:
    return "Command";
  case 3:
    return "Vibra";
  case 4:
    return "Trigger";
  default:
    return nullptr;
  }
}

static int mine_type_index(const char *name) {
  for (int index = 0; mine_type_name(index) != nullptr; index++)
    if (!strcasecmp(mine_type_name(index), name))
      return index;
  return -1;
}

void mine_field_add(BattleMap *map, int x, int y, int damage) {
  MapObject *o;
  MapObject foo;

  if (is_mine_hex(map, x, y)) {
    for (o = map->map_object[TYPE_MINE]; o; o = o->next)
      if (o->x == x && o->y == y)
        break;
    if (o)
      return;
  }
  memset(&foo, 0, sizeof(foo));
  foo.x = clamp_int_to_short(x);
  foo.y = clamp_int_to_short(y);
  foo.datas = clamp_int_to_short(damage);
  foo.datac = MINE_STANDARD;
  add_mapobj(map, &map->map_object[TYPE_MINE], &foo, 1);
}

typedef struct MineDamageRequest {
  BattleMap *map;
  MapHexPosition position;
  BlastMessages messages;
  BlastMessages neighbor_messages;
  int damage;
  int heat;
  bool affect_neighbors;
} MineDamageRequest;

static void mine_damage_mechs(const MineDamageRequest *mine) {
  BlastAreaRequest request = {
      .center =
          {
              .map = mine->map,
              .damage = {.total = mine->damage,
                         .hit_size = 5,
                         .heat = mine->heat},
              .impact = mine->position,
              .messages = mine->messages,
              .hit_table = MINE_TABLE,
              .safety = {.above = 2, .below = 1, .underwater = true},
          },
      .neighbor_messages = mine->neighbor_messages,
      .neighbor_radius = mine->affect_neighbors ? 1 : 0,
  };
  blast_hit_area(&request);
}

static void update_mine(BattleMap *map, MapObject *mine) {
  int i;

  i = mine->datas;
  i = i * MINE_NEXT_MODIFIER;
  if (i >= MINE_MIN)
    mine->datas = clamp_int_to_short(i);
}

typedef struct MineExplosion {
  Mech *mech;
  BattleMap *map;
  MapObject *mine;
  MapHexPosition triggering_position;
  MineTriggerReason reason;
} MineExplosion;

static void mine_explode(const MineExplosion *explosion) {
  Mech *mech = explosion->mech;
  BattleMap *map = explosion->map;
  MapObject *o = explosion->mine;
  int x = explosion->triggering_position.x;
  int y = explosion->triggering_position.y;
  MineTriggerReason reason = explosion->reason;
  int cool = (o->datas >= MINE_MIN);

  if ((o->datac == MINE_TRIGGER) && reason != MINE_STEP && reason != MINE_LAND)
    return;
  if (o->datac != MINE_TRIGGER) {
    if (o->datac != MINE_COMMAND) {
      switch (reason) {
      case MINE_COMMAND_DETONATION:
        break;
      case MINE_STEP:
        mech_los_broadcastf(mech, "moves to %d,%d, and triggers a mine!", x, y);
        mech_printf(mech, MECHALL, "As you move to %d,%d, you trigger a mine!",
                    x, y);
        break;
      case MINE_LAND:
        mech_los_broadcast(mech, "triggers a mine!");
        mech_notify(mech, MECHALL, "You trigger a mine!");
        break;
      case MINE_DROP:
      case MINE_FALL:
        mech_los_broadcast(mech, "triggers a mine!");
        mech_notify(mech, MECHALL, "You trigger a mine!");
        break;
      }
    } else {
      hex_los_broadcast(map, o->x, o->y, "A mine explodes in $H!");
    }
  }

  switch (o->datac) {
  case MINE_STANDARD:
    update_mine(map, o);
    mine_damage_mechs(&(MineDamageRequest){
        .map = map,
        .position = {.x = o->x, .y = o->y},
        .messages = {.target = "A blast of shrapnel hits you!",
                     .observers = "is hit by shrapnel!"},
        .damage = o->datas});
    if (!cool) {
      map_objects_delete(&(MapObjectLookupRequest){
          .map = map,
          .position = {.x = o->x, .y = o->y},
          .type = TYPE_MINE,
      });
    }
    break;
  case MINE_INFERNO:
    update_mine(map, o);
    mine_damage_mechs(&(MineDamageRequest){
        .map = map,
        .position = {.x = o->x, .y = o->y},
        .messages = {.target = "Globs of flaming gel hit you!",
                     .observers = "is hit by globs of flaming gel!"},
        .damage = o->datas / 3,
        .heat = o->datas});
    if (!cool) {
      map_objects_delete(&(MapObjectLookupRequest){
          .map = map,
          .position = {.x = o->x, .y = o->y},
          .type = TYPE_MINE,
      });
    }
    break;
  case MINE_COMMAND:
    unset_hex_mine(map, o->x, o->y);
    mine_damage_mechs(&(MineDamageRequest){
        .map = map,
        .position = {.x = o->x, .y = o->y},
        .messages = {.target = "A blast of shrapnel hits you!",
                     .observers = "is hit by shrapnel!"},
        .neighbor_messages = {.target = "A little blast of shrapnel hits you!",
                              .observers = "is hit by some of the shrapnel!"},
        .damage = o->datas,
        .affect_neighbors = true});
    map_objects_delete(&(MapObjectLookupRequest){
        .map = map,
        .position = {.x = o->x, .y = o->y},
        .type = TYPE_MINE,
    });
    break;
  case MINE_TRIGGER:
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MINE_TRIGGERS,
                       "#%ld %s activated trigger at %d,%d.", mech_dbref(mech),
                       mech_display_id(mech).text, o->x, o->y);

    // Trigger the unit's AMECHDEST attribute.
    if (mech_dbref(mech) > 0)
      notify_event(btech_context_evaluation(mech_context(mech)), nullptr,
                   mech_dbref(mech), mech_dbref(mech), mech_dbref(mech),
                   LUA_EVENT_MECH_MINE_TRIGGER, nullptr, 0);

    return;
  case MINE_VIBRA:
    unset_hex_mine(map, o->x, o->y);
    if (o->x != x || o->y != y)
      hex_los_broadcast(map, o->x, o->y, "A mine explodes in $H!");
    mine_damage_mechs(&(MineDamageRequest){
        .map = map,
        .position = {.x = o->x, .y = o->y},
        .messages = {.target = "A blast of shrapnel hits you!",
                     .observers = "is hit by shrapnel!"},
        .neighbor_messages = {.target = "A little blast of shrapnel hits you!",
                              .observers = "is hit by some of the shrapnel!"},
        .damage = o->datas,
        .affect_neighbors = true});
    map_objects_delete(&(MapObjectLookupRequest){
        .map = map,
        .position = {.x = o->x, .y = o->y},
        .type = TYPE_MINE,
    });
    break;
  }
  mine_fields_recalculate(map);
}

/* we find the mine(s) that cause this (vibras can do it long-distance),
   and eliminate it */

static void possible_mine_explosion(Mech *mech, BattleMap *map, int x, int y,
                                    int reason) {
  MapObject *o;
  MapObject *o2;
  int mdis = (mech_real_tonnage(mech) - 20) / 10;
  float x1;
  float y1;
  float x2;
  float y2;

  map_coord_to_real_coord(x, y, &x1, &y1);
  for (o = map->map_object[TYPE_MINE]; o; o = o2) {

    int real = 1;

    o2 = o->next;
    if (o->x == x && o->y == y) {

      switch (o->datac) {

      case MINE_TRIGGER:
        if (o->datas > mech_real_tonnage(mech))
          continue;
        break;
      case MINE_VIBRA:
        if (o->payload.scalar > mech_real_tonnage(mech))
          continue; /* No message, just boom */
        break;
      case MINE_COMMAND:
        mech_notify(mech, MECHALL,
                    "You spot small bomblets lying on the ground here..");
        continue;
      }

      if (!real)
        return;

      mine_explode(&(MineExplosion){.mech = mech,
                                    .map = map,
                                    .mine = o,
                                    .triggering_position = {.x = x, .y = y},
                                    .reason = reason});

    } else if (mine_type_is_vibrating(o->datac)) {

      if (o->datac == MINE_TRIGGER) {

        /* To small let it go */
        if (o->datas > mech_real_tonnage(mech))
          continue;

        map_coord_to_real_coord(o->x, o->y, &x2, &y2);

        /* Out side of range */
        /* Using round here because we get some funky ranges like
         * 0.999987 and 1.00000072 */
        if (nearbyintf(map_real_range(&(MapRealSegment){
                .start = {.x = x1, .y = y1},
                .end = {.x = x2, .y = y2},
            })) > (float)o->payload.scalar)
          continue;

        mine_explode(&(MineExplosion){.mech = mech,
                                      .map = map,
                                      .mine = o,
                                      .triggering_position = {.x = x, .y = y},
                                      .reason = reason});

      } else if (o->payload.scalar < mech_real_tonnage(mech)) {

        if (abs(o->x - x) <= mdis && abs(o->y - y) <= mdis) {

          /* Possible remote explosion */
          map_coord_to_real_coord(o->x, o->y, &x2, &y2);
          const long RANGE_LIMIT =
              (mech_real_tonnage(mech) - o->payload.scalar) / 10;
          if (map_real_range(&(MapRealSegment){
                  .start = {.x = x1, .y = y1},
                  .end = {.x = x2, .y = y2},
              }) > (float)RANGE_LIMIT)
            continue;

          mine_explode(&(MineExplosion){.mech = mech,
                                        .map = map,
                                        .mine = o,
                                        .triggering_position = {.x = x, .y = y},
                                        .reason = reason});
        }
      }
    }
  }
}

void mine_field_trigger(Mech *mech, MineTriggerReason reason) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int x = mech_position_x(mech);
  int y = mech_position_y(mech);

  if (!is_mine_hex(map, x, y))
    return;

  if (mech_position_z(mech) > (mech_real_terrain_get(mech) == ICE
                                   ? 0
                                   : battle_map_hex_elevation(map, x, y)))
    return;

  possible_mine_explosion(mech, map, x, y, reason);
}

void mine_field_possibly_remove(Mech *mech, int x, int y) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (!map)
    return;
  if (!is_mine_hex(map, x, y))
    return;

  /* Mine removal is intentionally disabled until trigger mines can be
     distinguished from ordinary minefields. */
}

/* for now, just put the hexes themselves ; vibras should have larger radius */
/* Added Exile's MINE_TRIGGER changes.  Can set a distance for the
   mine and it will add mines to the hexes within that range - Dany */
typedef struct MineFieldDefinition {
  BattleMap *map;
  MapHexPosition position;
  char type;
  int data;
} MineFieldDefinition;

static void add_mine_on_map(const MineFieldDefinition *definition) {
  BattleMap *map = definition->map;
  int x = definition->position.x;
  int y = definition->position.y;
  char type = definition->type;
  int data = definition->data;
  int x1;
  int y1;
  int mdis = (100 - data) / 10;
  int t = mdis * 3 / 2;

  if (type == MINE_TRIGGER) {

    float fx;
    float fy;
    float fx1;
    float fy1;

    /* Get the main hex's location in floating values */
    map_coord_to_real_coord(x, y, &fx, &fy);

    /* Loop through all the possible hexes within range
     * and add mines to those hexes if they are within
     * range */
    for (x1 = x - data; x1 <= x + data; x1++) {
      for (y1 = y - data; y1 <= y + data; y1++) {

        /* Check the range, if in range add a mine */
        /* We round because of weirdness with FindHexRange returning
         * values like 1.00215 */
        map_coord_to_real_coord(x1, y1, &fx1, &fy1);
        if (nearbyintf(map_real_range(&(MapRealSegment){
                .start = {.x = fx, .y = fy},
                .end = {.x = fx1, .y = fy1},
            })) <= ((float)data))
          set_hex_mine(map, x1, y1);
      }
    }

  } else if (type >= MINE_LOW && type <= MINE_HIGH) {

    if (mine_type_is_vibrating(type) && mdis) {
      for (x1 = x - mdis; x1 <= (x + mdis); x1++) {
        for (y1 = y - mdis; y1 <= (y + mdis); y1++)
          if ((abs(x1 - x) + abs(y1 - y)) <= t)
            if (!(x1 < 0 || y1 < 0 || x1 >= map->map_width ||
                  y1 >= map->map_height))
              set_hex_mine(map, x1, y1);
      }
    } else {
      set_hex_mine(map, x, y);
    }
  }
}

/* Re-set all the minefield bits on a map */
void mine_fields_recalculate(BattleMap *map) {
  MapObject *o;

  clear_hex_bits(map, 1);
  for (o = map->map_object[TYPE_MINE]; o; o = o->next) {
    add_mine_on_map(
        &(MineFieldDefinition){.map = map,
                               .position = {.x = o->x, .y = o->y},
                               .type = clamp_int_to_char(o->datac),
                               .data = clamp_intptr_to_int(o->payload.scalar)});
  }
}

/* x y type strength <optvalue> */
void mine_command_add(DbRef player, void *data, char *buffer) {

  char *args[6];
  int argc;
  int x;
  int y;
  int str;
  int type;
  int extra = 0;
  BattleMap *map = (BattleMap *)data;
  MapObject foo;

  if (!map)
    return;

  argc = mech_parseattributes(buffer, args, 6);
  if (argc < 4 || argc > 5) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "Invalid arguments!");
    return;
  }
  if (!parse_int_checked(args[0], &x)) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "Invalid number!");
    return;
  }
  if (!parse_int_checked(args[1], &y)) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "Invalid number!");
    return;
  }
  if (!parse_int_checked(args[3], &str)) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "Invalid number!");
    return;
  }

  if (argc == 5 && !parse_int_checked(args[4], &extra)) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "Invalid number!");
    return;
  }

  type = mine_type_index(args[2]);
  if (type < 0) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "Invalid mine type!");
    return;
  }
  if (!((x >= 0) && (x < map->map_width) && (y >= 0) &&
        (y < map->map_height))) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "X,Y out of range!");
    return;
  }

  memset(&foo, 0, sizeof(foo));
  foo.x = clamp_int_to_short(x);
  foo.y = clamp_int_to_short(y);
  foo.payload.scalar = extra;
  foo.datas = clamp_int_to_short(str);
  foo.datac = type + 1;
  foo.obj = player;
  add_mapobj(map, &map->map_object[TYPE_MINE], &foo, 1);

  notify_printf(btech_context_evaluation(battle_map_context(map)), player,
                "%s mine added to (%d,%d) (strength: %d / extra: %d)",
                mine_type_name(type), x, y, str, extra);
  mine_fields_recalculate(map);
}

void mine_command_detonate(Mech *mech, int channel) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  MapObject *o;
  MapObject *o2;
  int count = 0;

  if (!map)
    return;
  for (o = map->map_object[TYPE_MINE]; o; o = o2) {
    o2 = o->next;
    if (o->datac == MINE_COMMAND) {
      if (o->payload.scalar == channel) {
        mine_explode(&(MineExplosion){.mech = mech,
                                      .map = map,
                                      .mine = o,
                                      .reason = MINE_COMMAND_DETONATION});
        count++;
      }
    }
  }
  if (count)
    mine_fields_recalculate(map);
}

void mine_field_scan(const MineFieldScanRequest *request) {
  DbRef player = request->player;
  Mech *mech = request->mech;
  float range = request->range;
  int x = request->position.x;
  int y = request->position.y;
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  MapObject *o;

  if (!is_mine_hex(map, x, y)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You see nothing else of interest in the hex, either.");
    return;
  }

  for (o = map->map_object[TYPE_MINE]; o; o = o->next)
    if (o->x == x && o->y == y)
      break;

  if (!o) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You see nothing else of interest in the hex, either.");
    return;
  }
  if (btech_random_range(mech_context(mech), 2, 9) < ((int)range)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You see nothing else of interest in the hex, either.");
    return;
  }
  if (!made_perception_roll(mech, 0)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You see nothing else of interest in the hex, either.");
    return;
  }
  mech_notify(mech, MECHALL,
              "Small bomblets litter the hex, interesting... You vaguely "
              "recall them from some class or other.");
}
