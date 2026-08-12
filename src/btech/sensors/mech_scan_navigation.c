#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_coordinates.h"
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
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int map_signed_elevation(BattleMap *map, int x, int y) {
  const char TERRAIN = map_real_terrain_get(map, x, y);
  const int ELEVATION = (unsigned char)map_elevation_get(map, x, y);
  return TERRAIN == BATTLE_TERRAIN_WATER || TERRAIN == BATTLE_TERRAIN_ICE
             ? -ELEVATION
             : ELEVATION;
}

static float scaled_hex_elevation(int elevation) {
  return ZSCALE * (float)elevation;
}

static float map_scaled_elevation(BattleMap *map, int x, int y) {
  return scaled_hex_elevation(map_signed_elevation(map, x, y));
}

static bool parse_navigation_arguments(char *arguments[], size_t count,
                                       int values[]) {
  for (size_t index = 0; index < count; index++) {
    char *argument = *(char **)checked_storage_at((void *)arguments, count,
                                                  sizeof(*arguments), index);
    int *value =
        (int *)checked_storage_at(values, count, sizeof(*values), index);
    if (!parse_int_checked(argument, value))
      return false;
  }
  return true;
}

void mech_bearing(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *temp_mech = nullptr;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[4];
  int argc;
  int ix0;
  int iy0;
  float x0;
  float y0;
  int ix1;
  int iy1;
  int values[4];
  float x1;
  float y1;
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
        temp_mech =
            btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));
        if (temp_mech) {
          if (!mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                              mech_position_y(temp_mech),
                              mech_range_to(mech, temp_mech))) {
            mecha_notify(evaluation, player, "Target is not in line of sight!");
            return;
          }
        }
      }
      const MechTargetPositionResult TARGET_POSITION =
          mech_target_position(mech);
      if (!TARGET_POSITION.found) {
        mecha_notify(evaluation, player, "There is no default target!");
      } else {
        x1 = TARGET_POSITION.position.x;
        y1 = TARGET_POSITION.position.y;
        strcpy(buff, "Bearing to default target is: ");
      }
    } else if (argc == 2) {
      /* Bearing to X, Y */
      if (!parse_navigation_arguments(args, 2, values)) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        return;
      }
      ix1 = values[0];
      iy1 = values[1];
      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        (void)snprintf(buff, sizeof(buff), "Bearing to  %d,%d is: ", ix1, iy1);
        map_coord_to_real_coord(ix1, iy1, &x1, &y1);
      }
    } else if (argc == 4) {
      if (!parse_navigation_arguments(args, 4, values)) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        return;
      }
      ix0 = values[0];
      iy0 = values[1];
      ix1 = values[2];
      iy1 = values[3];

      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map) && ix0 >= 0 &&
            ix0 <= battle_map_width(mech_map) && iy0 >= 0 &&
            iy0 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        (void)snprintf(buff, sizeof(buff),
                       "Bearing to %d,%d from %d,%d is: ", ix1, iy1, ix0, iy0);
        map_coord_to_real_coord(ix0, iy0, &x0, &y0);
        map_coord_to_real_coord(ix1, iy1, &x1, &y1);
      }
    } else {
      mecha_notify(evaluation, player,
                   "Invalid number of attributes to Bearing function!");
    }
    if (x1 >= 0.0F) {
      const int BEARING = map_bearing(&(MapRealSegment){
          .start = {.x = x0, .y = y0}, .end = {.x = x1, .y = y1}});
      (void)snprintf(trash, sizeof(trash), "%d degrees.", BEARING);
      strlcat(buff, trash, sizeof(buff));
      mecha_notify(evaluation, player, buff);
    }
  } else {
    mecha_notify(evaluation, player, "You are not on a map!");
  }
}

void mech_range(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *temp_mech = nullptr;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[4];
  int argc;
  int ix0;
  int iy0;
  float x0;
  float y0;
  float z0;
  int ix1;
  int iy1;
  int values[4];
  float x1;
  float y1;
  float z1 = 0;
  float hr;
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
        temp_mech =
            btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));
        if (temp_mech) {
          if (!mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                              mech_position_y(temp_mech),
                              mech_range_to(mech, temp_mech))) {
            mecha_notify(evaluation, player, "Target is not in line of sight!");
            return;
          }
        }
      }
      const MechTargetPositionResult TARGET_POSITION =
          mech_target_position(mech);
      if (!TARGET_POSITION.found) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "There is no default target!");
        return;
      }
      x1 = TARGET_POSITION.position.x;
      y1 = TARGET_POSITION.position.y;
      z1 = TARGET_POSITION.position.z;
      if (battle_map_is_dark(mech_map) && !temp_mech)
        z1 = scaled_hex_elevation(mech_position_z(mech));
      strcpy(buff, "Range to default target is: ");
    } else if (argc == 2) {
      /* Range to X, Y */
      if (!parse_navigation_arguments(args, 2, values)) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        return;
      }
      ix1 = values[0];
      iy1 = values[1];
      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        (void)snprintf(buff, sizeof(buff), "Range to  %d,%d is: ", ix1, iy1);
        map_coord_to_real_coord(ix1, iy1, &x1, &y1);
        if (battle_map_is_dark(mech_map))
          z1 = scaled_hex_elevation(mech_position_z(mech));
        else
          z1 = map_scaled_elevation(mech_map, ix1, iy1);
      }
    } else if (argc == 4) {
      /* Range to X, Y from given X, Y */
      if (!parse_navigation_arguments(args, 4, values)) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        return;
      }
      ix0 = values[0];
      iy0 = values[1];
      ix1 = values[2];
      iy1 = values[3];

      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map) && ix0 >= 0 &&
            ix0 <= battle_map_width(mech_map) && iy0 >= 0 &&
            iy0 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        (void)snprintf(buff, sizeof(buff),
                       "Range to %d,%d from %d,%d is: ", ix1, iy1, ix0, iy0);
        map_coord_to_real_coord(ix1, iy1, &x1, &y1);
        map_coord_to_real_coord(ix0, iy0, &x0, &y0);
        if (battle_map_is_dark(mech_map)) {
          z1 = z0 = 0;
        } else {
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
      temp = map_spatial_range(&(MapSpatialSegment){
          .start = {.x = x0, .y = y0, .z = z0},
          .end = {.x = x1, .y = y1, .z = z1},
      });
      hr = map_real_range(&(MapRealSegment){
          .start = {.x = x0, .y = y0},
          .end = {.x = x1, .y = y1},
      });
      (void)snprintf(buf1, sizeof(buf1), "%.1f", (double)temp);
      (void)snprintf(buf2, sizeof(buf2), "%.1f", (double)hr);
      if (strcmp(buf1, buf2))
        (void)snprintf(trash, sizeof(trash), "%s hexes (%s ground hexes).",
                       buf1, buf2);
      else
        (void)snprintf(trash, sizeof(trash), "%s hexes.", buf1);
      strlcat(buff, trash, sizeof(buff));
      mecha_notify(evaluation, player, buff);
    }
  } else {
    mecha_notify(evaluation, player, "You are not on a map!");
  }
}

void mech_vector(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *temp_mech = nullptr;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[6];
  int argc;
  int ix0;
  int iy0;
  int iz0;
  float x0;
  float y0;
  float z0;
  int ix1;
  int iy1;
  int iz1;
  int values[6];
  float x1;
  float y1;
  float z1 = 0;
  float hr;
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
        temp_mech =
            btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));
        if (temp_mech) {
          if (!mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                              mech_position_y(temp_mech),
                              mech_range_to(mech, temp_mech))) {
            mecha_notify(evaluation, player, "Target is not in line of sight!");
            return;
          }
        }
      }
      const MechTargetPositionResult TARGET_POSITION =
          mech_target_position(mech);
      if (!TARGET_POSITION.found) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "There is no default target!");
        return;
      }
      x1 = TARGET_POSITION.position.x;
      y1 = TARGET_POSITION.position.y;
      z1 = TARGET_POSITION.position.z;
      strcpy(buff, "Vector to default target is: ");
    } else if (argc == 2) {
      /* Range to X, Y */
      if (!parse_navigation_arguments(args, 2, values)) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        return;
      }
      ix1 = values[0];
      iy1 = values[1];
      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        (void)snprintf(buff, sizeof(buff), "Vector to  %d,%d is: ", ix1, iy1);
        map_coord_to_real_coord(ix1, iy1, &x1, &y1);
        z1 = map_scaled_elevation(mech_map, ix1, iy1);
      }
    } else if (argc == 3) {
      if (!parse_navigation_arguments(args, 3, values)) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        return;
      }
      ix1 = values[0];
      iy1 = values[1];
      iz1 = values[2];
      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        (void)snprintf(buff, sizeof(buff), "Vector to  %d,%d,%d is: ", ix1, iy1,
                       iz1);
        map_coord_to_real_coord(ix1, iy1, &x1, &y1);
        z1 = scaled_hex_elevation(iz1);
      }
    } else if (argc == 4) {
      /* Range to X, Y from given X, Y */
      if (!parse_navigation_arguments(args, 4, values)) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        return;
      }
      ix0 = values[0];
      iy0 = values[1];
      ix1 = values[2];
      iy1 = values[3];

      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map) && ix0 >= 0 &&
            ix0 <= battle_map_width(mech_map) && iy0 >= 0 &&
            iy0 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        (void)snprintf(buff, sizeof(buff),
                       "Vector to %d,%d from %d,%d is: ", ix1, iy1, ix0, iy0);
        map_coord_to_real_coord(ix1, iy1, &x1, &y1);
        map_coord_to_real_coord(ix0, iy0, &x0, &y0);
        z1 = map_scaled_elevation(mech_map, ix1, iy1);
        z0 = map_scaled_elevation(mech_map, ix0, iy0);
      }
    } else if (argc == 6) {
      if (!parse_navigation_arguments(args, 6, values)) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        return;
      }
      ix0 = values[0];
      iy0 = values[1];
      iz0 = values[2];
      ix1 = values[3];
      iy1 = values[4];
      iz1 = values[5];

      if (!(ix1 >= 0 && ix1 < battle_map_width(mech_map) && iy1 >= 0 &&
            iy1 < battle_map_height(mech_map) && ix0 >= 0 &&
            ix0 <= battle_map_width(mech_map) && iy0 >= 0 &&
            iy0 < battle_map_height(mech_map))) {
        mecha_notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        (void)snprintf(buff, sizeof(buff),
                       "Vector to %d,%d,%d from %d,%d,%d is: ", ix1, iy1, iz1,
                       ix0, iy0, iz0);
        map_coord_to_real_coord(ix1, iy1, &x1, &y1);
        map_coord_to_real_coord(ix0, iy0, &x0, &y0);
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
      temp = map_spatial_range(&(MapSpatialSegment){
          .start = {.x = x0, .y = y0, .z = z0},
          .end = {.x = x1, .y = y1, .z = z1},
      });
      hr = map_real_range(&(MapRealSegment){
          .start = {.x = x0, .y = y0},
          .end = {.x = x1, .y = y1},
      });
      (void)snprintf(buf1, sizeof(buf1), "%.1f", (double)temp);
      (void)snprintf(buf2, sizeof(buf2), "%.1f", (double)hr);
      if (strcmp(buf1, buf2))
        (void)snprintf(trash, sizeof(trash), "%s hexes (%s ground hexes) and ",
                       buf1, buf2);
      else
        (void)snprintf(trash, sizeof(trash), "%s hexes and ", buf1);
      strlcat(buff, trash, sizeof(buff));

      /* bearing */
      const int BEARING = map_bearing(&(MapRealSegment){
          .start = {.x = x0, .y = y0}, .end = {.x = x1, .y = y1}});
      if (argc != 0 && argc != 3 && argc != 6)
        (void)snprintf(trash, sizeof(trash), "%d degrees.", BEARING);
      else {
        (void)snprintf(trash, sizeof(trash), "%d degrees mark %c%d.", BEARING,
                       (z1 > z0   ? '+'
                        : z1 < z0 ? '-'
                                  : ' '),
                       map_vertical_bearing(&(MapSpatialSegment){
                           .start = {.x = x0, .y = y0, .z = z0},
                           .end = {.x = x1, .y = y1, .z = z1}}));
      }
      strlcat(buff, trash, sizeof(buff));

      mecha_notify(evaluation, player, buff);
    }
  } else {
    mecha_notify(evaluation, player, "You are not on a map!");
  }
}
