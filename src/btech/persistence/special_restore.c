#include "autopilot.h"
#include "equipment_types.h"
#include "mechrep.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/utf8.h"
#include "registry_api.h"
#include "special_object.h"
#include "sqlite_internal.h"

#include "autopilot_argument_list_api.h"
#include "autopilot_commands_api.h"
#include "checked_conversion.h"
#include "turret.h"
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

static void *btech_special_object(BtechContext *context, DbRef object,
                                  BtechSpecialObjectType type) {
  if (!is_good_obj(context->database, object) ||
      btech_context_which_special(context, object) != (int)type)
    return NULL;
  return btech_context_find_object(context, object);
}

/* Restore repair-console target rows. */
int btech_special_load_mechrep(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  RepairFacility *mechrep;
  DbRef object;
  DbRef target;
  int result;
  int step;

  statement = NULL;
  result = btech_special_prepare_v2(
               sqlite,
               "SELECT dbref, current_target FROM btech_mechrep "
               "ORDER BY dbref;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &object) < 0) {
      result = -1;
      continue;
    }
    mechrep = btech_special_object(context, object, GTYPE_MECHREP);
    if (!mechrep || btech_special_column_dbref(context->database, statement, 1,
                                               &target) < 0)
      result = -1;
    else
      mechrep->current_target = target;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore a turret parent and every independent timing slot. */
int btech_special_load_turrets(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Turret *turret;
  DbRef object;
  DbRef parent;
  DbRef gunner;
  DbRef target;
  int arcs;
  int lock_mode;
  int result;
  int step;
  int target_x;
  int target_y;
  int target_z;

  statement = NULL;
  result =
      btech_special_prepare_v2(
          sqlite,
          "SELECT dbref, arcs, parent, gunner, target, target_x, target_y, "
          "target_z, lock_mode FROM btech_turrets ORDER BY dbref;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &object) < 0) {
      result = -1;
      break;
    }
    turret = btech_special_object(context, object, GTYPE_TURRET);
    if (!turret || btech_special_column_int(statement, 1, &arcs) < 0 ||
        btech_special_column_dbref(context->database, statement, 2, &parent) <
            0 ||
        btech_special_column_dbref(context->database, statement, 3, &gunner) <
            0 ||
        btech_special_column_dbref(context->database, statement, 4, &target) <
            0 ||
        btech_special_column_int(statement, 5, &target_x) < 0 ||
        btech_special_column_int(statement, 6, &target_y) < 0 ||
        btech_special_column_int(statement, 7, &target_z) < 0 ||
        btech_special_column_int(statement, 8, &lock_mode) < 0) {
      result = -1;
      break;
    }
    turret->arcs = arcs;
    turret->parent = parent;
    turret->gunner = gunner;
    turret->target = target;
    turret->targx = clamp_int_to_short(target_x);
    turret->targy = clamp_int_to_short(target_y);
    turret->targz = clamp_int_to_short(target_z);
    turret->lockmode = lock_mode;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore all NUM_TICS turret timing values in fixed index order. */
int btech_special_load_turret_tics(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Turret *turret;
  DbRef current_turret;
  DbRef turret_dbref;
  int expected_tic;
  int result;
  int step;
  int tic_index;
  int value;

  statement = NULL;
  current_turret = NOTHING;
  expected_tic = 0;
  turret = NULL;
  result = btech_special_prepare_v2(
               sqlite,
               "SELECT turret_dbref, tic_index, value FROM btech_turret_tics "
               "ORDER BY turret_dbref, tic_index;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &turret_dbref) < 0 ||
        turret_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &tic_index) < 0 ||
        btech_special_column_int(statement, 2, &value) < 0) {
      result = -1;
      break;
    }
    if (turret_dbref != current_turret) {
      if (turret && expected_tic != NUM_TICS) {
        result = -1;
        break;
      }
      turret = btech_special_object(context, turret_dbref, GTYPE_TURRET);
      if (!turret) {
        result = -1;
        break;
      }
      current_turret = turret_dbref;
      expected_tic = 0;
    }
    if (tic_index != expected_tic || tic_index >= NUM_TICS) {
      result = -1;
      break;
    }
    if (value < 0) {
      result = -1;
      break;
    }
    unsigned long *tic = checked_storage_at(
        turret->tic, NUM_TICS, sizeof(*turret->tic), (size_t)tic_index);
    *tic = (unsigned long)value;
    expected_tic++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && turret && expected_tic != NUM_TICS)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore AUTOPILOT scalar state; command and path lists are loaded separately.
 */
int btech_special_load_autopilots(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Autopilot *autopilot;
  DbRef object;
  DbRef map_dbref;
  DbRef mech_dbref;
  DbRef target;
  int result;
  int step;

  statement = NULL;
  result = btech_special_prepare_v2(
               sqlite, "SELECT * FROM btech_autopilots ORDER BY dbref;", -1,
               &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &object) < 0) {
      result = -1;
      break;
    }
    autopilot = btech_special_object(context, object, GTYPE_AUTO);
    if (!autopilot ||
        btech_special_column_dbref(context->database, statement, 1,
                                   &mech_dbref) < 0 ||
        btech_special_column_dbref(context->database, statement, 2,
                                   &map_dbref) < 0 ||
        btech_special_column_ushort(statement, 3, &autopilot->speed) < 0 ||
        btech_special_column_int(statement, 4, &autopilot->ofsx) < 0 ||
        btech_special_column_int(statement, 5, &autopilot->ofsy) < 0 ||
        btech_special_column_uchar(statement, 6, &autopilot->verbose_level) <
            0 ||
        btech_special_column_dbref(context->database, statement, 7, &target) <
            0 ||
        btech_special_column_int(statement, 8, &autopilot->target_score) < 0 ||
        btech_special_column_int(statement, 9, &autopilot->target_threshold) <
            0 ||
        btech_special_column_int(statement, 10,
                                 &autopilot->target_update_tick) < 0 ||
        btech_special_column_dbref(context->database, statement, 11,
                                   &autopilot->chase_target) < 0 ||
        btech_special_column_int(statement, 12,
                                 &autopilot->chasetarg_update_tick) < 0 ||
        btech_special_column_int(statement, 13,
                                 &autopilot->follow_update_tick) < 0 ||
        btech_special_column_ushort(statement, 14, &autopilot->flags) < 0 ||
        btech_special_column_int(statement, 15, &autopilot->mech_max_range) <
            0 ||
        btech_special_column_uchar(statement, 16, &autopilot->roam_type) < 0 ||
        btech_special_column_int(statement, 17, &autopilot->roam_update_tick) <
            0 ||
        btech_special_column_short(statement, 18,
                                   &autopilot->roam_target_hex_x) < 0 ||
        btech_special_column_short(statement, 19,
                                   &autopilot->roam_target_hex_y) < 0 ||
        btech_special_column_int(statement, 20, &autopilot->roam_anchor_hex_x) <
            0 ||
        btech_special_column_int(statement, 21, &autopilot->roam_anchor_hex_y) <
            0 ||
        btech_special_column_int(statement, 22,
                                 &autopilot->roam_anchor_distance) < 0 ||
        btech_special_column_int(statement, 23, &autopilot->ahead_ok) < 0 ||
        btech_special_column_int(statement, 24, &autopilot->auto_cmode) < 0 ||
        btech_special_column_int(statement, 25, &autopilot->auto_cdist) < 0 ||
        btech_special_column_int(statement, 26, &autopilot->auto_goweight) <
            0 ||
        btech_special_column_int(statement, 27, &autopilot->auto_fweight) < 0 ||
        btech_special_column_int(statement, 28, &autopilot->auto_nervous) < 0 ||
        btech_special_column_int(statement, 29, &autopilot->b_msc) < 0 ||
        btech_special_column_int(statement, 30, &autopilot->w_msc) < 0 ||
        btech_special_column_int(statement, 31, &autopilot->b_bsc) < 0 ||
        btech_special_column_int(statement, 32, &autopilot->w_bsc) < 0 ||
        btech_special_column_int(statement, 33, &autopilot->b_dan) < 0 ||
        btech_special_column_int(statement, 34, &autopilot->w_dan) < 0 ||
        btech_special_column_long(statement, 35, &autopilot->last_upd) < 0 ||
        (mech_dbref != 0 && !btech_context_get_mech(context, mech_dbref)) ||
        (map_dbref != NOTHING && map_dbref != 0 &&
         !btech_context_get_map(context, map_dbref))) {
      result = -1;
      break;
    }
    autopilot->mymechnum = mech_dbref;
    autopilot->mapindex = map_dbref;
    autopilot->target = target;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Resolve the live command callback from the durable command enum. */
static const AutopilotCommandDefinition *
btech_special_autopilot_command(int command_enum) {
  for (int index = 0; index < AUTO_NUM_COMMANDS; index++) {
    const AutopilotCommandDefinition *definition =
        autopilot_command_definition_at(index);
    if (definition->name && definition->command_enum == command_enum)
      return definition;
  }
  return NULL;
}

/* Load one command's ordered text arguments and derive its callback locally. */
typedef struct AutopilotCommandRestoreRequest {
  sqlite3 *sqlite;
  Autopilot *autopilot;
  DbRef autopilot_dbref;
  int position;
  int command;
  int argument_count;
} AutopilotCommandRestoreRequest;

static int btech_special_load_autopilot_command_args(
    const AutopilotCommandRestoreRequest *request) {
  sqlite3 *sqlite = request->sqlite;
  Autopilot *autopilot = request->autopilot;
  const DbRef AUTOPILOT_DBREF = request->autopilot_dbref;
  const int POSITION = request->position;
  const int COMMAND_ENUM = request->command;
  const int ARGUMENT_COUNT = request->argument_count;
  sqlite3_stmt *statement;
  const AutopilotCommandDefinition *definition;
  AutopilotCommand *command;
  DoublyLinkedListNode *list_node;
  const unsigned char *value;
  int argument_index;
  int length;
  int result;
  int step;

  definition = btech_special_autopilot_command(COMMAND_ENUM);
  if (!definition || ARGUMENT_COUNT != definition->argcount + 1 ||
      ARGUMENT_COUNT < 1 || ARGUMENT_COUNT > AUTOPILOT_MAX_ARGS)
    return -1;
  command = calloc(1, sizeof(*command));
  if (!command)
    return -1;
  autopilot_argument_list_initialize(&command->arguments, AUTOPILOT_MAX_ARGS);
  statement = NULL;
  result = btech_special_prepare_v2(
               sqlite,
               "SELECT argument_index, value FROM btech_autopilot_command_args "
               "WHERE autopilot_dbref = ? AND command_position = ? "
               "ORDER BY argument_index;",
               -1, &statement, NULL) == SQLITE_OK &&
                   btech_special_bind_int(statement, 1, AUTOPILOT_DBREF) == 0 &&
                   btech_special_bind_int(statement, 2, POSITION) == 0
               ? 0
               : -1;
  argument_index = 0;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    int stored_index;

    value = sqlite3_column_text(statement, 1);
    length = sqlite3_column_bytes(statement, 1);
    if (btech_special_column_int(statement, 0, &stored_index) < 0 ||
        stored_index != argument_index || !value || length < 0 ||
        length >= LBUF_SIZE || (int)strlen((const char *)value) != length ||
        !utf8_validate((const char *)value, (size_t)length)) {
      result = -1;
      break;
    }
    char *argument = strndup((const char *)value, (size_t)length);
    if (!argument) {
      result = -1;
      break;
    }
    autopilot_argument_list_set(&command->arguments, (size_t)argument_index,
                                argument);
    argument_index++;
  }
  if (result == 0 && (step != SQLITE_DONE || argument_index != ARGUMENT_COUNT))
    result = -1;
  sqlite3_finalize(statement);
  if (result < 0) {
    auto_destroy_command_node(command);
    return -1;
  }
  command->argcount = (unsigned char)(ARGUMENT_COUNT - 1);
  command->command_enum = COMMAND_ENUM;
  command->ai_command_function = definition->ai_command_function;
  list_node = doubly_linked_list_create_node(command);
  if (!list_node) {
    auto_destroy_command_node(command);
    return -1;
  }
  doubly_linked_list_insert_end(autopilot->commands, list_node);
  return 0;
}

/* Restore the command queue in stable execution order. */
int btech_special_load_autopilot_commands(sqlite3 *sqlite,
                                          BtechContext *context) {
  sqlite3_stmt *statement;
  Autopilot *autopilot;
  DbRef current_autopilot;
  DbRef autopilot_dbref;
  int argument_count;
  int command_enum;
  int expected_position;
  int position;
  int result;
  int step;

  statement = NULL;
  current_autopilot = NOTHING;
  expected_position = 0;
  autopilot = NULL;
  result =
      btech_special_prepare_v2(
          sqlite,
          "SELECT autopilot_dbref, position, command_enum, arg_count "
          "FROM btech_autopilot_commands ORDER BY autopilot_dbref, position;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &autopilot_dbref) < 0 ||
        autopilot_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &position) < 0 ||
        btech_special_column_int(statement, 2, &command_enum) < 0 ||
        btech_special_column_int(statement, 3, &argument_count) < 0) {
      result = -1;
      break;
    }
    if (autopilot_dbref != current_autopilot) {
      autopilot = btech_special_object(context, autopilot_dbref, GTYPE_AUTO);
      if (!autopilot || !autopilot->commands ||
          doubly_linked_list_size(autopilot->commands)) {
        result = -1;
        break;
      }
      current_autopilot = autopilot_dbref;
      expected_position = 0;
    }
    if (position != expected_position ||
        btech_special_load_autopilot_command_args(
            &(AutopilotCommandRestoreRequest){
                .sqlite = sqlite,
                .autopilot = autopilot,
                .autopilot_dbref = autopilot_dbref,
                .position = position,
                .command = command_enum,
                .argument_count = argument_count,
            }) < 0) {
      result = -1;
      break;
    }
    expected_position++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore an A* path as ordered nodes, never as saved list pointers. */
int btech_special_load_autopilot_path(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Autopilot *autopilot;
  AutopilotPathNode *path_node;
  DbRef current_autopilot;
  DbRef autopilot_dbref;
  DoublyLinkedListNode *list_node;
  long f_score;
  long g_score;
  long h_score;
  long hex_offset;
  int expected_position;
  int parent_x;
  int parent_y;
  int position;
  int result;
  int step;
  int x;
  int y;

  statement = NULL;
  current_autopilot = NOTHING;
  expected_position = 0;
  autopilot = NULL;
  result =
      btech_special_prepare_v2(
          sqlite,
          "SELECT autopilot_dbref, position, x, y, parent_x, parent_y, "
          "g_score, h_score, f_score, hex_offset FROM btech_autopilot_path "
          "ORDER BY autopilot_dbref, position;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &autopilot_dbref) < 0 ||
        autopilot_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &position) < 0 ||
        btech_special_column_int(statement, 2, &x) < 0 ||
        btech_special_column_int(statement, 3, &y) < 0 ||
        btech_special_column_int(statement, 4, &parent_x) < 0 ||
        btech_special_column_int(statement, 5, &parent_y) < 0 ||
        btech_special_column_long(statement, 6, &g_score) < 0 ||
        btech_special_column_long(statement, 7, &h_score) < 0 ||
        btech_special_column_long(statement, 8, &f_score) < 0 ||
        btech_special_column_long(statement, 9, &hex_offset) < 0 ||
        x < SHRT_MIN || x > SHRT_MAX || y < SHRT_MIN || y > SHRT_MAX ||
        parent_x < SHRT_MIN || parent_x > SHRT_MAX || parent_y < SHRT_MIN ||
        parent_y > SHRT_MAX) {
      result = -1;
      break;
    }
    if (autopilot_dbref != current_autopilot) {
      autopilot = btech_special_object(context, autopilot_dbref, GTYPE_AUTO);
      if (!autopilot || autopilot->astar_path) {
        result = -1;
        break;
      }
      autopilot->astar_path = doubly_linked_list_create_list();
      if (!autopilot->astar_path) {
        result = -1;
        break;
      }
      current_autopilot = autopilot_dbref;
      expected_position = 0;
    }
    if (position != expected_position) {
      result = -1;
      break;
    }
    path_node = calloc(1, sizeof(*path_node));
    if (!path_node) {
      result = -1;
      break;
    }
    path_node->x = (short)x;
    path_node->y = (short)y;
    path_node->x_parent = (short)parent_x;
    path_node->y_parent = (short)parent_y;
    path_node->g_score = clamp_intptr_to_int((intptr_t)g_score);
    path_node->h_score = clamp_intptr_to_int((intptr_t)h_score);
    path_node->f_score = clamp_intptr_to_int((intptr_t)f_score);
    path_node->hexoffset = clamp_intptr_to_int((intptr_t)hex_offset);
    list_node = doubly_linked_list_create_node(path_node);
    if (!list_node) {
      free(path_node);
      result = -1;
      break;
    }
    doubly_linked_list_insert_end(autopilot->astar_path, list_node);
    expected_position++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Store the bounded explicit state of repair consoles and dropship turrets. */
