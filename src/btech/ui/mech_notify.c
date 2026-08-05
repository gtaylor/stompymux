#include "mech_notify.h"
#include "btech/context.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_crew_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_progress_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_api.h"
#include "mech_utils_api.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

static int map_base_elevation(BattleMap *map, int x, int y) {
  int elevation = map_elevation_get(map, x, y);
  char terrain = map_real_terrain_get(map, x, y);
  return terrain == WATER || terrain == ICE ? -elevation : elevation;
}

void mech_los_broadcast(Mech *mech, char *message) {
  /* Sends msg to everyone except the mech */
  int i;
  Mech *tempMech;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char buf[LBUF_SIZE];

  mech_sensor_visibility_refresh(mech);
  if (!mech_map)
    return;
  for (i = 0; i < mech_map->first_free; i++)
    if (mech_map->mechsOnMap[i] != -1 &&
        mech_map->mechsOnMap[i] != mech_dbref(mech))
      if ((tempMech = btech_context_get_mech(mech_map->xcode.context,
                                             mech_map->mechsOnMap[i])))
        if (mech_los_check(tempMech, mech, mech_position_x(mech),
                           mech_position_y(mech),
                           mech_range_to(tempMech, mech))) {
          snprintf(buf, sizeof(buf), "%s%s%s",
                   mech_to_mech_display_id(tempMech, mech).text,
                   *message != '\'' ? " " : "", message);
          mech_notify(tempMech, MECHSTARTED, buf);
        }
}

int MechSeesHexF(Mech *mech, BattleMap *map, float x, float y, int ix, int iy) {
  return mech_los_check(mech, nullptr, ix, iy,
                        FindRange(mech_position_real_x(mech),
                                  mech_position_real_y(mech),
                                  mech_position_real_z(mech), x, y,
                                  ZSCALE * map_base_elevation(map, ix, iy)));
}

int MechSeesHex(Mech *mech, BattleMap *map, int x, int y) {
  float fx, fy;

  MapCoordToRealCoord(x, y, &fx, &fy);
  return MechSeesHexF(mech, map, fx, fy, x, y);
}

void HexLOSBroadcast(BattleMap *mech_map, int x, int y, char *message) {
  int i;
  Mech *tempMech;
  float fx, fy;

  /* substitution:
     $h = !alarming ('your hex', '%d,%d')
     $H = alarming ('YOUR HEX', '%d,%d (%.2f away)')
   */
  if (!mech_map)
    return;
  MapCoordToRealCoord(x, y, &fx, &fy);
  for (i = 0; i < mech_map->first_free; i++)
    if (mech_map->mechsOnMap[i] != -1)
      if ((tempMech = btech_context_get_mech(mech_map->xcode.context,
                                             mech_map->mechsOnMap[i])))
        if (MechSeesHexF(tempMech, mech_map, fx, fy, x, y)) {
          char tbuf[LBUF_SIZE];
          char *c, *d = tbuf;
          int done;

          for (c = message; *c; c++) {
            done = 0;
            if (*c == '$') {
              if (*(c + 1) == 'h' || *(c + 1) == 'H') {
                c++;
                if (*c == 'h') {
                  if (x == mech_position_x(tempMech) &&
                      y == mech_position_y(tempMech))
                    strcpy(d, "your hex");
                  else
                    snprintf(d, sizeof(tbuf) - (tbuf - d), "%d,%d", x, y);
                  while (*d)
                    d++;
                } else {
                  /* Dangerous */
                  if (x == mech_position_x(tempMech) &&
                      y == mech_position_y(tempMech))
                    strcpy(d, "[fg=red bold]YOUR HEX[reset]");
                  else
                    snprintf(d, sizeof(tbuf) - (tbuf - d),
                             "[fg=yellow bold]%d,%d[reset]", x, y);
                  while (*d)
                    d++;
                }
                done = 1;
              }
            }
            if (!done)
              *(d++) = *c;
          }
          /* Apparently, it's necessary to remove trailing $'s ?? */
          if (d > tbuf && *(d - 1) == '$')
            d--;
          *d = '\0';
          mech_notify(tempMech, MECHSTARTED, tbuf);
        }
}

static void format_mech_los_message(char *buffer, size_t buffer_size,
                                    const char *message,
                                    const char *target_name) {
  const char *placeholder = strstr(message, "%s");

  if (!placeholder) {
    snprintf(buffer, buffer_size, "%s", message);
    return;
  }
  snprintf(buffer, buffer_size, "%.*s%s%s", (int)(placeholder - message),
           message, target_name, placeholder + 2);
}

void mech_los_broadcast_unit(Mech *mech, Mech *target, const char *message) {
  /* Sends msg to everyone except the mech */
  int i, a, b;
  char oddbuff[LBUF_SIZE];
  char oddbuff2[LBUF_SIZE];
  Mech *tempMech;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (!mech_map)
    return;
  mech_sensor_visibility_refresh(mech);
  mech_sensor_visibility_refresh(target);
  for (i = 0; i < mech_map->first_free; i++)
    if (mech_map->mechsOnMap[i] != -1 &&
        mech_map->mechsOnMap[i] != mech_dbref(mech) &&
        mech_map->mechsOnMap[i] != mech_dbref(target))
      if ((tempMech = btech_context_get_mech(mech_context(mech),
                                             mech_map->mechsOnMap[i]))) {
        a = mech_los_check(tempMech, mech, mech_position_x(mech),
                           mech_position_y(mech),
                           mech_range_to(tempMech, mech));
        b = mech_los_check(tempMech, target, mech_position_x(target),
                           mech_position_y(target),
                           mech_range_to(tempMech, target));
        if (a || b) {
          char *obp = oddbuff2;

          format_mech_los_message(
              oddbuff, sizeof(oddbuff), message,
              b ? mech_to_mech_display_id(tempMech, target).text : "someone");
          safe_str((char *)(a ? mech_to_mech_display_id(tempMech, mech).text
                              : "Someone"),
                   oddbuff2, &obp);
          if (*oddbuff != '\'')
            safe_chr(' ', oddbuff2, &obp);
          safe_str(oddbuff, oddbuff2, &obp);
          *obp = '\0';
          mech_notify(tempMech, MECHSTARTED, oddbuff2);
        }
      }
}

void MapBroadcast(BattleMap *map, char *message) {
  /* Sends msg to everyone except the mech */
  int i;
  Mech *tempMech;

  for (i = 0; i < map->first_free; i++)
    if (map->mechsOnMap[i] != -1)
      if ((tempMech =
               btech_context_get_mech(map->xcode.context, map->mechsOnMap[i])))
        mech_notify(tempMech, MECHSTARTED, message);
}

void MechFireBroadcast(Mech *mech, Mech *target, int x, int y,
                       BattleMap *mech_map, char *weapname, int IsHit) {
  int loop, attacker, defender;
  float fx, fy, fz;
  int mapx, mapy;
  Mech *tempMech;
  char buff[SBUF_SIZE];

  /* Stat recording */
  if (target) { /* only if we have a mecha */
    if (IsHit)
      mech_shot_result_record(mech, true);
    else
      mech_shot_result_record(mech, false);
  }

  mech_sensor_visibility_refresh(mech);
  if (target) {
    mech_sensor_visibility_refresh(target);
    mapx = mech_position_x(target);
    mapy = mech_position_y(target);
    fx = mech_position_real_x(target);
    fy = mech_position_real_y(target);
    fz = mech_position_real_z(target);
    for (loop = 0; loop < mech_map->first_free; loop++)
      if (mech_map->mechsOnMap[loop] != mech_dbref(mech) &&
          mech_map->mechsOnMap[loop] != -1 &&
          mech_map->mechsOnMap[loop] != mech_dbref(target)) {
        attacker = 0;
        defender = 0;
        tempMech = (Mech *)btech_context_find_object(
            mech_context(mech), mech_map->mechsOnMap[loop]);
        if (!tempMech)
          continue;
        if (mech_los_check(tempMech, mech, mech_position_x(mech),
                           mech_position_y(mech),
                           mech_range_to(tempMech, mech)))
          attacker = 1;
        if (target) {
          if (mech_los_check(tempMech, target, mapx, mapy,
                             mech_range_to(tempMech, target)))
            defender = 1;
        } else if (mech_los_check(tempMech, target, mapx, mapy,
                                  FindRange(mech_position_real_x(tempMech),
                                            mech_position_real_y(tempMech),
                                            mech_position_real_z(tempMech), fx,
                                            fy, fz)))
          defender = 1;

        if (!attacker && !defender)
          continue;
        if (defender)
          snprintf(buff, sizeof(buff), "%s",
                   mech_to_mech_display_id(tempMech, target).text);
        if (attacker) {
          if (defender)
            mech_printf(tempMech, MECHSTARTED, "%s %s %s with a %s",
                        mech_to_mech_display_id(tempMech, mech).text,
                        IsHit ? "hits" : "misses", buff, weapname);
          else
            mech_printf(tempMech, MECHSTARTED, "%s fires a %s at something!",
                        mech_to_mech_display_id(tempMech, mech).text, weapname);
        } else
          mech_printf(tempMech, MECHSTARTED, "Something %s %s with a %s",
                      IsHit ? "hits" : "misses", buff, weapname);
      }
  } else {
    mapx = x;
    mapy = y;
    MapCoordToRealCoord(x, y, &fx, &fy);
    fz = ZSCALE * map_base_elevation(mech_map, x, y);
    snprintf(buff, sizeof(buff), "hex %d %d!", mapx, mapy);
    for (loop = 0; loop < mech_map->first_free; loop++)
      if (mech_map->mechsOnMap[loop] != mech_dbref(mech) &&
          mech_map->mechsOnMap[loop] != -1) {
        attacker = 0;
        defender = 0;
        tempMech = (Mech *)btech_context_find_object(
            mech_context(mech), mech_map->mechsOnMap[loop]);
        if (!tempMech)
          continue;
        if (mech_los_check(tempMech, mech, mech_position_x(mech),
                           mech_position_y(mech),
                           mech_range_to(tempMech, mech)))
          attacker = 1;
        if (target) {
          if (mech_los_check(tempMech, target, mapx, mapy,
                             mech_range_to(tempMech, target)))
            defender = 1;
        } else if (mech_los_check(tempMech, target, mapx, mapy,
                                  FindRange(mech_position_real_x(tempMech),
                                            mech_position_real_y(tempMech),
                                            mech_position_real_z(tempMech), fx,
                                            fy, fz)))
          defender = 1;
        if (!attacker && !defender)
          continue;
        if (attacker) {
          if (defender) /* att + def */
            mech_printf(tempMech, MECHSTARTED, "%s fires a %s at %s",
                        mech_to_mech_display_id(tempMech, mech).text, weapname,
                        buff);
          else /* att */
            mech_printf(tempMech, MECHSTARTED, "%s fires a %s at something!",
                        mech_to_mech_display_id(tempMech, mech).text, weapname);
        } else /* def */
          mech_printf(tempMech, MECHSTARTED, "Something fires a %s at %s",
                      weapname, buff);
      }
  }
}

void mech_notify(Mech *mech, int type, const char *buffer) {
  int i;

  if (mech_pilot_is_unconscious(mech))
    return;
  if (mech_is_blinded(mech))
    return;
  if (mech_dbref(mech) < 0)
    return;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  /* Let's do colorization too, just in case. */

  if (type == MECHPILOT) {
    if (mech_has_pilot(mech))
      notify(evaluation, mech_pilot_dbref(mech), buffer);
    else
      mech_notify(mech, MECHALL, buffer);
  } else if ((type == MECHALL && !mech_is_destroyed(mech)) ||
             (type == MECHSTARTED && mech_is_started(mech))) {
    notify_except(evaluation, mech_dbref(mech), NOTHING, mech_dbref(mech),
                  buffer);
    if (btech_context_combat_arcs_enabled(mech_context(mech)))
      for (i = 0; i < NUM_TURRETS; i++)
        if (mech_turret_dbref(mech, i) > 0)
          notify_except(evaluation, mech_turret_dbref(mech, i), NOTHING,
                        mech_turret_dbref(mech, i), buffer);
  }
}

void mech_printf(Mech *mech, int type, const char *format, ...) {
  char buffer[LBUF_SIZE];
  int i;
  va_list ap;

  if (mech_pilot_is_unconscious(mech))
    return;
  if (mech_is_blinded(mech))
    return;
  if (mech_dbref(mech) < 0)
    return;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  /* Let's do colorization too, just in case. */

  va_start(ap, format);
  vsnprintf(buffer, LBUF_SIZE, format, ap);
  va_end(ap);

  if (type == MECHPILOT) {
    if (mech_has_pilot(mech))
      notify(evaluation, mech_pilot_dbref(mech), buffer);
    else
      mech_notify(mech, MECHALL, buffer);
  } else if ((type == MECHALL && !mech_is_destroyed(mech)) ||
             (type == MECHSTARTED && mech_is_started(mech))) {
    notify_except(evaluation, mech_dbref(mech), NOTHING, mech_dbref(mech),
                  buffer);
    if (btech_context_combat_arcs_enabled(mech_context(mech)))
      for (i = 0; i < NUM_TURRETS; i++)
        if (mech_turret_dbref(mech, i) > 0)
          notify_except(evaluation, mech_turret_dbref(mech, i), NOTHING,
                        mech_turret_dbref(mech, i), buffer);
  }
}
