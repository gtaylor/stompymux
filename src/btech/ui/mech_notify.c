#include "btech/context.h"
#include "btech_text_builder.h"
#include "equipment_types.h"
#include "map.h"
#include "map_coordinates.h"
#include "map_terrain.h"
#include "map_units_api.h"
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
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

static int map_base_elevation(BattleMap *map, int x, int y) {
  int elevation = (unsigned char)map_elevation_get(map, x, y);
  char terrain = map_real_terrain_get(map, x, y);
  return terrain == WATER || terrain == ICE ? -elevation : elevation;
}

void mech_los_broadcast(Mech *mech, const char *message) {
  /* Sends msg to everyone except the mech */
  int i;
  Mech *temp_mech;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char buf[LBUF_SIZE];

  mech_sensor_visibility_refresh(mech);
  if (!mech_map)
    return;
  for (i = 0; i < battle_map_unit_count(mech_map); i++) {
    const DbRef CANDIDATE = battle_map_unit_dbref(mech_map, i);
    if (CANDIDATE != -1 && CANDIDATE != mech_dbref(mech)) {
      temp_mech =
          btech_context_get_mech(battle_map_context(mech_map), CANDIDATE);
      if (temp_mech) {
        if (mech_los_check(temp_mech, mech, mech_position_x(mech),
                           mech_position_y(mech),
                           mech_range_to(temp_mech, mech))) {
          (void)snprintf(buf, sizeof(buf), "%s%s%s",
                         mech_to_mech_display_id(temp_mech, mech).text,
                         *message != '\'' ? " " : "", message);
          mech_notify(temp_mech, MECHSTARTED, buf);
        }
      }
    }
  }
}

int mech_sees_hex_f(Mech *mech, BattleMap *map, float x, float y, int ix,
                    int iy) {
  int elevation = map_base_elevation(map, ix, iy);

  return mech_los_check(
      mech, nullptr, ix, iy,
      map_spatial_range(&(MapSpatialSegment){
          .start = {.x = mech_position_real_x(mech),
                    .y = mech_position_real_y(mech),
                    .z = mech_position_real_z(mech)},
          .end = {.x = x, .y = y, .z = ZSCALE * (float)elevation},
      }));
}

int mech_sees_hex(Mech *mech, BattleMap *map, int x, int y) {
  float fx;
  float fy;

  map_coord_to_real_coord(x, y, &fx, &fy);
  return mech_sees_hex_f(mech, map, fx, fy, x, y);
}

void hex_los_broadcast(BattleMap *mech_map, int x, int y, const char *message) {
  int i;
  Mech *temp_mech;
  float fx;
  float fy;

  /* substitution:
     $h = !alarming ('your hex', '%d,%d')
     $H = alarming ('YOUR HEX', '%d,%d (%.2f away)')
   */
  if (!mech_map)
    return;
  map_coord_to_real_coord(x, y, &fx, &fy);
  for (i = 0; i < battle_map_unit_count(mech_map); i++) {
    const DbRef CANDIDATE = battle_map_unit_dbref(mech_map, i);
    if (CANDIDATE != -1) {
      temp_mech =
          btech_context_get_mech(battle_map_context(mech_map), CANDIDATE);
      if (temp_mech) {
        if (mech_sees_hex_f(temp_mech, mech_map, fx, fy, x, y)) {
          char tbuf[LBUF_SIZE];
          BtechTextBuilder output;
          btech_text_builder_initialize(&output, tbuf, sizeof(tbuf));
          const size_t MESSAGE_LENGTH = strlen(message);

          for (size_t input = 0; input < MESSAGE_LENGTH;) {
            const char CHARACTER = *checked_string_suffix(message, input);
            if (CHARACTER == '$' && input + 1 == MESSAGE_LENGTH)
              break;
            if (CHARACTER == '$' && input + 1 < MESSAGE_LENGTH) {
              const char PLACEHOLDER =
                  *checked_string_suffix(message, input + 1);
              if (PLACEHOLDER == 'h' || PLACEHOLDER == 'H') {
                const bool CURRENT_HEX = (x == mech_position_x(temp_mech) &&
                                          y == mech_position_y(temp_mech)) != 0;
                if (PLACEHOLDER == 'h') {
                  if (CURRENT_HEX)
                    btech_text_builder_append(&output, "your hex");
                  else
                    btech_text_builder_append_format(&output, "%d,%d", x, y);
                } else if (CURRENT_HEX) {
                  btech_text_builder_append(&output,
                                            "[fg=red bold]YOUR HEX[reset]");
                } else {
                  btech_text_builder_append_format(
                      &output, "[fg=yellow bold]%d,%d[reset]", x, y);
                }
                input += 2;
                continue;
              }
            }
            btech_text_builder_append_character(&output, CHARACTER);
            input++;
          }
          mech_notify(temp_mech, MECHSTARTED, tbuf);
        }
      }
    }
  }
}

typedef struct MechLosMessageFormat {
  char *buffer;
  size_t buffer_size;
  const char *message;
  const char *target_name;
} MechLosMessageFormat;

static void format_mech_los_message(const MechLosMessageFormat *format) {
  char *buffer = format->buffer;
  const size_t BUFFER_SIZE = format->buffer_size;
  const char *message = format->message;
  const char *target_name = format->target_name;
  const char *placeholder = strstr(message, "%s");

  if (!placeholder) {
    (void)snprintf(buffer, BUFFER_SIZE, "%s", message);
    return;
  }
  const size_t PREFIX_LENGTH = (size_t)(placeholder - message);
  BtechTextBuilder output;
  btech_text_builder_initialize(&output, buffer, BUFFER_SIZE);
  btech_text_builder_append_count(&output, message, PREFIX_LENGTH);
  btech_text_builder_append(&output, target_name);
  btech_text_builder_append(&output,
                            checked_string_suffix(message, PREFIX_LENGTH + 2));
}

void mech_los_broadcast_unit(Mech *mech, Mech *target, const char *message) {
  /* Sends msg to everyone except the mech */
  int i;
  int a;
  int b;
  char *oddbuff;
  char *oddbuff2;
  Mech *temp_mech;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (!mech_map)
    return;
  oddbuff = alloc_lbuf("mech_los_broadcast_unit.message");
  oddbuff2 = alloc_lbuf("mech_los_broadcast_unit.broadcast");
  mech_sensor_visibility_refresh(mech);
  mech_sensor_visibility_refresh(target);
  for (i = 0; i < battle_map_unit_count(mech_map); i++) {
    const DbRef CANDIDATE = battle_map_unit_dbref(mech_map, i);
    if (CANDIDATE != -1 && CANDIDATE != mech_dbref(mech) &&
        CANDIDATE != mech_dbref(target)) {
      temp_mech = btech_context_get_mech(mech_context(mech), CANDIDATE);
      if (temp_mech) {
        a = mech_los_check(temp_mech, mech, mech_position_x(mech),
                           mech_position_y(mech),
                           mech_range_to(temp_mech, mech));
        b = mech_los_check(temp_mech, target, mech_position_x(target),
                           mech_position_y(target),
                           mech_range_to(temp_mech, target));
        if (a || b) {
          format_mech_los_message(&(MechLosMessageFormat){
              .buffer = oddbuff,
              .buffer_size = LBUF_SIZE,
              .message = message,
              .target_name = b ? mech_to_mech_display_id(temp_mech, target).text
                               : "someone"});
          BtechTextBuilder output;
          btech_text_builder_initialize(&output, oddbuff2, LBUF_SIZE);
          btech_text_builder_append(
              &output,
              a ? mech_to_mech_display_id(temp_mech, mech).text : "Someone");
          if (*oddbuff != '\'')
            btech_text_builder_append_character(&output, ' ');
          btech_text_builder_append(&output, oddbuff);
          mech_notify(temp_mech, MECHSTARTED, oddbuff2);
        }
      }
    }
  }
  free_buf(oddbuff2);
  free_buf(oddbuff);
}

void map_broadcast(BattleMap *map, char *message) {
  /* Sends msg to everyone except the mech */
  int i;
  Mech *temp_mech;

  for (i = 0; i < battle_map_unit_count(map); i++) {
    const DbRef CANDIDATE = battle_map_unit_dbref(map, i);
    if (CANDIDATE != -1) {
      temp_mech = btech_context_get_mech(battle_map_context(map), CANDIDATE);
      if (temp_mech)
        mech_notify(temp_mech, MECHSTARTED, message);
    }
  }
}

void mech_fire_broadcast(Mech *mech, Mech *target, int x, int y,
                         BattleMap *mech_map, const char *weapname,
                         int is_hit) {
  int loop;
  int attacker;
  int defender;
  float fx;
  float fy;
  float fz;
  int mapx;
  int mapy;
  Mech *temp_mech;
  char buff[SBUF_SIZE];

  /* Stat recording */
  if (target) { /* only if we have a mecha */
    if (is_hit)
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
    for (loop = 0; loop < battle_map_unit_count(mech_map); loop++) {
      const DbRef CANDIDATE = battle_map_unit_dbref(mech_map, loop);
      if (CANDIDATE != mech_dbref(mech) && CANDIDATE != -1 &&
          CANDIDATE != mech_dbref(target)) {
        attacker = 0;
        defender = 0;
        temp_mech =
            (Mech *)btech_context_find_object(mech_context(mech), CANDIDATE);
        if (!temp_mech)
          continue;
        if (mech_los_check(temp_mech, mech, mech_position_x(mech),
                           mech_position_y(mech),
                           mech_range_to(temp_mech, mech)))
          attacker = 1;
        if (target) {
          if (mech_los_check(temp_mech, target, mapx, mapy,
                             mech_range_to(temp_mech, target)))
            defender = 1;
        } else if (mech_los_check(
                       temp_mech, target, mapx, mapy,
                       map_spatial_range(&(MapSpatialSegment){
                           .start = {.x = mech_position_real_x(temp_mech),
                                     .y = mech_position_real_y(temp_mech),
                                     .z = mech_position_real_z(temp_mech)},
                           .end = {.x = fx, .y = fy, .z = fz},
                       }))) {
          defender = 1;
        }

        if (!attacker && !defender)
          continue;
        if (defender)
          (void)snprintf(buff, sizeof(buff), "%s",
                         mech_to_mech_display_id(temp_mech, target).text);
        if (attacker) {
          if (defender)
            mech_printf(temp_mech, MECHSTARTED, "%s %s %s with a %s",
                        mech_to_mech_display_id(temp_mech, mech).text,
                        is_hit ? "hits" : "misses", buff, weapname);
          else
            mech_printf(temp_mech, MECHSTARTED, "%s fires a %s at something!",
                        mech_to_mech_display_id(temp_mech, mech).text,
                        weapname);
        } else {
          mech_printf(temp_mech, MECHSTARTED, "Something %s %s with a %s",
                      is_hit ? "hits" : "misses", buff, weapname);
        }
      }
    }
  } else {
    mapx = x;
    mapy = y;
    map_coord_to_real_coord(x, y, &fx, &fy);
    int elevation = map_base_elevation(mech_map, x, y);
    fz = ZSCALE * (float)elevation;
    (void)snprintf(buff, sizeof(buff), "hex %d %d!", mapx, mapy);
    for (loop = 0; loop < battle_map_unit_count(mech_map); loop++) {
      const DbRef CANDIDATE = battle_map_unit_dbref(mech_map, loop);
      if (CANDIDATE != mech_dbref(mech) && CANDIDATE != -1) {
        attacker = 0;
        defender = 0;
        temp_mech =
            (Mech *)btech_context_find_object(mech_context(mech), CANDIDATE);
        if (!temp_mech)
          continue;
        if (mech_los_check(temp_mech, mech, mech_position_x(mech),
                           mech_position_y(mech),
                           mech_range_to(temp_mech, mech)))
          attacker = 1;
        if (target) {
          if (mech_los_check(temp_mech, target, mapx, mapy,
                             mech_range_to(temp_mech, target)))
            defender = 1;
        } else if (mech_los_check(
                       temp_mech, target, mapx, mapy,
                       map_spatial_range(&(MapSpatialSegment){
                           .start = {.x = mech_position_real_x(temp_mech),
                                     .y = mech_position_real_y(temp_mech),
                                     .z = mech_position_real_z(temp_mech)},
                           .end = {.x = fx, .y = fy, .z = fz},
                       }))) {
          defender = 1;
        }
        if (!attacker && !defender)
          continue;
        if (attacker) {
          if (defender) /* att + def */
            mech_printf(temp_mech, MECHSTARTED, "%s fires a %s at %s",
                        mech_to_mech_display_id(temp_mech, mech).text, weapname,
                        buff);
          else /* att */
            mech_printf(temp_mech, MECHSTARTED, "%s fires a %s at something!",
                        mech_to_mech_display_id(temp_mech, mech).text,
                        weapname);
        } else { /* def */
          mech_printf(temp_mech, MECHSTARTED, "Something fires a %s at %s",
                      weapname, buff);
        }
      }
    }
  }
}

void mech_notify(Mech *mech, MechNotifyAudience audience, const char *buffer) {
  int i;

  if (mech_pilot_is_unconscious(mech))
    return;
  if (mech_is_blinded(mech))
    return;
  if (mech_dbref(mech) < 0)
    return;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  /* Let's do colorization too, just in case. */

  if (audience == MECHPILOT) {
    if (mech_has_pilot(mech))
      mecha_notify(evaluation, mech_pilot_dbref(mech), buffer);
    else
      mech_notify(mech, MECHALL, buffer);
  } else if ((audience == MECHALL && !mech_is_destroyed(mech)) ||
             (audience == MECHSTARTED && mech_is_started(mech))) {
    mecha_notify_except(
        &(MechaNotificationExclusion){.evaluation = evaluation,
                                      .location = mech_dbref(mech),
                                      .actor = NOTHING,
                                      .exception = mech_dbref(mech),
                                      .message = buffer});
    if (btech_context_combat_arcs_enabled(mech_context(mech))) {
      for (i = 0; i < NUM_TURRETS; i++) {
        if (mech_turret_dbref(mech, i) > 0) {
          mecha_notify_except(&(MechaNotificationExclusion){
              .evaluation = evaluation,
              .location = mech_turret_dbref(mech, i),
              .actor = NOTHING,
              .exception = mech_turret_dbref(mech, i),
              .message = buffer});
        }
      }
    }
  }
}

void mech_printf(Mech *mech, MechNotifyAudience audience, const char *format,
                 ...) {
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
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(buffer, LBUF_SIZE, format, ap);
  va_end(ap);

  if (audience == MECHPILOT) {
    if (mech_has_pilot(mech))
      mecha_notify(evaluation, mech_pilot_dbref(mech), buffer);
    else
      mech_notify(mech, MECHALL, buffer);
  } else if ((audience == MECHALL && !mech_is_destroyed(mech)) ||
             (audience == MECHSTARTED && mech_is_started(mech))) {
    mecha_notify_except(
        &(MechaNotificationExclusion){.evaluation = evaluation,
                                      .location = mech_dbref(mech),
                                      .actor = NOTHING,
                                      .exception = mech_dbref(mech),
                                      .message = buffer});
    if (btech_context_combat_arcs_enabled(mech_context(mech))) {
      for (i = 0; i < NUM_TURRETS; i++) {
        if (mech_turret_dbref(mech, i) > 0) {
          mecha_notify_except(&(MechaNotificationExclusion){
              .evaluation = evaluation,
              .location = mech_turret_dbref(mech, i),
              .actor = NOTHING,
              .exception = mech_turret_dbref(mech, i),
              .message = buffer});
        }
      }
    }
  }
}
