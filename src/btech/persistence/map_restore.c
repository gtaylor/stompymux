#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "sqlite_internal.h"

#include "checked_conversion.h"
#include "map_los_api.h"
#include "mux/support/checked_storage.h"
#include <limits.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

static unsigned short **restore_los_row(BattleMap *map, size_t count,
                                        int index) {
  if (index < 0)
    abort();
  return (unsigned short **)checked_storage_at(
      (void *)map->lo_sinfo, count, sizeof(*map->lo_sinfo), (size_t)index);
}

static DbRef *restore_unit_slot(BattleMap *map, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(map->mechs_on_map, (size_t)map->first_free,
                            sizeof(*map->mechs_on_map), (size_t)index);
}

static char *restore_unit_flag(BattleMap *map, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(map->mechflags, (size_t)map->first_free,
                            sizeof(*map->mechflags), (size_t)index);
}

static unsigned char *restore_map_cell(BattleMap *map, int x, int y) {
  if (x < 0 || y < 0)
    abort();
  unsigned char **row = (unsigned char **)checked_storage_at(
      (void *)map->map, (size_t)map->map_height, sizeof(*map->map), (size_t)y);
  return checked_storage_at(*row, (size_t)map->map_width, sizeof(**row),
                            (size_t)x);
}

static MapObject **restore_object_slot(BattleMap *map, int type) {
  if (type < 0)
    abort();
  return (MapObject **)checked_storage_at(
      (void *)map->map_object, NUM_MAPOBJTYPES, sizeof(*map->map_object),
      (size_t)type);
}

static unsigned char **restore_bits_row(unsigned char **bits, int height,
                                        int y) {
  if (y < 0)
    abort();
  return (unsigned char **)checked_storage_at((void *)bits, (size_t)height,
                                              sizeof(*bits), (size_t)y);
}

static int btech_special_resize_map(BattleMap *map, int width, int height) {
  if (width < 1 || width > MAPX || height < 1 || height > MAPY)
    return -1;
  unsigned char **grid = battle_map_grid_create(width, height);
  if (!grid)
    return -1;
  battle_map_grid_destroy(map->map, map->map_height);
  map->map = grid;
  map->map_width = clamp_int_to_short(width);
  map->map_height = clamp_int_to_short(height);
  return 0;
}

/* Allocate the dynamic occupancy and LOS matrices after first_free is known. */
static int btech_special_allocate_map_dynamic(BattleMap *map) {
  int index;
  size_t allocation_count;

  if (!map->first_free) {
    map->dynamic_size = 0;
    return 0;
  }
  allocation_count = (size_t)map->first_free;
  map->mechs_on_map = calloc(allocation_count, sizeof(*map->mechs_on_map));
  map->mechflags = calloc(allocation_count, sizeof(*map->mechflags));
  map->lo_sinfo =
      (unsigned short **)calloc(allocation_count, sizeof(*map->lo_sinfo));
  for (index = 0; map->mechs_on_map && map->mechflags && map->lo_sinfo &&
                  index < map->first_free;
       index++) {
    unsigned short **row = restore_los_row(map, allocation_count, index);
    *row = calloc(allocation_count, sizeof(**row));
  }
  if (map->mechs_on_map && map->mechflags && map->lo_sinfo &&
      index == map->first_free) {
    map->dynamic_size = map->first_free;
    return 0;
  }
  if (map->lo_sinfo)
    for (index = 0; index < map->first_free; index++)
      free(*restore_los_row(map, allocation_count, index));
  free((void *)map->lo_sinfo);
  free(map->mechflags);
  free(map->mechs_on_map);
  map->lo_sinfo = NULL;
  map->mechflags = NULL;
  map->mechs_on_map = NULL;
  map->dynamic_size = 0;
  return -1;
}

/* Restore map parent rows before any child row uses their dimensions. */
int btech_special_load_map_parents(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  BattleMap *map;
  char map_name[MAP_NAME_SIZE + 1];
  DbRef map_dbref;
  long long_value;
  int build_flag;
  int cf;
  int cf_max;
  int cloudbase;
  int first_free;
  int flags;
  int gravity;
  int height;
  int light;
  int max_visibility;
  int move_mod;
  int moves;
  int regen_factor;
  int reserved;
  int result;
  int sensor_flags;
  int step;
  int temperature;
  int visibility;
  int width;
  int wind_direction;
  int wind_speed;

  statement = NULL;
  result = SQLITE3_PREPARE_V2(
               sqlite,
               "SELECT dbref, map_name, width, height, temperature, gravity, "
               "cloudbase, visibility, max_visibility, light, wind_direction, "
               "wind_speed, reserved, flags, cf, cf_max, on_map, build_flag, "
               "first_free, moves, move_mod, sensor_flags, regen_factor "
               "FROM btech_maps ORDER BY dbref;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0) {
      result = -1;
      break;
    }
    map = btech_context_get_map(context, map_dbref);
    if (!map ||
        btech_special_column_text(statement, 1, map_name, sizeof(map_name)) <
            0 ||
        btech_special_column_int(statement, 2, &width) < 0 ||
        btech_special_column_int(statement, 3, &height) < 0 ||
        btech_special_column_int(statement, 4, &temperature) < 0 ||
        btech_special_column_int(statement, 5, &gravity) < 0 ||
        btech_special_column_int(statement, 6, &cloudbase) < 0 ||
        btech_special_column_int(statement, 7, &visibility) < 0 ||
        btech_special_column_int(statement, 8, &max_visibility) < 0 ||
        btech_special_column_int(statement, 9, &light) < 0 ||
        btech_special_column_int(statement, 10, &wind_direction) < 0 ||
        btech_special_column_int(statement, 11, &wind_speed) < 0 ||
        btech_special_column_int(statement, 12, &reserved) < 0 ||
        btech_special_column_int(statement, 13, &flags) < 0 ||
        btech_special_column_int(statement, 14, &cf) < 0 ||
        btech_special_column_int(statement, 15, &cf_max) < 0 ||
        btech_special_column_long(statement, 16, &long_value) < 0 ||
        btech_special_column_int(statement, 17, &build_flag) < 0 ||
        btech_special_column_int(statement, 18, &first_free) < 0 ||
        btech_special_column_int(statement, 19, &moves) < 0 ||
        btech_special_column_int(statement, 20, &move_mod) < 0 ||
        btech_special_column_int(statement, 21, &sensor_flags) < 0 ||
        btech_special_column_int(statement, 22, &regen_factor) < 0 ||
        temperature < CHAR_MIN || temperature > CHAR_MAX || gravity < 0 ||
        gravity > UCHAR_MAX || cloudbase < SHRT_MIN || cloudbase > SHRT_MAX ||
        visibility < CHAR_MIN || visibility > CHAR_MAX ||
        max_visibility < SHRT_MIN || max_visibility > SHRT_MAX ||
        light < CHAR_MIN || light > CHAR_MAX || wind_direction < SHRT_MIN ||
        wind_direction > SHRT_MAX || wind_speed < SHRT_MIN ||
        wind_speed > SHRT_MAX || reserved < CHAR_MIN || reserved > CHAR_MAX ||
        cf < SHRT_MIN || cf > SHRT_MAX || cf_max < SHRT_MIN ||
        cf_max > SHRT_MAX || build_flag < CHAR_MIN || build_flag > CHAR_MAX ||
        first_free < 0 || first_free > MAX_MECHS_PER_MAP || moves < SHRT_MIN ||
        moves > SHRT_MAX || move_mod < SHRT_MIN || move_mod > SHRT_MAX ||
        btech_special_resize_map(map, width, height) < 0) {
      result = -1;
      break;
    }
    memcpy(map->mapname, map_name, sizeof(map_name));
    map->temp = (char)temperature;
    map->grav = (unsigned char)gravity;
    map->cloudbase = (short)cloudbase;
    map->mapvis = (char)visibility;
    map->maxvis = (short)max_visibility;
    map->maplight = (char)light;
    map->winddir = (short)wind_direction;
    map->windspeed = (short)wind_speed;
    map->unused_char = (char)reserved;
    map->flags = flags;
    map->cf = (short)cf;
    map->cfmax = (short)cf_max;
    map->onmap = long_value;
    map->buildflag = (char)build_flag;
    map->first_free = (unsigned char)first_free;
    map->moves = (short)moves;
    map->movemod = (short)move_mod;
    map->sensorflags = sensor_flags;
    map->regen_factor = regen_factor;
    if (btech_special_allocate_map_dynamic(map) < 0) {
      result = -1;
      break;
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore every base-grid byte in order, rejecting incomplete or sparse maps.
 */
int btech_special_load_map_hexes(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  BattleMap *map;
  DbRef current_map;
  DbRef map_dbref;
  int expected_x;
  int expected_y;
  int result;
  int step;
  int value;
  int x;
  int y;

  statement = NULL;
  current_map = NOTHING;
  expected_x = 0;
  expected_y = 0;
  map = NULL;
  result =
      SQLITE3_PREPARE_V2(sqlite,
                         "SELECT map_dbref, x, y, value FROM btech_map_hexes "
                         "ORDER BY map_dbref, y, x;",
                         -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        map_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &x) < 0 ||
        btech_special_column_int(statement, 2, &y) < 0 ||
        btech_special_column_int(statement, 3, &value) < 0 || value < 0 ||
        value > UCHAR_MAX) {
      result = -1;
      break;
    }
    if (map_dbref != current_map) {
      if (map && expected_y != map->map_height) {
        result = -1;
        break;
      }
      map = btech_context_get_map(context, map_dbref);
      if (!map) {
        result = -1;
        break;
      }
      current_map = map_dbref;
      expected_x = 0;
      expected_y = 0;
    }
    if (x != expected_x || y != expected_y || y >= map->map_height) {
      result = -1;
      break;
    }
    *restore_map_cell(map, x, y) = (unsigned char)value;
    if (++expected_x == map->map_width) {
      expected_x = 0;
      expected_y++;
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && map && expected_y != map->map_height)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore occupancy rows into the dimensions allocated from each map parent. */
int btech_special_load_map_slots(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  BattleMap *map;
  DbRef current_map;
  DbRef map_dbref;
  long mech_dbref;
  int expected_slot;
  int flags;
  int result;
  int slot;
  int step;

  statement = NULL;
  current_map = NOTHING;
  expected_slot = 0;
  map = NULL;
  result = SQLITE3_PREPARE_V2(sqlite,
                              "SELECT map_dbref, slot, mech_dbref, mech_flags "
                              "FROM btech_map_slots ORDER BY map_dbref, slot;",
                              -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        map_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &slot) < 0 ||
        btech_special_column_long(statement, 2, &mech_dbref) < 0 ||
        btech_special_column_int(statement, 3, &flags) < 0 ||
        flags < CHAR_MIN || flags > CHAR_MAX) {
      result = -1;
      break;
    }
    if (map_dbref != current_map) {
      if (map && expected_slot != map->first_free) {
        result = -1;
        break;
      }
      map = btech_context_get_map(context, map_dbref);
      if (!map) {
        result = -1;
        break;
      }
      current_map = map_dbref;
      expected_slot = 0;
    }
    if (slot != expected_slot || slot >= map->first_free ||
        (mech_dbref != NOTHING &&
         !btech_context_get_mech(context, mech_dbref))) {
      result = -1;
      break;
    }
    *restore_unit_slot(map, slot) = mech_dbref;
    *restore_unit_flag(map, slot) = (char)flags;
    expected_slot++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && map && expected_slot != map->first_free)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore the complete square LOS matrix in its stable source/target order. */
int btech_special_load_map_los(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  BattleMap *map;
  DbRef current_map;
  DbRef map_dbref;
  int expected_source;
  int expected_target;
  int flags;
  int result;
  int source;
  int step;
  int target;

  statement = NULL;
  current_map = NOTHING;
  expected_source = 0;
  expected_target = 0;
  map = NULL;
  result =
      SQLITE3_PREPARE_V2(
          sqlite,
          "SELECT map_dbref, source_slot, target_slot, flags "
          "FROM btech_map_los ORDER BY map_dbref, source_slot, target_slot;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        map_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &source) < 0 ||
        btech_special_column_int(statement, 2, &target) < 0 ||
        btech_special_column_int(statement, 3, &flags) < 0 || flags < 0 ||
        flags > USHRT_MAX) {
      result = -1;
      break;
    }
    if (map_dbref != current_map) {
      if (map && expected_source != map->first_free) {
        result = -1;
        break;
      }
      map = btech_context_get_map(context, map_dbref);
      if (!map) {
        result = -1;
        break;
      }
      current_map = map_dbref;
      expected_source = 0;
      expected_target = 0;
    }
    if (source != expected_source || target != expected_target ||
        source >= map->first_free || target >= map->first_free) {
      result = -1;
      break;
    }
    battle_map_los_flags_set(map, source, target, (unsigned short)flags);
    if (++expected_target == map->first_free) {
      expected_target = 0;
      expected_source++;
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && map && expected_source != map->first_free)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Ensure maps with zero children, and maps omitted from a child query, are
 * checked too. */
int btech_special_validate_map_child_counts(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  int invalid_rows;
  int result;

  statement = NULL;
  result =
      SQLITE3_PREPARE_V2(
          sqlite,
          "SELECT count(*) FROM btech_maps AS maps WHERE "
          "(SELECT count(*) FROM btech_map_hexes AS hexes "
          " WHERE hexes.map_dbref = maps.dbref) != maps.width * maps.height "
          "OR (SELECT count(*) FROM btech_map_slots AS slots "
          " WHERE slots.map_dbref = maps.dbref) != maps.first_free "
          "OR (SELECT count(*) FROM btech_map_los AS los "
          " WHERE los.map_dbref = maps.dbref) != maps.first_free * "
          "maps.first_free;",
          -1, &statement, NULL) == SQLITE_OK &&
              sqlite3_step(statement) == SQLITE_ROW &&
              btech_special_column_int(statement, 0, &invalid_rows) == 0 &&
              invalid_rows == 0 && sqlite3_step(statement) == SQLITE_DONE
          ? 0
          : -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore ordered map objects through the normal map-object allocator. */
int btech_special_load_map_objects(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  BattleMap *map;
  DbRef current_map;
  DbRef map_dbref;
  long data_int;
  long object_dbref;
  MapObject source;
  MapObject *stored;
  MapObject **tail;
  int current_object_type;
  int data_char;
  int data_short;
  int expected_ordinal;
  int object_type;
  int ordinal;
  int result;
  int step;
  int x;
  int y;

  statement = NULL;
  current_map = NOTHING;
  current_object_type = -1;
  object_type = -1;
  expected_ordinal = 0;
  map = NULL;
  tail = NULL;
  result = SQLITE3_PREPARE_V2(
               sqlite,
               "SELECT map_dbref, object_type, ordinal, x, y, object_dbref, "
               "data_char, data_short, data_int FROM btech_map_objects "
               "ORDER BY map_dbref, object_type, ordinal;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        map_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &object_type) < 0 ||
        btech_special_column_int(statement, 2, &ordinal) < 0 ||
        btech_special_column_int(statement, 3, &x) < 0 ||
        btech_special_column_int(statement, 4, &y) < 0 ||
        btech_special_column_long(statement, 5, &object_dbref) < 0 ||
        btech_special_column_int(statement, 6, &data_char) < 0 ||
        btech_special_column_int(statement, 7, &data_short) < 0 ||
        btech_special_column_long(statement, 8, &data_int) < 0 ||
        object_type < 0 || object_type >= NUM_MAPOBJTYPES ||
        object_type == TYPE_BITS || ordinal < 0 || x < SHRT_MIN ||
        x > SHRT_MAX || y < SHRT_MIN || y > SHRT_MAX || data_short < SHRT_MIN ||
        data_short > SHRT_MAX) {
      result = -1;
      break;
    }
    if (map_dbref != current_map || object_type != current_object_type) {
      map = btech_context_get_map(context, map_dbref);
      if (!map) {
        result = -1;
        break;
      }
      current_map = map_dbref;
      current_object_type = object_type;
      expected_ordinal = 0;
      tail = restore_object_slot(map, object_type);
      source.type = (char)object_type;
    }
    if (ordinal != expected_ordinal || x < 0 || x >= map->map_width || y < 0 ||
        y >= map->map_height ||
        (object_dbref != NOTHING &&
         !is_good_obj(context->database, object_dbref))) {
      result = -1;
      break;
    }
    memset(&source, 0, sizeof(source));
    source.type = (char)object_type;
    source.x = (short)x;
    source.y = (short)y;
    source.obj = object_dbref;
    source.datac = data_char;
    source.datas = (short)data_short;
    source.payload.scalar = data_int;
    stored = add_mapobj(map, tail, &source, 0);
    if (!stored) {
      result = -1;
      break;
    }
    tail = &stored->next;
    expected_ordinal++;
    if (object_type == TYPE_BUILD)
      possibly_start_building_regen(context, source.obj);
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Rebuild the TYPE_BITS allocation without ever serializing its pointer. */
int btech_special_load_map_bits(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  BattleMap *map;
  DbRef current_map;
  DbRef map_dbref;
  MapObject source;
  unsigned char **bits;
  int bytes_per_row = 0;
  int current_y;
  int expected_byte;
  int result;
  int step;
  int value;
  int byte_index;
  int y;

  statement = NULL;
  current_map = NOTHING;
  current_y = -1;
  expected_byte = 0;
  map = NULL;
  bits = NULL;
  result = SQLITE3_PREPARE_V2(
               sqlite,
               "SELECT map_dbref, y, byte_index, value FROM btech_map_bits "
               "ORDER BY map_dbref, y, byte_index;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &map_dbref) < 0 ||
        map_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &y) < 0 ||
        btech_special_column_int(statement, 2, &byte_index) < 0 ||
        btech_special_column_int(statement, 3, &value) < 0 || value < 0 ||
        value > UCHAR_MAX) {
      result = -1;
      break;
    }
    if (map_dbref != current_map) {
      if (current_y >= 0 && expected_byte != bytes_per_row) {
        result = -1;
        break;
      }
      map = btech_context_get_map(context, map_dbref);
      if (!map || first_mapobj(map, TYPE_BITS)) {
        result = -1;
        break;
      }
      bits = (unsigned char **)calloc((size_t)map->map_height, sizeof(*bits));
      if (!bits) {
        result = -1;
        break;
      }
      memset(&source, 0, sizeof(source));
      source.type = TYPE_BITS;
      source.payload.bits = bits;
      if (!add_mapobj_to_type(map, TYPE_BITS, &source, 0)) {
        free((void *)bits);
        result = -1;
        break;
      }
      current_map = map_dbref;
      current_y = -1;
      expected_byte = 0;
    }
    bytes_per_row = (map->map_width / 4) + (map->map_width % 4 ? 1 : 0);
    if (y < 0 || y >= map->map_height || byte_index < 0 ||
        byte_index >= bytes_per_row || (current_y >= 0 && y < current_y) ||
        (y == current_y && byte_index != expected_byte)) {
      result = -1;
      break;
    }
    if (y != current_y) {
      if (current_y >= 0 && expected_byte != bytes_per_row) {
        result = -1;
        break;
      }
      unsigned char **row = restore_bits_row(bits, map->map_height, y);
      *row = calloc((size_t)bytes_per_row, sizeof(**row));
      if (!*row) {
        result = -1;
        break;
      }
      current_y = y;
      expected_byte = 0;
    }
    unsigned char *row = *restore_bits_row(bits, map->map_height, y);
    unsigned char *byte = checked_storage_at(row, (size_t)bytes_per_row,
                                             sizeof(*row), (size_t)byte_index);
    *byte = (unsigned char)value;
    expected_byte++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && current_y >= 0 && expected_byte != bytes_per_row)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Map each persisted repair type to the canonical repair completion callback.
 */
