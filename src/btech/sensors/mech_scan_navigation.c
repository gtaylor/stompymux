#include "btech/context.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_scan_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int map_signed_elevation(BattleMap *map, int x, int y) {
  const char terrain = map_real_terrain_get(map, x, y);
  const int elevation = map_elevation_get(map, x, y);
  return terrain == BATTLE_TERRAIN_WATER || terrain == BATTLE_TERRAIN_ICE
             ? -elevation
             : elevation;
}

static float scaled_hex_elevation(int elevation) {
  return ZSCALE * (float)elevation;
}

static float map_scaled_elevation(BattleMap *map, int x, int y) {
  return scaled_hex_elevation(map_signed_elevation(map, x, y));
}

void mech_bearing(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *tempMech = nullptr;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[4];
  int argc;
  int ix0, iy0;
  float x0, y0;
  int ix1, iy1;
  float x1, y1, z1;
  char trash[20] = {0};
  char buff[100] = {0};

  x1 = y1 = -1;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  x0 = mech_position_real_x(mech);
  y0 = mech_position_real_y(mech);
  if (mech_map_dbref(mech) != -1) {
    mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
    argc = mech_parseattributes(buffer, args, 4);
    if (argc == 0) {
      /* Bearing to current target */
      if (mech_target_dbref(mech) != -1) {
        tempMech =
            btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));
        if (tempMech) {
          if (!mech_los_check(mech, tempMech, mech_position_x(tempMech),
                              mech_position_y(tempMech),
                              mech_range_to(mech, tempMech))) {
            mecha_notify(evaluation, player, "Target is not in line of sight!");
            return;
          }
        }
      }
      if (!FindTargetXY(mech, &x1, &y1, &z1)) {
        mecha_notify(evaluation, player, "There is no default target!");
      } else {
        strcpy(buff, "Bearing to default target is: ");
      }
    } else if (argc == 2) {
      /* Bearing to X, Y */
      ix1 = atoi(args[0]);
      iy1 = atoi(args[1]);
      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        snprintf(buff, sizeof(buff), "Bearing to  %d,%d is: ", ix1, iy1);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
      }
    } else if (argc == 4) {
      ix0 = atoi(args[0]);
      iy0 = atoi(args[1]);
      ix1 = atoi(args[2]);
      iy1 = atoi(args[3]);

      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map) && ix0 >= 0 &&
            ix0 <= battle_map_width(mech_map) && iy0 >= 0 &&
            iy0 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        snprintf(buff, sizeof(buff), "Bearing to %d,%d from %d,%d is: ", ix1,
                 iy1, ix0, iy0);
        MapCoordToRealCoord(ix0, iy0, &x0, &y0);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
      }
    } else {
      mecha_notify(evaluation, player,
                   "Invalid number of attributes to Bearing function!");
    }
    if (x1 >= 0.0F) {
      const int bearing = FindBearing(x0, y0, x1, y1);
      snprintf(trash, sizeof(trash), "%d degrees.", bearing);
      strlcat(buff, trash, sizeof(buff));
      mecha_notify(evaluation, player, buff);
    }
  } else {
    mecha_notify(evaluation, player, "You are not on a map!");
  }
}

void mech_range(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *tempMech = nullptr;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[4];
  int argc;
  int ix0, iy0;
  float x0, y0, z0;
  int ix1, iy1;
  float x1, y1, z1 = 0, hr;
  float temp;
  char trash[80];
  char buff[100];
  char buf1[20];
  char buf2[20];

  x1 = y1 = -1;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  x0 = mech_position_real_x(mech);
  y0 = mech_position_real_y(mech);
  z0 = mech_position_real_z(mech);
  if (mech_map_dbref(mech) != -1) {
    mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
    argc = mech_parseattributes(buffer, args, 4);
    if (argc == 0) {
      /* Range to current target */
      if (mech_target_dbref(mech) != -1) {
        tempMech =
            btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));
        if (tempMech) {
          if (!mech_los_check(mech, tempMech, mech_position_x(tempMech),
                              mech_position_y(tempMech),
                              mech_range_to(mech, tempMech))) {
            mecha_notify(evaluation, player, "Target is not in line of sight!");
            return;
          }
        }
      }
      if (!FindTargetXY(mech, &x1, &y1, &z1)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "There is no default target!");
        return;
      }
      if (battle_map_is_dark(mech_map) && !tempMech)
        z1 = scaled_hex_elevation(mech_position_z(mech));
      strcpy(buff, "Range to default target is: ");
    } else if (argc == 2) {
      /* Range to X, Y */
      ix1 = atoi(args[0]);
      iy1 = atoi(args[1]);
      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        snprintf(buff, sizeof(buff), "Range to  %d,%d is: ", ix1, iy1);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        if (battle_map_is_dark(mech_map))
          z1 = scaled_hex_elevation(mech_position_z(mech));
        else
          z1 = map_scaled_elevation(mech_map, ix1, iy1);
      }
    } else if (argc == 4) {
      /* Range to X, Y from given X, Y */
      ix0 = atoi(args[0]);
      iy0 = atoi(args[1]);
      ix1 = atoi(args[2]);
      iy1 = atoi(args[3]);

      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map) && ix0 >= 0 &&
            ix0 <= battle_map_width(mech_map) && iy0 >= 0 &&
            iy0 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        snprintf(buff, sizeof(buff), "Range to %d,%d from %d,%d is: ", ix1, iy1,
                 ix0, iy0);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        MapCoordToRealCoord(ix0, iy0, &x0, &y0);
        if (battle_map_is_dark(mech_map))
          z1 = z0 = 0;
        else {
          z1 = map_scaled_elevation(mech_map, ix1, iy1);
          z0 = map_scaled_elevation(mech_map, ix0, iy0);
        }
      }
    } else {
      mecha_notify(evaluation, player,
                   "Invalid number of attributes to Range function!");
      x1 = y1 = -1;
    }
    if (x1 >= 0.0F) {
      temp = FindRange(x0, y0, z0, x1, y1, z1);
      hr = FindHexRange(x0, y0, x1, y1);
      snprintf(buf1, sizeof(buf1), "%.1f", (double)temp);
      snprintf(buf2, sizeof(buf2), "%.1f", (double)hr);
      if (strcmp(buf1, buf2))
        snprintf(trash, sizeof(trash), "%s hexes (%s ground hexes).", buf1,
                 buf2);
      else
        snprintf(trash, sizeof(trash), "%s hexes.", buf1);
      strlcat(buff, trash, sizeof(buff));
      mecha_notify(evaluation, player, buff);
    }
  } else {
    mecha_notify(evaluation, player, "You are not on a map!");
  }
}

void mech_vector(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *tempMech = nullptr;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[6];
  int argc;
  int ix0, iy0, iz0;
  float x0, y0, z0;
  int ix1, iy1, iz1;
  float x1, y1, z1 = 0, hr;
  float temp;
  char trash[80];
  char buff[100];
  char buf1[20];
  char buf2[20];

  x1 = y1 = -1;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  x0 = mech_position_real_x(mech);
  y0 = mech_position_real_y(mech);
  z0 = mech_position_real_z(mech);
  if (mech_map_dbref(mech) != -1) {
    mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
    argc = mech_parseattributes(buffer, args, 6);
    if (argc == 0) {
      /* Range to current target */
      if (mech_target_dbref(mech) != -1) {
        tempMech =
            btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));
        if (tempMech) {
          if (!mech_los_check(mech, tempMech, mech_position_x(tempMech),
                              mech_position_y(tempMech),
                              mech_range_to(mech, tempMech))) {
            mecha_notify(evaluation, player, "Target is not in line of sight!");
            return;
          }
        }
      }
      if (!FindTargetXY(mech, &x1, &y1, &z1)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "There is no default target!");
        return;
      }
      strcpy(buff, "Vector to default target is: ");
    } else if (argc == 2) {
      /* Range to X, Y */
      ix1 = atoi(args[0]);
      iy1 = atoi(args[1]);
      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        snprintf(buff, sizeof(buff), "Vector to  %d,%d is: ", ix1, iy1);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        z1 = map_scaled_elevation(mech_map, ix1, iy1);
      }
    } else if (argc == 3) {
      iz0 = clamp_float_to_int(z0 / ZSCALE);
      ix1 = atoi(args[0]);
      iy1 = atoi(args[1]);
      iz1 = atoi(args[2]);
      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        snprintf(buff, sizeof(buff), "Vector to  %d,%d,%d is: ", ix1, iy1, iz1);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        z1 = scaled_hex_elevation(iz1);
      }
    } else if (argc == 4) {
      /* Range to X, Y from given X, Y */
      ix0 = atoi(args[0]);
      iy0 = atoi(args[1]);
      ix1 = atoi(args[2]);
      iy1 = atoi(args[3]);

      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map) && ix0 >= 0 &&
            ix0 <= battle_map_width(mech_map) && iy0 >= 0 &&
            iy0 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        snprintf(buff, sizeof(buff), "Vector to %d,%d from %d,%d is: ", ix1,
                 iy1, ix0, iy0);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        MapCoordToRealCoord(ix0, iy0, &x0, &y0);
        z1 = map_scaled_elevation(mech_map, ix1, iy1);
        z0 = map_scaled_elevation(mech_map, ix0, iy0);
      }
    } else if (argc == 6) {
      ix0 = atoi(args[0]);
      iy0 = atoi(args[1]);
      iz0 = atoi(args[2]);
      ix1 = atoi(args[3]);
      iy1 = atoi(args[4]);
      iz1 = atoi(args[5]);

      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map) && ix0 >= 0 &&
            ix0 <= battle_map_width(mech_map) && iy0 >= 0 &&
            iy0 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        snprintf(buff, sizeof(buff),
                 "Vector to %d,%d,%d from %d,%d,%d is: ", ix1, iy1, iz1, ix0,
                 iy0, iz0);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        MapCoordToRealCoord(ix0, iy0, &x0, &y0);
        z1 = scaled_hex_elevation(iz1);
        z0 = scaled_hex_elevation(iz0);
      }

    } else {
      mecha_notify(evaluation, player,
                   "Invalid number of attributes to Vector function!");
      x1 = y1 = -1;
    }
    if (x1 >= 0.0F) {
      /* range */
      temp = FindRange(x0, y0, z0, x1, y1, z1);
      hr = FindHexRange(x0, y0, x1, y1);
      snprintf(buf1, sizeof(buf1), "%.1f", (double)temp);
      snprintf(buf2, sizeof(buf2), "%.1f", (double)hr);
      if (strcmp(buf1, buf2))
        snprintf(trash, sizeof(trash), "%s hexes (%s ground hexes) and ", buf1,
                 buf2);
      else
        snprintf(trash, sizeof(trash), "%s hexes and ", buf1);
      strlcat(buff, trash, sizeof(buff));

      /* bearing */
      const int bearing = FindBearing(x0, y0, x1, y1);
      if (argc != 0 && argc != 3 && argc != 6)
        snprintf(trash, sizeof(trash), "%d degrees.", bearing);
      else
        snprintf(trash, sizeof(trash), "%d degrees mark %c%d.", bearing,
                 (z1 > z0   ? '+'
                  : z1 < z0 ? '-'
                            : ' '),
                 FindZBearing(x0, y0, z0, x1, y1, z1));
      strlcat(buff, trash, sizeof(buff));

      mecha_notify(evaluation, player, buff);
    }
  } else {
    mecha_notify(evaluation, player, "You are not on a map!");
  }
}
