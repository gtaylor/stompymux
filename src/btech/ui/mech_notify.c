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
  Mech *tempMech;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char buf[LBUF_SIZE];

  mech_sensor_visibility_refresh(mech);
  if (!mech_map)
    return;
  for (i = 0; i < battle_map_unit_count(mech_map); i++) {
    const DbRef candidate = battle_map_unit_dbref(mech_map, i);
    if (candidate != -1 && candidate != mech_dbref(mech)) {
      tempMech =
          btech_context_get_mech(battle_map_context(mech_map), candidate);
      if (tempMech)
        if (mech_los_check(tempMech, mech, mech_position_x(mech),
                           mech_position_y(mech),
                           mech_range_to(tempMech, mech))) {
          (void)snprintf(buf, sizeof(buf), "%s%s%s",
                         mech_to_mech_display_id(tempMech, mech).text,
                         *message != '\'' ? " " : "", message);
          mech_notify(tempMech, MECHSTARTED, buf);
        }
    }
  }
}

int MechSeesHexF(Mech *mech, BattleMap *map, float x, float y, int ix, int iy) {
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

int MechSeesHex(Mech *mech, BattleMap *map, int x, int y) {
  float fx, fy;

  MapCoordToRealCoord(x, y, &fx, &fy);
  return MechSeesHexF(mech, map, fx, fy, x, y);
}

void HexLOSBroadcast(BattleMap *mech_map, int x, int y, const char *message) {
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
  for (i = 0; i < battle_map_unit_count(mech_map); i++) {
    const DbRef candidate = battle_map_unit_dbref(mech_map, i);
    if (candidate != -1) {
      tempMech =
          btech_context_get_mech(battle_map_context(mech_map), candidate);
      if (tempMech)
        if (MechSeesHexF(tempMech, mech_map, fx, fy, x, y)) {
          char tbuf[LBUF_SIZE];
          BtechTextBuilder output;
          btech_text_builder_initialize(&output, tbuf, sizeof(tbuf));
          const size_t message_length = strlen(message);

          for (size_t input = 0; input < message_length;) {
            const char character = *checked_string_suffix(message, input);
            if (character == '$' && input + 1 == message_length)
              break;
            if (character == '$' && input + 1 < message_length) {
              const char placeholder =
                  *checked_string_suffix(message, input + 1);
              if (placeholder == 'h' || placeholder == 'H') {
                const bool current_hex = x == mech_position_x(tempMech) &&
                                         y == mech_position_y(tempMech);
                if (placeholder == 'h') {
                  if (current_hex)
                    btech_text_builder_append(&output, "your hex");
                  else
                    btech_text_builder_append_format(&output, "%d,%d", x, y);
                } else if (current_hex) {
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
            btech_text_builder_append_character(&output, character);
            input++;
          }
          mech_notify(tempMech, MECHSTARTED, tbuf);
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
  const size_t buffer_size = format->buffer_size;
  const char *message = format->message;
  const char *target_name = format->target_name;
  const char *placeholder = strstr(message, "%s");

  if (!placeholder) {
    (void)snprintf(buffer, buffer_size, "%s", message);
    return;
  }
  const size_t prefix_length = (size_t)(placeholder - message);
  BtechTextBuilder output;
  btech_text_builder_initialize(&output, buffer, buffer_size);
  btech_text_builder_append_count(&output, message, prefix_length);
  btech_text_builder_append(&output, target_name);
  btech_text_builder_append(&output,
                            checked_string_suffix(message, prefix_length + 2));
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
  for (i = 0; i < battle_map_unit_count(mech_map); i++) {
    const DbRef candidate = battle_map_unit_dbref(mech_map, i);
    if (candidate != -1 && candidate != mech_dbref(mech) &&
        candidate != mech_dbref(target)) {
      tempMech = btech_context_get_mech(mech_context(mech), candidate);
      if (tempMech) {
        a = mech_los_check(tempMech, mech, mech_position_x(mech),
                           mech_position_y(mech),
                           mech_range_to(tempMech, mech));
        b = mech_los_check(tempMech, target, mech_position_x(target),
                           mech_position_y(target),
                           mech_range_to(tempMech, target));
        if (a || b) {
          format_mech_los_message(&(MechLosMessageFormat){
              .buffer = oddbuff,
              .buffer_size = sizeof(oddbuff),
              .message = message,
              .target_name = b ? mech_to_mech_display_id(tempMech, target).text
                               : "someone"});
          BtechTextBuilder output;
          btech_text_builder_initialize(&output, oddbuff2, sizeof(oddbuff2));
          btech_text_builder_append(
              &output,
              a ? mech_to_mech_display_id(tempMech, mech).text : "Someone");
          if (*oddbuff != '\'')
            btech_text_builder_append_character(&output, ' ');
          btech_text_builder_append(&output, oddbuff);
          mech_notify(tempMech, MECHSTARTED, oddbuff2);
        }
      }
    }
  }
}

void MapBroadcast(BattleMap *map, char *message) {
  /* Sends msg to everyone except the mech */
  int i;
  Mech *tempMech;

  for (i = 0; i < battle_map_unit_count(map); i++) {
    const DbRef candidate = battle_map_unit_dbref(map, i);
    if (candidate != -1) {
      tempMech = btech_context_get_mech(battle_map_context(map), candidate);
      if (tempMech)
        mech_notify(tempMech, MECHSTARTED, message);
    }
  }
}

void MechFireBroadcast(Mech *mech, Mech *target, int x, int y,
                       BattleMap *mech_map, const char *weapname, int IsHit) {
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
    for (loop = 0; loop < battle_map_unit_count(mech_map); loop++) {
      const DbRef candidate = battle_map_unit_dbref(mech_map, loop);
      if (candidate != mech_dbref(mech) && candidate != -1 &&
          candidate != mech_dbref(target)) {
        attacker = 0;
        defender = 0;
        tempMech =
            (Mech *)btech_context_find_object(mech_context(mech), candidate);
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
        } else if (mech_los_check(
                       tempMech, target, mapx, mapy,
                       map_spatial_range(&(MapSpatialSegment){
                           .start = {.x = mech_position_real_x(tempMech),
                                     .y = mech_position_real_y(tempMech),
                                     .z = mech_position_real_z(tempMech)},
                           .end = {.x = fx, .y = fy, .z = fz},
                       })))
          defender = 1;

        if (!attacker && !defender)
          continue;
        if (defender)
          (void)snprintf(buff, sizeof(buff), "%s",
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
    }
  } else {
    mapx = x;
    mapy = y;
    MapCoordToRealCoord(x, y, &fx, &fy);
    int elevation = map_base_elevation(mech_map, x, y);
    fz = ZSCALE * (float)elevation;
    (void)snprintf(buff, sizeof(buff), "hex %d %d!", mapx, mapy);
    for (loop = 0; loop < battle_map_unit_count(mech_map); loop++) {
      const DbRef candidate = battle_map_unit_dbref(mech_map, loop);
      if (candidate != mech_dbref(mech) && candidate != -1) {
        attacker = 0;
        defender = 0;
        tempMech =
            (Mech *)btech_context_find_object(mech_context(mech), candidate);
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
        } else if (mech_los_check(
                       tempMech, target, mapx, mapy,
                       map_spatial_range(&(MapSpatialSegment){
                           .start = {.x = mech_position_real_x(tempMech),
                                     .y = mech_position_real_y(tempMech),
                                     .z = mech_position_real_z(tempMech)},
                           .end = {.x = fx, .y = fy, .z = fz},
                       })))
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
    if (btech_context_combat_arcs_enabled(mech_context(mech)))
      for (i = 0; i < NUM_TURRETS; i++)
        if (mech_turret_dbref(mech, i) > 0)
          mecha_notify_except(&(MechaNotificationExclusion){
              .evaluation = evaluation,
              .location = mech_turret_dbref(mech, i),
              .actor = NOTHING,
              .exception = mech_turret_dbref(mech, i),
              .message = buffer});
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
    if (btech_context_combat_arcs_enabled(mech_context(mech)))
      for (i = 0; i < NUM_TURRETS; i++)
        if (mech_turret_dbref(mech, i) > 0)
          mecha_notify_except(&(MechaNotificationExclusion){
              .evaluation = evaluation,
              .location = mech_turret_dbref(mech, i),
              .actor = NOTHING,
              .exception = mech_turret_dbref(mech, i),
              .message = buffer});
  }
}
