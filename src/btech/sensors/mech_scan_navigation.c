#include "mech_identity_api.h"
#include "mech_scan_internal.h"

void mech_bearing(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *tempMech = NULL;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[4];
  int argc;
  int ix0, iy0;
  float x0, y0;
  int ix1, iy1;
  float x1, y1, z1;
  float temp;
  char trash[20] = {0};
  char buff[100] = {0};

  x1 = y1 = -1;

  cch(MECH_USUAL);
  x0 = MechFX(mech);
  y0 = MechFY(mech);
  if (mech_map_dbref(mech) != -1) {
    mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
    argc = mech_parseattributes(buffer, args, 4);
    if (argc == 0) {
      /* Bearing to current target */
      if (MechTarget(mech) != -1) {
        tempMech = btech_context_get_mech(mech_context(mech), MechTarget(mech));
        if (tempMech) {
          if (!InLineOfSight(mech, tempMech, MechX(tempMech), MechY(tempMech),
                             FaMechRange(mech, tempMech))) {
            notify(evaluation, player, "Target is not in line of sight!");
            return;
          }
        }
      }
      if (!FindTargetXY(mech, &x1, &y1, &z1)) {
        notify(evaluation, player, "There is no default target!");
      } else {
        strcpy(buff, "Bearing to default target is: ");
      }
    } else if (argc == 2) {
      /* Bearing to X, Y */
      ix1 = atoi(args[0]);
      iy1 = atoi(args[1]);
      if (!(ix1 >= 0 && ix1 < mech_map->map_width && iy1 >= 0 &&
            iy1 < mech_map->map_height)) {
        notify(evaluation, player, "Invalid map coordinates!");
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

      if (!(ix1 >= 0 && ix1 < mech_map->map_width && iy1 >= 0 &&
            iy1 < mech_map->map_height && ix0 >= 0 &&
            ix0 <= mech_map->map_width && iy0 >= 0 &&
            iy0 < mech_map->map_height)) {
        notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        snprintf(buff, sizeof(buff), "Bearing to %d,%d from %d,%d is: ", ix1,
                 iy1, ix0, iy0);
        MapCoordToRealCoord(ix0, iy0, &x0, &y0);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
      }
    } else {
      notify(evaluation, player,
             "Invalid number of attributes to Bearing function!");
    }
    if (x1 != -1) {
      temp = FindBearing(x0, y0, x1, y1);
      snprintf(trash, sizeof(trash), "%.0f degrees.", temp);
      strcat(buff, trash);
      notify(evaluation, player, buff);
    }
  } else {
    notify(evaluation, player, "You are not on a map!");
  }
}

void mech_range(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *tempMech = NULL;
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

  cch(MECH_USUAL);
  x0 = MechFX(mech);
  y0 = MechFY(mech);
  z0 = MechFZ(mech);
  if (mech_map_dbref(mech) != -1) {
    mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
    argc = mech_parseattributes(buffer, args, 4);
    if (argc == 0) {
      /* Range to current target */
      if (MechTarget(mech) != -1) {
        tempMech = btech_context_get_mech(mech_context(mech), MechTarget(mech));
        if (tempMech) {
          if (!InLineOfSight(mech, tempMech, MechX(tempMech), MechY(tempMech),
                             FaMechRange(mech, tempMech))) {
            notify(evaluation, player, "Target is not in line of sight!");
            return;
          }
        }
      }
      DOCHECK_CONTEXT(mech_context(mech), !FindTargetXY(mech, &x1, &y1, &z1),
                      "There is no default target!");
      if (MapIsDark(mech_map) && !tempMech)
        z1 = ZSCALE * MechZ(mech);
      strcpy(buff, "Range to default target is: ");
    } else if (argc == 2) {
      /* Range to X, Y */
      ix1 = atoi(args[0]);
      iy1 = atoi(args[1]);
      if (!(ix1 >= 0 && ix1 < mech_map->map_width && iy1 >= 0 &&
            iy1 < mech_map->map_height)) {
        notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        snprintf(buff, sizeof(buff), "Range to  %d,%d is: ", ix1, iy1);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        if (MapIsDark(mech_map))
          z1 = ZSCALE * MechZ(mech);
        else
          z1 = ZSCALE * Elevation(mech_map, ix1, iy1);
      }
    } else if (argc == 4) {
      /* Range to X, Y from given X, Y */
      ix0 = atoi(args[0]);
      iy0 = atoi(args[1]);
      ix1 = atoi(args[2]);
      iy1 = atoi(args[3]);

      if (!(ix1 >= 0 && ix1 < mech_map->map_width && iy1 >= 0 &&
            iy1 < mech_map->map_height && ix0 >= 0 &&
            ix0 <= mech_map->map_width && iy0 >= 0 &&
            iy0 < mech_map->map_height)) {
        notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        snprintf(buff, sizeof(buff), "Range to %d,%d from %d,%d is: ", ix1, iy1,
                 ix0, iy0);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        MapCoordToRealCoord(ix0, iy0, &x0, &y0);
        if (MapIsDark(mech_map))
          z1 = z0 = 0;
        else {
          z1 = ZSCALE * Elevation(mech_map, ix1, iy1);
          z0 = ZSCALE * Elevation(mech_map, ix0, iy0);
        }
      }
    } else {
      notify(evaluation, player,
             "Invalid number of attributes to Range function!");
      x1 = y1 = -1;
    }
    if (x1 != -1) {
      temp = FindRange(x0, y0, z0, x1, y1, z1);
      hr = FindHexRange(x0, y0, x1, y1);
      snprintf(buf1, sizeof(buf1), "%.1f", temp);
      snprintf(buf2, sizeof(buf2), "%.1f", hr);
      if (strcmp(buf1, buf2))
        snprintf(trash, sizeof(trash), "%s hexes (%s ground hexes).", buf1,
                 buf2);
      else
        snprintf(trash, sizeof(trash), "%s hexes.", buf1);
      strcat(buff, trash);
      notify(evaluation, player, buff);
    }
  } else {
    notify(evaluation, player, "You are not on a map!");
  }
}

void mech_vector(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *tempMech = NULL;
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

  cch(MECH_USUAL);
  x0 = MechFX(mech);
  y0 = MechFY(mech);
  z0 = MechFZ(mech);
  if (mech_map_dbref(mech) != -1) {
    mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
    argc = mech_parseattributes(buffer, args, 6);
    if (argc == 0) {
      /* Range to current target */
      if (MechTarget(mech) != -1) {
        tempMech = btech_context_get_mech(mech_context(mech), MechTarget(mech));
        if (tempMech) {
          if (!InLineOfSight(mech, tempMech, MechX(tempMech), MechY(tempMech),
                             FaMechRange(mech, tempMech))) {
            notify(evaluation, player, "Target is not in line of sight!");
            return;
          }
        }
      }
      DOCHECK_CONTEXT(mech_context(mech), !FindTargetXY(mech, &x1, &y1, &z1),
                      "There is no default target!");
      strcpy(buff, "Vector to default target is: ");
    } else if (argc == 2) {
      /* Range to X, Y */
      ix1 = atoi(args[0]);
      iy1 = atoi(args[1]);
      if (!(ix1 >= 0 && ix1 < mech_map->map_width && iy1 >= 0 &&
            iy1 < mech_map->map_height)) {
        notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        snprintf(buff, sizeof(buff), "Vector to  %d,%d is: ", ix1, iy1);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        z1 = ZSCALE * Elevation(mech_map, ix1, iy1);
      }
    } else if (argc == 3) {
      iz0 = z0 / ZSCALE;
      ix1 = atoi(args[0]);
      iy1 = atoi(args[1]);
      iz1 = atoi(args[2]);
      if (!(ix1 >= 0 && ix1 < mech_map->map_width && iy1 >= 0 &&
            iy1 < mech_map->map_height)) {
        notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1.;
      } else {
        snprintf(buff, sizeof(buff), "Vector to  %d,%d,%d is: ", ix1, iy1, iz1);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        z1 = ZSCALE * iz1;
      }
    } else if (argc == 4) {
      /* Range to X, Y from given X, Y */
      ix0 = atoi(args[0]);
      iy0 = atoi(args[1]);
      ix1 = atoi(args[2]);
      iy1 = atoi(args[3]);

      if (!(ix1 >= 0 && ix1 < mech_map->map_width && iy1 >= 0 &&
            iy1 < mech_map->map_height && ix0 >= 0 &&
            ix0 <= mech_map->map_width && iy0 >= 0 &&
            iy0 < mech_map->map_height)) {
        notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        snprintf(buff, sizeof(buff), "Vector to %d,%d from %d,%d is: ", ix1,
                 iy1, ix0, iy0);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        MapCoordToRealCoord(ix0, iy0, &x0, &y0);
        z1 = ZSCALE * Elevation(mech_map, ix1, iy1);
        z0 = ZSCALE * Elevation(mech_map, ix0, iy0);
      }
    } else if (argc == 6) {
      ix0 = atoi(args[0]);
      iy0 = atoi(args[1]);
      iz0 = atoi(args[2]);
      ix1 = atoi(args[3]);
      iy1 = atoi(args[4]);
      iz1 = atoi(args[5]);

      if (!(ix1 >= 0 && ix1 < mech_map->map_width && iy1 >= 0 &&
            iy1 < mech_map->map_height && ix0 >= 0 &&
            ix0 <= mech_map->map_width && iy0 >= 0 &&
            iy0 < mech_map->map_height)) {
        notify(evaluation, player, "Invalid map coordinates!");
        x1 = y1 = -1;
      } else {
        snprintf(buff, sizeof(buff),
                 "Vector to %d,%d,%d from %d,%d,%d is: ", ix1, iy1, iz1, ix0,
                 iy0, iz0);
        MapCoordToRealCoord(ix1, iy1, &x1, &y1);
        MapCoordToRealCoord(ix0, iy0, &x0, &y0);
        z1 = ZSCALE * iz1;
        z0 = ZSCALE * iz0;
      }

    } else {
      notify(evaluation, player,
             "Invalid number of attributes to Vector function!");
      x1 = y1 = -1;
    }
    if (x1 != -1) {
      /* range */
      temp = FindRange(x0, y0, z0, x1, y1, z1);
      hr = FindHexRange(x0, y0, x1, y1);
      snprintf(buf1, sizeof(buf1), "%.1f", temp);
      snprintf(buf2, sizeof(buf2), "%.1f", hr);
      if (strcmp(buf1, buf2))
        snprintf(trash, sizeof(trash), "%s hexes (%s ground hexes) and ", buf1,
                 buf2);
      else
        snprintf(trash, sizeof(trash), "%s hexes and ", buf1);
      strcat(buff, trash);

      /* bearing */
      temp = FindBearing(x0, y0, x1, y1);
      if (argc != 0 && argc != 3 && argc != 6)
        snprintf(trash, sizeof(trash), "%.0f degrees.", temp);
      else
        snprintf(trash, sizeof(trash), "%.0f degrees mark %c%d.", temp,
                 (z1 > z0   ? '+'
                  : z1 < z0 ? '-'
                            : ' '),
                 FindZBearing(x0, y0, z0, x1, y1, z1));
      strcat(buff, trash);

      notify(evaluation, player, buff);
    }
  } else {
    notify(evaluation, player, "You are not on a map!");
  }
}
