
/* Implements unit line-of-sight calculations. */

#include "btech/context.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "map_conditions_api.h"
#include "map_los_api.h"
#include "map_los_types.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_lostracer_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_sensor_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"

/* 'nice' sensor stuff's in the mech.sensor.c ; nasty brute code
   lies here */

/* from here onwards.. black magic happens. Enter if you're sure of
   your peace of mind :-P */

/* -------------------------------------------------------------------- */

static bool terrain_is_water(int terrain) {
  return (terrain == BATTLE_TERRAIN_ICE || terrain == BATTLE_TERRAIN_WATER ||
          terrain == BATTLE_TERRAIN_BRIDGE) != 0;
}

static bool mech_is_in_water(Mech *mech) {
  return (terrain_is_water(mech_real_terrain_get(mech)) &&
          mech_position_z(mech) < 0) != 0;
}

static bool mech_is_water_beast(const Mech *mech) {
  return (mech_movement_type(mech) == MOVE_HULL ||
          mech_movement_type(mech) == MOVE_FOIL) != 0;
}

static float mech_los_position_z(const Mech *mech) {
  const int ELEVATION = mech_position_z(mech);
  return (float)ELEVATION;
}

float mech_los_actual_elevation(BattleMap *map, int x, int y, Mech *mech) {

  if (!map)
    return 0.0;
  if (!mech) {
    const int ELEVATION = battle_map_hex_elevation(map, x, y);
    return (float)ELEVATION + 0.1F;
  }
  if (mech_class(mech) == CLASS_MECH && !mech_is_fallen(mech))
    return mech_los_position_z(mech) + 1.5F;
  if (mech_movement_type(mech) == MOVE_NONE)
    return mech_los_position_z(mech) + 1.5F;
  if (mech_is_dropship(mech))
    return mech_los_position_z(mech) + 2.5F +
           (mech_class(mech) == CLASS_DS ? 0.0F : 2.0F);
  if (mech_condition_summary(mech).dug_in)
    return mech_los_position_z(mech) + 0.1F;
  return mech_los_position_z(mech) + 0.5F;
}

/* from/mech: mech _mech_ seeing _target_ on map _map_, _ff_
   is the previous flag (or seeing _x_,_y_ if _target_ is NULL),
   hexRange is range in hexes */
int mech_los_calculate_flags(const MechLosCalculation *calculation) {
  Mech *mech = calculation->observer;
  Mech *target = calculation->target;
  BattleMap *map = calculation->map;
  const int X = calculation->target_hex.x;
  const int Y = calculation->target_hex.y;
  const int PREVIOUS_FLAGS = calculation->previous_flags;
  const float HEX_RANGE = calculation->hex_range;
  int new_flag = (PREVIOUS_FLAGS & (BATTLE_MAP_LOS_SEEN)) +
                 BATTLE_MAP_LOS_TERRAIN_CALCULATED;
  int woods_count = 0;
  int water_count = 0;
  int height;
  int i;
  int pos_x;
  int pos_y;
  float pos_z;
  float z_inc;
  float end_z;
  int terrain;
  int dopartials = 0;
  int underwater;
  int bothworlds;
  int t_underwater;
  int t_bothworlds;
  int uwatercount = 0;
  int coordcount;
  LosTrace trace;

  /* A Hex target off the map? Don't bother */
  if (!target && !battle_map_coordinate_is_valid(map, X, Y))
    return new_flag + BATTLE_MAP_LOS_BLOCKED;

  /* Outside max sensor range in the worst case? Don't bother. */
  const int MAXIMUM_VISIBILITY =
      ((mech_technology_flags(mech) & AA_TECH) ||
       (target && (mech_technology_flags(target) & AA_TECH)))
          ? 180
          : battle_map_maximum_visibility(map);
  if (HEX_RANGE > (float)MAXIMUM_VISIBILITY)
    return new_flag + BATTLE_MAP_LOS_BLOCKED;

  /* We start at the observer hex and wind up at (x,y). */
  pos_x = mech_position_x(mech);
  pos_y = mech_position_y(mech);
  pos_z = mech_los_actual_elevation(map, pos_x, pos_y, mech);
  end_z = mech_los_actual_elevation(map, X, Y, target);

  /* Definition of 'both worlds': According to FASA, if a mech is half
     submerged, or a sub is surfaced, or any naval or hover is on top
     of the water, it can see in both the underwater and overwater 'worlds'.
     In other words, it'll never get a block from the water/air interface.
     Neither will anything get such a block against it. That's what the
     'both worlds' variables test for. */

  if (end_z > 10 && pos_z > 10)
    return new_flag;
  bothworlds =
      terrain_is_water(mech_real_terrain_get(mech)) &&
      (((mech_class(mech) == CLASS_MECH) && (mech_position_z(mech) == -1)) ||
       (mech_is_water_beast(mech) && (mech_position_z(mech) == 0)) ||
       ((mech_movement_type(mech) == MOVE_HOVER) &&
        (mech_position_z(mech) == 0)));
  underwater = mech_is_in_water(mech) && (pos_z < 0.0F);

  /* Ice hex targeting special case */
  if (!target && !underwater &&
      map_real_terrain_get(map, X, Y) == BATTLE_TERRAIN_ICE)
    end_z = 0.0;

  if (target) {
    /* What about him? Both worlds? Or flat out underwater? */
    t_bothworlds =
        terrain_is_water(mech_real_terrain_get(target)) &&
        (((mech_class(target) == CLASS_MECH) &&
          (mech_position_z(target) == -1)) ||
         (mech_is_water_beast(target) && (mech_position_z(target) == 0)) ||
         ((mech_movement_type(target) == MOVE_HOVER) &&
          (mech_position_z(target) == 0)));

    t_underwater = mech_is_in_water(target) && (end_z < 0.0F);
  } else {
    if (map_real_terrain_get(map, X, Y) == BATTLE_TERRAIN_ICE)
      t_bothworlds = 1;
    else
      t_bothworlds = 0;
    t_underwater = (end_z < 0.0F); /* Is the hex underwater? */
  }

  /* And now we look once more to make sure we aren't wasting our time */
  if (((underwater) && !(t_underwater)) || ((t_underwater) && !(underwater))) {
    return new_flag + BATTLE_MAP_LOS_BLOCKED;
  }

  /* Worth our time to mess with figuring partial cover? */
  if (target && mech)
    dopartials = (mech_class(target) == CLASS_MECH) && !mech_is_fallen(target);

  /*Same hex is always LoS */
  if ((X == pos_x) && (Y == pos_y))
    return new_flag;

  /* Special cases are out of the way, looks like we have to do actual work. */
  coordcount = trace_los(map, pos_x, pos_y, X, Y, &trace);
  if (coordcount > 0) {
    z_inc = (end_z - pos_z) / (float)coordcount;
  } else {
    z_inc = 0; /* In theory, this should never happen. */
  }

  if (coordcount > 0) { /* not in same hex ; in same hex, you see always */
    for (i = 0; i < coordcount; i++) {
      const LosTracePoint *point = los_trace_point_at(&trace, i);
      pos_z += z_inc;
      if (!battle_map_coordinate_is_valid(map, point->x, point->y))
        continue;
      /* Should be possible to see into water.. perhaps. But not
         on vislight */
      terrain = (unsigned char)map_real_terrain_get(map, point->x, point->y);
      /* get the current height */
      height = battle_map_hex_elevation(map, point->x, point->y);
      const float HEIGHT_AS_FLOAT = (float)height;

      /* If you, persoanlly, are underwater, the only way you can see someone
         if if they are underwater or in both worlds AND your LoS passes thru
         nothing but water hexes AND your LoS doesn't go thru the sea floor */
      if (underwater) {

        /* LoS hits sea floor */
        if (!(terrain_is_water(terrain)) ||
            (terrain != BATTLE_TERRAIN_BRIDGE && HEIGHT_AS_FLOAT >= pos_z)) {
          new_flag |= BATTLE_MAP_LOS_BLOCKED;
          return new_flag;
        }

        /* LoS pops out of water, AND we're not tracing to half-submerged mech's
         * head */
        if (!t_bothworlds && pos_z > 0.0F) {
          new_flag |= BATTLE_MAP_LOS_BLOCKED;
          return new_flag;
        }

        /* uwatercount = # hexes LoS travel UNDERWATER */
        if (pos_z <= 0.0F)
          uwatercount++;
        water_count++;
      } else { /* Viewer is not underwater */
        /* keep track of how many wooded hexes we cross */
        if (pos_z < HEIGHT_AS_FLOAT + 2.0F) {
          switch (map_terrain_get(map, point->x, point->y)) {
          case BATTLE_TERRAIN_SMOKE:
            if (i < coordcount - 1)
              new_flag |= BATTLE_MAP_LOS_SMOKE;
            break;
          case BATTLE_TERRAIN_FIRE:
            if (i < coordcount - 1)
              new_flag |= BATTLE_MAP_LOS_FIRE;
            break;
          }
          switch (terrain) {
          case BATTLE_TERRAIN_LIGHT_FOREST:
          case BATTLE_TERRAIN_HEAVY_FOREST:
            if (i < coordcount - 1)
              woods_count += (terrain == BATTLE_TERRAIN_LIGHT_FOREST) ? 1 : 2;
            break;
          case BATTLE_TERRAIN_HIGH_WATER:
            water_count++;
            break;
          case BATTLE_TERRAIN_ICE:
            if (pos_z < -0.0F) {
              new_flag |= BATTLE_MAP_LOS_BLOCKED;
              return new_flag;
            }
            water_count++;
            break;
          case BATTLE_TERRAIN_WATER:

            /* LoS goes INTO water and we're not tracing to a target in both
             * worlds */
            if (!bothworlds && (pos_z < 0.0F)) {
              new_flag |= BATTLE_MAP_LOS_BLOCKED;
              return new_flag;
            }

            /* Hexes in LoS that are phsyically underwater */
            if (pos_z < 0.0F)
              uwatercount++;
            water_count++;
            break;
          case BATTLE_TERRAIN_MOUNTAINS:
            if (i < coordcount - 1)
              new_flag |= BATTLE_MAP_LOS_MOUNTAIN;
            break;
          }
        }
        /* make this the new 'current hex' */
        if (HEIGHT_AS_FLOAT >= pos_z && terrain != BATTLE_TERRAIN_BRIDGE) {
          new_flag |= BATTLE_MAP_LOS_BLOCKED;
          return new_flag;
        }
      }
    }
  }
  /* Then, we check the hex before target hex */

  if (coordcount >= 2) {
    if (dopartials) {
      const LosTracePoint *penultimate =
          los_trace_point_at(&trace, coordcount - 2);
      if (mech_position_z(target) >= mech_position_z(mech) &&
          (battle_map_hex_elevation(map, penultimate->x, penultimate->y) ==
           (mech_position_z(target) + 1)))
        new_flag |= BATTLE_MAP_LOS_PARTIAL_COVER;
      if (mech_position_z(target) == -1 &&
          mech_real_terrain_get(target) == BATTLE_TERRAIN_WATER)
        new_flag |= BATTLE_MAP_LOS_PARTIAL_COVER;
    }
  }

  water_count = bounded(0, water_count, BATTLE_MAP_LOS_MAX_WATER - 1);
  woods_count = bounded(0, woods_count, BATTLE_MAP_LOS_MAX_WOOD - 1);
  new_flag += BATTLE_MAP_LOS_WOOD * woods_count;
  new_flag += BATTLE_MAP_LOS_WATER * water_count;

  /* Block EM after 2, Vis/IR after 6 */
  if (uwatercount > 2)
    new_flag |= BATTLE_MAP_LOS_MOUNTAIN;
  if (uwatercount > 6)
    new_flag |= BATTLE_MAP_LOS_FIRE;
  return new_flag;
}

int mech_los_terrain_modifier(const MechLosTerrainRequest *request) {
  Mech *mech = request->observer;
  Mech *target = request->target;
  BattleMap *map = request->map;
  const int AMMUNITION_MODE = request->ammunition_mode;
  /* Possibly do a quickie check only */
  if (mech && target) {
    const int FLAGS =
        battle_map_los_flags(map, mech_map_slot(mech), mech_map_slot(target));
    mech_partial_cover_set(target, (FLAGS & BATTLE_MAP_LOS_PARTIAL_COVER) != 0);

    MechSensorToHitRequest sensor_request = {
        .observer = mech,
        .target = target,
        .los_flags = FLAGS,
        .map_light = battle_map_light(map),
        .ammunition_mode = AMMUNITION_MODE,
    };
    return mech_sensor_to_hit_bonus(&sensor_request);
  }
  return 0;
}

int mech_los_check_unblocked(Mech *mech, Mech *target, int x, int y,
                             float hex_range) {
  int i;

  if (mech == target)
    return 1;

  i = mech_los_check(mech, target, x, y, hex_range);
  if (i & BATTLE_MAP_LOS_BLOCKED)
    return 0;
  return i;
}

int mech_los_check(Mech *mech, Mech *target, int x, int y, float hex_range) {
  BattleMap *map;
  float x1;
  float y1;
  int arc;
  int losflag;

  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!map) {
    mech_notify(mech, MECHPILOT, "You are on an invalid map! Map index reset!");
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS,
                       "InLineOfSight:invalid map:Mech %ld  Index %ld",
                       mech_dbref(mech), mech_map_dbref(mech));
    mech_map_dbref_set(mech, NOTHING);
    return 0;
  }
  if (!battle_map_coordinate_is_valid(map, x, y)) {
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS,
                       "x:%d y:%d out of bounds for #%ld (LOS check)", x, y,
                       mech ? mech_dbref(mech) : -1);
  }

  /* Possibly do a quickie check only */
  if (mech_is_clairvoyant(mech))
    return 1;

  if (target) {
    x1 = mech_position_real_x(target);
    y1 = mech_position_real_y(target);
  } else {
    map_coord_to_real_coord(x, y, &x1, &y1);
  }
  arc = in_weapon_arc(mech, x1, y1);

  if (mech && target) {
    if (battle_map_los_flags(map, mech_map_slot(mech), mech_map_slot(target)) &
        (BATTLE_MAP_LOS_SEEN_PRIMARY | BATTLE_MAP_LOS_SEEN_SECONDARY))
      return battle_map_los_flags(map, mech_map_slot(mech),
                                  mech_map_slot(target)) &
             (BATTLE_MAP_LOS_SEEN_PRIMARY | BATTLE_MAP_LOS_SEEN_SECONDARY |
              BATTLE_MAP_LOS_BLOCKED);
    return 0;
  }
  losflag = mech_los_calculate_flags(&(MechLosCalculation){
      .observer = mech,
      .target = nullptr,
      .map = map,
      .target_hex = {.x = x, .y = y},
      .hex_range = hex_range,
  });
  MechSensorObservationRequest request = {
      .observer = mech,
      .los_flags = &losflag,
      .arc = arc,
      .range = hex_range,
      .map_visibility = battle_map_visibility(map),
      .map_light = battle_map_light(map),
      .cloud_base = battle_map_cloud_base(map),
  };
  return mech_sensor_can_see(&request);
}

void mech_losemit(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALSP))
    return;
  mech_los_broadcast(mech, buffer);
  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "Broadcast done.");
}
