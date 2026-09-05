#include "mux/server/platform.h"
#include "snapshot_store_objects_internal.h"

#include "autopilot.h"
#include "autopilot_argument_list_api.h"
#include "equipment_types.h"
#include "mux/support/checked_storage.h"
#include "mux/support/doubly_linked_list.h"
#include "special_object.h"
#include "sqlite_internal.h"
#include "turret.h"

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

static unsigned long stored_unsigned_long(const unsigned long *values,
                                          size_t count, int index) {
  if (index < 0)
    abort();
  const unsigned long *value =
      checked_storage_at_const(values, count, sizeof(*values), (size_t)index);
  return *value;
}

static int bind_unsigned_long(sqlite3_stmt *statement, int index,
                              unsigned long value) {
  if (value > INT64_MAX)
    return -1;
  return btech_special_bind_int(statement, index, (sqlite3_int64)value);
}

static void store_turret(BtechObjectStoreContext *context, DbRef object_id,
                         const Turret *turret) {
  if (btech_special_bind_int(context->turret, 1, object_id) < 0 ||
      btech_special_bind_int(context->turret, 2, turret->arcs) < 0 ||
      btech_special_bind_int(context->turret, 3, turret->parent) < 0 ||
      btech_special_bind_int(context->turret, 4, turret->gunner) < 0 ||
      btech_special_bind_int(context->turret, 5, turret->target) < 0 ||
      btech_special_bind_int(context->turret, 6, turret->targx) < 0 ||
      btech_special_bind_int(context->turret, 7, turret->targy) < 0 ||
      btech_special_bind_int(context->turret, 8, turret->targz) < 0 ||
      btech_special_bind_int(context->turret, 9, turret->lockmode) < 0 ||
      btech_special_write_step(context->fault, context->turret) < 0)
    context->result = -1;

  for (int index = 0; context->result == 0 && index < NUM_TICS; ++index) {
    if (btech_special_bind_int(context->turret_tic, 1, object_id) < 0 ||
        btech_special_bind_int(context->turret_tic, 2, index) < 0 ||
        bind_unsigned_long(context->turret_tic, 3,
                           stored_unsigned_long(turret->tic, NUM_TICS, index)) <
            0 ||
        btech_special_write_step(context->fault, context->turret_tic) < 0)
      context->result = -1;
  }
}

static void store_autopilot_commands(BtechObjectStoreContext *context,
                                     DbRef object_id, Autopilot *autopilot) {
  for (int index = 1; context->result == 0 && autopilot->commands != nullptr &&
                      index <= doubly_linked_list_size(autopilot->commands);
       ++index) {
    AutopilotCommand *command =
        doubly_linked_list_get_node(autopilot->commands, index);
    if (command == nullptr || command->argcount >= AUTOPILOT_MAX_ARGS ||
        btech_special_bind_int(context->autopilot_command, 1, object_id) < 0 ||
        btech_special_bind_int(context->autopilot_command, 2, index - 1) < 0 ||
        btech_special_bind_int(context->autopilot_command, 3,
                               command->command_enum) < 0 ||
        btech_special_bind_int(context->autopilot_command, 4,
                               command->argcount + 1) < 0 ||
        btech_special_write_step(context->fault, context->autopilot_command) <
            0) {
      context->result = -1;
      break;
    }
    for (int argument_index = 0;
         context->result == 0 && argument_index <= command->argcount;
         ++argument_index) {
      const char *argument = autopilot_argument_list_get(
          &command->arguments, (size_t)argument_index);
      if (argument == nullptr ||
          btech_special_bind_int(context->autopilot_command_arg, 1, object_id) <
              0 ||
          btech_special_bind_int(context->autopilot_command_arg, 2, index - 1) <
              0 ||
          btech_special_bind_int(context->autopilot_command_arg, 3,
                                 argument_index) < 0 ||
          sqlite3_bind_text(context->autopilot_command_arg, 4, argument, -1,
                            SQLITE_TRANSIENT) != SQLITE_OK ||
          btech_special_write_step(context->fault,
                                   context->autopilot_command_arg) < 0)
        context->result = -1;
    }
  }
}

static void store_autopilot_path(BtechObjectStoreContext *context,
                                 DbRef object_id, Autopilot *autopilot) {
  for (int index = 1;
       context->result == 0 && autopilot->astar_path != nullptr &&
       index <= doubly_linked_list_size(autopilot->astar_path);
       ++index) {
    const AutopilotPathNode *node =
        doubly_linked_list_get_node(autopilot->astar_path, index);
    if (node == nullptr ||
        btech_special_bind_int(context->autopilot_path, 1, object_id) < 0 ||
        btech_special_bind_int(context->autopilot_path, 2, index - 1) < 0 ||
        btech_special_bind_int(context->autopilot_path, 3, node->x) < 0 ||
        btech_special_bind_int(context->autopilot_path, 4, node->y) < 0 ||
        btech_special_bind_int(context->autopilot_path, 5, node->x_parent) <
            0 ||
        btech_special_bind_int(context->autopilot_path, 6, node->y_parent) <
            0 ||
        btech_special_bind_int(context->autopilot_path, 7, node->g_score) < 0 ||
        btech_special_bind_int(context->autopilot_path, 8, node->h_score) < 0 ||
        btech_special_bind_int(context->autopilot_path, 9, node->f_score) < 0 ||
        btech_special_bind_int(context->autopilot_path, 10, node->hexoffset) <
            0 ||
        btech_special_write_step(context->fault, context->autopilot_path) < 0) {
      context->result = -1;
      break;
    }
  }
}

static void store_autopilot(BtechObjectStoreContext *context, DbRef object_id,
                            Autopilot *autopilot) {
  sqlite3_int64 values[] = {
      autopilot->mymechnum,
      autopilot->mapindex,
      autopilot->speed,
      autopilot->ofsx,
      autopilot->ofsy,
      autopilot->verbose_level,
      autopilot->target,
      autopilot->target_score,
      autopilot->target_threshold,
      autopilot->target_update_tick,
      autopilot->chase_target,
      autopilot->chasetarg_update_tick,
      autopilot->follow_update_tick,
      autopilot->flags,
      autopilot->mech_max_range,
      autopilot->roam_type,
      autopilot->roam_update_tick,
      autopilot->roam_target_hex_x,
      autopilot->roam_target_hex_y,
      autopilot->roam_anchor_hex_x,
      autopilot->roam_anchor_hex_y,
      autopilot->roam_anchor_distance,
      autopilot->ahead_ok,
      autopilot->auto_cmode,
      autopilot->auto_cdist,
      autopilot->auto_goweight,
      autopilot->auto_fweight,
      autopilot->auto_nervous,
      autopilot->b_msc,
      autopilot->w_msc,
      autopilot->b_bsc,
      autopilot->w_bsc,
      autopilot->b_dan,
      autopilot->w_dan,
      autopilot->last_upd,
  };
  if (btech_special_bind_int(context->autopilot, 1, object_id) < 0)
    context->result = -1;
  for (size_t index = 0;
       context->result == 0 && index < sizeof(values) / sizeof(*values);
       ++index) {
    const sqlite3_int64 *value = checked_storage_at_const(
        values, sizeof(values) / sizeof(*values), sizeof(*values), index);
    if (btech_special_bind_int(context->autopilot, (int)index + 2, *value) < 0)
      context->result = -1;
  }
  if (context->result == 0 &&
      btech_special_write_step(context->fault, context->autopilot) < 0)
    context->result = -1;
  store_autopilot_commands(context, object_id, autopilot);
  store_autopilot_path(context, object_id, autopilot);
}

void btech_store_auxiliary_object(BtechObjectStoreContext *context,
                                  DbRef object_id, BtechSpecialObject *object) {
  if (object->type == GTYPE_TURRET)
    store_turret(context, object_id, (Turret *)object);
  else if (object->type == GTYPE_AUTO)
    store_autopilot(context, object_id, (Autopilot *)object);
}
