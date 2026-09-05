#include "ai_api.h"
#include "equipment_types.h"
#include "mech_persistence.h"
#include "mech_stagger.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/red_black_tree.h"
#include "section_types.h"
#include "snapshot_store_objects_internal.h"
#include "special_object.h"
#include "sqlite_internal.h"

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <time.h>

#include "mux/support/checked_storage.h"

static const struct MechSection *
stored_section(const MechPersistenceSnapshot *snapshot, int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(snapshot->definition.sections, NUM_SECTIONS,
                                  sizeof(*snapshot->definition.sections),
                                  (size_t)index);
}

static const struct CriticalSlot *
stored_critical(const struct MechSection *section, int slot) {
  if (slot < 0)
    abort();
  return checked_storage_at_const(section->criticals, NUM_CRITICALS,
                                  sizeof(*section->criticals), (size_t)slot);
}

static DbRef stored_dbref(const DbRef *values, size_t count, int index) {
  if (index < 0)
    abort();
  const DbRef *value =
      checked_storage_at_const(values, count, sizeof(*values), (size_t)index);
  return *value;
}

static int stored_int(const int *values, size_t count, int index) {
  if (index < 0)
    abort();
  const int *value =
      checked_storage_at_const(values, count, sizeof(*values), (size_t)index);
  return *value;
}

static int stored_char(const char *values, size_t count, int index) {
  if (index < 0)
    abort();
  const char *value =
      checked_storage_at_const(values, count, sizeof(*values), (size_t)index);
  return *value;
}

static unsigned long stored_unsigned_long(const unsigned long *values,
                                          size_t count, int index) {
  if (index < 0)
    abort();
  const unsigned long *value =
      checked_storage_at_const(values, count, sizeof(*values), (size_t)index);
  return *value;
}

static unsigned long stored_tic(const MechPersistenceSnapshot *snapshot,
                                int tic, int word) {
  if (tic < 0 || word < 0)
    abort();
  const unsigned long (*row)[TICLONGS] = checked_storage_at_const(
      snapshot->tics, NUM_TICS, sizeof(*snapshot->tics), (size_t)tic);
  return stored_unsigned_long(*row, TICLONGS, word);
}

static const char *stored_channel_title(const MechPersistenceSnapshot *snapshot,
                                        int index) {
  if (index < 0)
    abort();
  const char (*title)[CHTITLELEN + 1] = checked_storage_at_const(
      snapshot->channel_titles, FREQS, sizeof(*snapshot->channel_titles),
      (size_t)index);
  return *title;
}

static int bind_runtime_int(sqlite3_stmt *statement, int *index,
                            sqlite3_int64 value) {
  return btech_special_bind_int(statement, (*index)++, value);
}

static int bind_runtime_time(sqlite3_stmt *statement, int *index,
                             time_t value) {
  return btech_special_bind_int(statement, (*index)++, (sqlite3_int64)value);
}

static int bind_float(sqlite3_stmt *statement, int index, float value) {
  return sqlite3_bind_double(statement, index, (double)value);
}

static int bind_unsigned_long(sqlite3_stmt *statement, int index,
                              unsigned long value) {
  if (value > INT64_MAX)
    return -1;
  return btech_special_bind_int(statement, index, (sqlite3_int64)value);
}

static int bind_runtime_real(sqlite3_stmt *statement, int *index, float value) {
  return btech_special_bind_real(statement, (*index)++, (double)value);
}

void btech_finalize_object_statements(BtechObjectStoreContext *context) {
  sqlite3_finalize(context->turret);
  sqlite3_finalize(context->turret_tic);
  sqlite3_finalize(context->autopilot);
  sqlite3_finalize(context->mech);
  sqlite3_finalize(context->section);
  sqlite3_finalize(context->critical);
  sqlite3_finalize(context->position);
  sqlite3_finalize(context->bay);
  sqlite3_finalize(context->mech_turret);
  sqlite3_finalize(context->c3);
  sqlite3_finalize(context->c3node);
  sqlite3_finalize(context->tic);
  sqlite3_finalize(context->frequency);
  sqlite3_finalize(context->runtime);
  sqlite3_finalize(context->unit_aux);
  sqlite3_finalize(context->stagger_damage);
  sqlite3_finalize(context->autopilot_command);
  sqlite3_finalize(context->autopilot_command_arg);
  sqlite3_finalize(context->autopilot_path);
}

/* Replace the default map grid with a validated grid owned by this MAP. */

bool btech_store_simple_object(const RedBlackTreeVisitCall *call) {
  const DbRef KEY = (DbRef) * (const intptr_t *)call->key;
  void *data = call->data;
  int depth [[maybe_unused]] = call->depth;
  void *argument = call->context;
  BtechObjectStoreContext *context = argument;
  BtechSpecialObject *xcode = data;
  Mech *mech;
  MechPersistenceSnapshot snapshot;
  int index;
  int slot;
  int runtime_index;
  MechStaggerDamageSnapshot damage;

  if (context->result < 0)
    return false;
  if (!is_good_obj(context->database, KEY) ||
      !is_thing(context->database, KEY) || is_going(context->database, KEY))
    return true;
  if (xcode->type == GTYPE_MECH) {
    mech = (Mech *)xcode;
    mech_persistence_snapshot_export(mech, &snapshot);
    if (btech_special_bind_int(context->mech, 1, KEY) < 0 ||
        btech_special_bind_int(context->mech, 2, snapshot.id[0]) < 0 ||
        btech_special_bind_int(context->mech, 3, snapshot.id[1]) < 0 ||
        btech_special_bind_int(context->mech, 4, snapshot.brief) < 0 ||
        btech_special_bind_int(context->mech, 5, snapshot.map_number) < 0 ||
        btech_special_bind_int(context->mech, 6, snapshot.map_dbref) < 0 ||
        sqlite3_bind_text(context->mech, 7, snapshot.definition.mech_name, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(context->mech, 8, snapshot.definition.mech_type, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(context->mech, 9, snapshot.definition.unit_era, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(context->mech, 10, snapshot.definition.unit_tro, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        btech_special_bind_int(context->mech, 11, snapshot.definition.type) <
            0 ||
        btech_special_bind_int(context->mech, 12, snapshot.definition.move) <
            0 ||
        btech_special_bind_int(context->mech, 13,
                               snapshot.definition.tac_range) < 0 ||
        btech_special_bind_int(context->mech, 14,
                               snapshot.definition.lrs_range) < 0 ||
        btech_special_bind_int(context->mech, 15,
                               snapshot.definition.scan_range) < 0 ||
        btech_special_bind_int(context->mech, 16,
                               snapshot.definition.numsinks) < 0 ||
        btech_special_bind_int(context->mech, 17,
                               snapshot.definition.hsengoverride) < 0 ||
        btech_special_bind_int(context->mech, 18,
                               snapshot.definition.computer) < 0 ||
        btech_special_bind_int(context->mech, 19, snapshot.definition.radio) <
            0 ||
        btech_special_bind_int(context->mech, 20,
                               snapshot.definition.radioinfo) < 0 ||
        btech_special_bind_int(context->mech, 21, snapshot.definition.si) < 0 ||
        btech_special_bind_int(context->mech, 22, snapshot.definition.si_orig) <
            0 ||
        btech_special_bind_int(context->mech, 23,
                               snapshot.definition.radio_range) < 0 ||
        btech_special_bind_int(context->mech, 24, snapshot.definition.fuel) <
            0 ||
        btech_special_bind_int(context->mech, 25,
                               snapshot.definition.fuel_orig) < 0 ||
        btech_special_bind_int(context->mech, 26, snapshot.definition.tons) <
            0 ||
        btech_special_bind_int(context->mech, 27,
                               snapshot.definition.walkspeed) < 0 ||
        btech_special_bind_int(context->mech, 28,
                               snapshot.definition.runspeed) < 0 ||
        bind_float(context->mech, 29, snapshot.definition.maxspeed) !=
            SQLITE_OK ||
        bind_float(context->mech, 30, snapshot.definition.template_maxspeed) !=
            SQLITE_OK ||
        btech_special_bind_int(context->mech, 31,
                               snapshot.definition.cargospace) < 0 ||
        btech_special_bind_int(context->mech, 32,
                               snapshot.definition.targcomp) < 0 ||
        btech_special_bind_int(context->mech, 33,
                               snapshot.definition.carmaxton) < 0 ||
        btech_special_write_step(context->fault, context->mech) < 0)
      context->result = -1;
    for (index = 0; context->result == 0 && index < NUM_SECTIONS; index++) {
      const struct MechSection *section = stored_section(&snapshot, index);
      if (btech_special_bind_int(context->section, 1, KEY) < 0 ||
          btech_special_bind_int(context->section, 2, index) < 0 ||
          btech_special_bind_int(context->section, 3, section->armor) < 0 ||
          btech_special_bind_int(context->section, 4, section->internal) < 0 ||
          btech_special_bind_int(context->section, 5, section->rear) < 0 ||
          btech_special_bind_int(context->section, 6, section->armor_orig) <
              0 ||
          btech_special_bind_int(context->section, 7, section->internal_orig) <
              0 ||
          btech_special_bind_int(context->section, 8, section->rear_orig) < 0 ||
          btech_special_bind_int(context->section, 9, section->basetohit) < 0 ||
          btech_special_bind_int(context->section, 10, section->config) < 0 ||
          btech_special_bind_int(context->section, 11, section->recycle) < 0 ||
          btech_special_bind_int(context->section, 12, section->specials) < 0 ||
          btech_special_write_step(context->fault, context->section) < 0) {
        context->result = -1;
        break;
      }
      for (slot = 0; context->result == 0 && slot < NUM_CRITICALS; slot++) {
        const struct CriticalSlot *critical = stored_critical(section, slot);
        if (btech_special_bind_int(context->critical, 1, KEY) < 0 ||
            btech_special_bind_int(context->critical, 2, index) < 0 ||
            btech_special_bind_int(context->critical, 3, slot) < 0 ||
            btech_special_bind_int(context->critical, 4, critical->brand) < 0 ||
            btech_special_bind_int(context->critical, 5, critical->data) < 0 ||
            btech_special_bind_int(context->critical, 6, critical->type) < 0 ||
            btech_special_bind_int(context->critical, 7, critical->firemode) <
                0 ||
            btech_special_bind_int(context->critical, 8, critical->ammomode) <
                0 ||
            btech_special_bind_int(context->critical, 9,
                                   critical->weap_damage_flags) < 0 ||
            btech_special_bind_int(context->critical, 10,
                                   critical->desired_ammo_loc) < 0 ||
            btech_special_write_step(context->fault, context->critical) < 0)
          context->result = -1;
      }
    }
    if (context->result == 0 &&
        (btech_special_bind_int(context->position, 1, KEY) < 0 ||
         btech_special_bind_int(context->position, 2,
                                snapshot.position.pilotstatus) < 0 ||
         bind_float(context->position, 3, snapshot.position.hexes_walked) !=
             SQLITE_OK ||
         btech_special_bind_int(context->position, 4,
                                snapshot.position.facing) < 0 ||
         btech_special_bind_int(context->position, 5, snapshot.position.x) <
             0 ||
         btech_special_bind_int(context->position, 6, snapshot.position.y) <
             0 ||
         btech_special_bind_int(context->position, 7, snapshot.position.z) <
             0 ||
         btech_special_bind_int(context->position, 8,
                                snapshot.position.last_x) < 0 ||
         btech_special_bind_int(context->position, 9,
                                snapshot.position.last_y) < 0 ||
         bind_float(context->position, 10, snapshot.position.fx) != SQLITE_OK ||
         bind_float(context->position, 11, snapshot.position.fy) != SQLITE_OK ||
         bind_float(context->position, 12, snapshot.position.fz) != SQLITE_OK ||
         btech_special_bind_int(context->position, 13, snapshot.position.team) <
             0 ||
         btech_special_bind_int(context->position, 14,
                                snapshot.position.unusable_arcs) < 0 ||
         btech_special_bind_int(context->position, 15,
                                snapshot.position.stall) < 0 ||
         btech_special_bind_int(context->position, 16,
                                snapshot.position.pilot) < 0 ||
         btech_special_write_step(context->fault, context->position) < 0))
      context->result = -1;
    for (index = 0; context->result == 0 && index < NUM_BAYS; index++) {
      if (btech_special_bind_int(context->bay, 1, KEY) < 0 ||
          btech_special_bind_int(context->bay, 2, index) < 0 ||
          btech_special_bind_int(
              context->bay, 3,
              stored_dbref(snapshot.position.bay, NUM_BAYS, index)) < 0 ||
          btech_special_write_step(context->fault, context->bay) < 0)
        context->result = -1;
    }
    for (index = 0; context->result == 0 && index < NUM_TURRETS; index++) {
      if (btech_special_bind_int(context->mech_turret, 1, KEY) < 0 ||
          btech_special_bind_int(context->mech_turret, 2, index) < 0 ||
          btech_special_bind_int(
              context->mech_turret, 3,
              stored_dbref(snapshot.position.turret, NUM_TURRETS, index)) < 0 ||
          btech_special_write_step(context->fault, context->mech_turret) < 0)
        context->result = -1;
    }
    if (context->result == 0 &&
        (btech_special_bind_int(context->c3, 1, KEY) < 0 ||
         sqlite3_bind_text(context->c3, 2, snapshot.network.c3_chan_title, -1,
                           SQLITE_TRANSIENT) != SQLITE_OK ||
         btech_special_bind_int(context->c3, 3,
                                snapshot.network.w_c3i_network_size) < 0 ||
         btech_special_bind_int(context->c3, 4,
                                snapshot.network.w_c3_network_size) < 0 ||
         btech_special_bind_int(context->c3, 5,
                                snapshot.network.w_total_c3_masters) < 0 ||
         btech_special_bind_int(context->c3, 6,
                                snapshot.network.w_working_c3_masters) < 0 ||
         btech_special_bind_int(context->c3, 7, snapshot.network.c3_freq_mode) <
             0 ||
         btech_special_bind_int(context->c3, 8, snapshot.network.tag_target) <
             0 ||
         btech_special_bind_int(context->c3, 9, snapshot.network.tagged_by) <
             0 ||
         btech_special_write_step(context->fault, context->c3) < 0))
      context->result = -1;
    for (index = 0;
         context->result == 0 && index < C3I_NETWORK_SIZE + C3_NETWORK_SIZE;
         index++) {
      DbRef node =
          index < C3I_NETWORK_SIZE
              ? stored_dbref(snapshot.network.c3i_network, C3I_NETWORK_SIZE,
                             index)
              : stored_dbref(snapshot.network.c3_network, C3_NETWORK_SIZE,
                             index - C3I_NETWORK_SIZE);
      int network = index < C3I_NETWORK_SIZE ? 0 : 1;
      int node_index =
          index < C3I_NETWORK_SIZE ? index : index - C3I_NETWORK_SIZE;
      if (btech_special_bind_int(context->c3node, 1, KEY) < 0 ||
          btech_special_bind_int(context->c3node, 2, network) < 0 ||
          btech_special_bind_int(context->c3node, 3, node_index) < 0 ||
          btech_special_bind_int(context->c3node, 4, node) < 0 ||
          btech_special_write_step(context->fault, context->c3node) < 0)
        context->result = -1;
    }
    for (index = 0; context->result == 0 && index < NUM_TICS; index++) {
      for (slot = 0; context->result == 0 && slot < TICLONGS; slot++) {
        if (btech_special_bind_int(context->tic, 1, KEY) < 0 ||
            btech_special_bind_int(context->tic, 2, index) < 0 ||
            btech_special_bind_int(context->tic, 3, slot) < 0 ||
            bind_unsigned_long(context->tic, 4,
                               stored_tic(&snapshot, index, slot)) < 0 ||
            btech_special_write_step(context->fault, context->tic) < 0)
          context->result = -1;
      }
    }
    for (index = 0; context->result == 0 && index < FREQS; index++) {
      if (btech_special_bind_int(context->frequency, 1, KEY) < 0 ||
          btech_special_bind_int(context->frequency, 2, index) < 0 ||
          btech_special_bind_int(
              context->frequency, 3,
              stored_int(snapshot.frequencies, FREQS, index)) < 0 ||
          btech_special_bind_int(
              context->frequency, 4,
              stored_int(snapshot.frequency_modes, FREQS, index)) < 0 ||
          sqlite3_bind_text(context->frequency, 5,
                            stored_channel_title(&snapshot, index), -1,
                            SQLITE_TRANSIENT) != SQLITE_OK ||
          btech_special_write_step(context->fault, context->frequency) < 0)
        context->result = -1;
    }
    if (context->result == 0) {
      runtime_index = 1;
      if (bind_runtime_int(context->runtime, &runtime_index, KEY) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.jumptop) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.aim) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.basetohit) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.pilotskillbase) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.engineheat) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.masc_value) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.aim_type) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.sensor[0]) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.sensor[1]) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.fire_adjustment) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.vis_mod) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.chargetimer) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.chargedist) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.staggerstamp) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.mech_prefs) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.jumplength) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.goingx) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.goingy) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.desiredfacing) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.angle) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.jumpheading) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.targx) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.targy) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.targz) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.turretfacing) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.turndamage) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.lateral) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.num_seen) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.lx) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.ly) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.chgtarget) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.dfatarget) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.target) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.swarming) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.swarmedby) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.carrying) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.spotter) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.heat) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.weapheat) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.plus_heat) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.minus_heat) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.startfx) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.startfy) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.startfz) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.endfz) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.verticalspeed) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.speed) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.desired_speed) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.jumpspeed) < 0 ||
          /* Persist status enums as the legacy SQLite integer bit patterns. */
          bind_runtime_int(context->runtime, &runtime_index,
                           (int)snapshot.runtime.critstatus) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           (int)snapshot.runtime.status) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           (int)snapshot.runtime.status2) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.specials) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.specials2) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           (int)snapshot.runtime.specialsstatus) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           (int)snapshot.runtime.tankcritstatus) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.last_weapon_recycle) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.cargo_weight) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.lastrndu) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.rnd) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.last_ds_msg) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.boom_start) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.maxfuel) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.lastused) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.cocoon) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.commconv) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.commconv_last) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.onumsinks) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.disabled_hs) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.autopilot_num) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.heatboom_last) < 0 ||
          bind_runtime_time(context->runtime, &runtime_index,
                            snapshot.runtime.sspin) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.can_see) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.row) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.rcw) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.rspd) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.erat) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.per) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.wxf) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.last_startup) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.maxsuits) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.infantry_specials) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.scharge_value) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.stagger_damage) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.last_stagger_notify) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           (int)snapshot.runtime.critstatus2) < 0 ||
          bind_runtime_real(context->runtime, &runtime_index,
                            snapshot.runtime.xpmod) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.shots_fired) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.shots_hit) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.shots_missed) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.damage_taken) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.damage_inflicted) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.units_killed) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.last_stagger_check) < 0 ||
          btech_special_write_step(context->fault, context->runtime) < 0)
        context->result = -1;
    }
    for (index = 0; context->result == 0 && index < 3; index++) {
      if (btech_special_bind_int(context->unit_aux, 1, KEY) < 0 ||
          btech_special_bind_int(context->unit_aux, 2, 8 + index) < 0 ||
          btech_special_bind_int(
              context->unit_aux, 3,
              stored_char(snapshot.definition.unused_char, 3, index)) < 0 ||
          btech_special_write_step(context->fault, context->unit_aux) < 0)
        context->result = -1;
    }
    for (index = 0;
         context->result == 0 && mech_stagger_damage_get(mech, index, &damage);
         index++) {
      if (btech_special_bind_int(context->stagger_damage, 1, KEY) < 0 ||
          btech_special_bind_int(context->stagger_damage, 2, index) < 0 ||
          btech_special_bind_int(context->stagger_damage, 3, damage.amount) <
              0 ||
          btech_special_bind_int(context->stagger_damage, 4,
                                 (sqlite3_int64)damage.occurred_at) < 0 ||
          btech_special_bind_int(context->stagger_damage, 5, damage.attacker) <
              0 ||
          btech_special_bind_int(context->stagger_damage, 6, damage.counted) <
              0 ||
          btech_special_write_step(context->fault, context->stagger_damage) < 0)
        context->result = -1;
    }
  } else {
    btech_store_auxiliary_object(context, KEY, xcode);
  }
  return context->result == 0;
}

/* Store one map's scalar state plus its explicit occupancy and LOS matrices. */
