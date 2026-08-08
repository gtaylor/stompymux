#include "sqlite_internal.h"

#include <stdint.h>

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

void btech_finalize_object_statements(BTECH_OBJECT_STORE_CONTEXT *context) {
  sqlite3_finalize(context->mechrep);
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
  sqlite3_finalize(context->runtime_unused);
  sqlite3_finalize(context->unit_aux);
  sqlite3_finalize(context->stagger_damage);
  sqlite3_finalize(context->autopilot_command);
  sqlite3_finalize(context->autopilot_command_arg);
  sqlite3_finalize(context->autopilot_path);
}

/* Replace the default map grid with a validated grid owned by this MAP. */

int btech_store_simple_object(void *key, void *data, int depth,
                              void *argument) {
  BTECH_OBJECT_STORE_CONTEXT *context = argument;
  BtechSpecialObject *xcode = data;
  RepairFacility *mechrep;
  Turret *turret;
  Autopilot *autopilot;
  Mech *mech;
  MechPersistenceSnapshot snapshot;
  int index;
  int slot;
  int argument_index;
  int runtime_index;
  AutopilotCommand *command;
  AutopilotPathNode *path_node;
  MechStaggerDamageSnapshot damage;

  (void)depth;
  if (context->result < 0)
    return 0;
  if (xcode->type == GTYPE_MECH) {
    mech = (Mech *)xcode;
    mech_persistence_snapshot_export(mech, &snapshot);
    if (btech_special_bind_int(context->mech, 1, (DbRef)key) < 0 ||
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
        btech_special_bind_int(context->mech, 31, snapshot.definition.mechbv) <
            0 ||
        btech_special_bind_int(context->mech, 32,
                               snapshot.definition.cargospace) < 0 ||
        btech_special_bind_int(context->mech, 33,
                               snapshot.definition.targcomp) < 0 ||
        btech_special_bind_int(context->mech, 34,
                               snapshot.definition.carmaxton) < 0 ||
        btech_special_step(context->mech) < 0)
      context->result = -1;
    for (index = 0; context->result == 0 && index < NUM_SECTIONS; index++) {
      struct MechSection *section = &snapshot.definition.sections[index];
      if (btech_special_bind_int(context->section, 1, (DbRef)key) < 0 ||
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
          btech_special_step(context->section) < 0) {
        context->result = -1;
        break;
      }
      for (slot = 0; context->result == 0 && slot < NUM_CRITICALS; slot++) {
        struct CriticalSlot *critical = &section->criticals[slot];
        if (btech_special_bind_int(context->critical, 1, (DbRef)key) < 0 ||
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
                                   critical->weapDamageFlags) < 0 ||
            btech_special_bind_int(context->critical, 10,
                                   critical->desiredAmmoLoc) < 0 ||
            btech_special_step(context->critical) < 0)
          context->result = -1;
      }
    }
    if (context->result == 0 &&
        (btech_special_bind_int(context->position, 1, (DbRef)key) < 0 ||
         btech_special_bind_int(context->position, 2,
                                snapshot.position.pilotstatus) < 0 ||
         btech_special_bind_int(context->position, 3,
                                snapshot.position.terrain) < 0 ||
         btech_special_bind_int(context->position, 4, snapshot.position.elev) <
             0 ||
         bind_float(context->position, 5, snapshot.position.hexes_walked) !=
             SQLITE_OK ||
         btech_special_bind_int(context->position, 6,
                                snapshot.position.facing) < 0 ||
         btech_special_bind_int(context->position, 7, snapshot.position.x) <
             0 ||
         btech_special_bind_int(context->position, 8, snapshot.position.y) <
             0 ||
         btech_special_bind_int(context->position, 9, snapshot.position.z) <
             0 ||
         btech_special_bind_int(context->position, 10,
                                snapshot.position.last_x) < 0 ||
         btech_special_bind_int(context->position, 11,
                                snapshot.position.last_y) < 0 ||
         bind_float(context->position, 12, snapshot.position.fx) != SQLITE_OK ||
         bind_float(context->position, 13, snapshot.position.fy) != SQLITE_OK ||
         bind_float(context->position, 14, snapshot.position.fz) != SQLITE_OK ||
         btech_special_bind_int(context->position, 15, snapshot.position.team) <
             0 ||
         btech_special_bind_int(context->position, 16,
                                snapshot.position.unusable_arcs) < 0 ||
         btech_special_bind_int(context->position, 17,
                                snapshot.position.stall) < 0 ||
         btech_special_bind_int(context->position, 18,
                                snapshot.position.pilot) < 0 ||
         btech_special_step(context->position) < 0))
      context->result = -1;
    for (index = 0; context->result == 0 && index < NUM_BAYS; index++) {
      if (btech_special_bind_int(context->bay, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->bay, 2, index) < 0 ||
          btech_special_bind_int(context->bay, 3,
                                 snapshot.position.bay[index]) < 0 ||
          btech_special_step(context->bay) < 0)
        context->result = -1;
    }
    for (index = 0; context->result == 0 && index < NUM_TURRETS; index++) {
      if (btech_special_bind_int(context->mech_turret, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->mech_turret, 2, index) < 0 ||
          btech_special_bind_int(context->mech_turret, 3,
                                 snapshot.position.turret[index]) < 0 ||
          btech_special_step(context->mech_turret) < 0)
        context->result = -1;
    }
    if (context->result == 0 &&
        (btech_special_bind_int(context->c3, 1, (DbRef)key) < 0 ||
         sqlite3_bind_text(context->c3, 2, snapshot.network.C3ChanTitle, -1,
                           SQLITE_TRANSIENT) != SQLITE_OK ||
         btech_special_bind_int(context->c3, 3,
                                snapshot.network.wC3iNetworkSize) < 0 ||
         btech_special_bind_int(context->c3, 4,
                                snapshot.network.wC3NetworkSize) < 0 ||
         btech_special_bind_int(context->c3, 5,
                                snapshot.network.wTotalC3Masters) < 0 ||
         btech_special_bind_int(context->c3, 6,
                                snapshot.network.wWorkingC3Masters) < 0 ||
         btech_special_bind_int(context->c3, 7, snapshot.network.C3FreqMode) <
             0 ||
         btech_special_bind_int(context->c3, 8, snapshot.network.tagTarget) <
             0 ||
         btech_special_bind_int(context->c3, 9, snapshot.network.taggedBy) <
             0 ||
         btech_special_step(context->c3) < 0))
      context->result = -1;
    for (index = 0;
         context->result == 0 && index < C3I_NETWORK_SIZE + C3_NETWORK_SIZE;
         index++) {
      DbRef node = index < C3I_NETWORK_SIZE
                       ? snapshot.network.C3iNetwork[index]
                       : snapshot.network.C3Network[index - C3I_NETWORK_SIZE];
      int network = index < C3I_NETWORK_SIZE ? 0 : 1;
      int node_index =
          index < C3I_NETWORK_SIZE ? index : index - C3I_NETWORK_SIZE;
      if (btech_special_bind_int(context->c3node, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->c3node, 2, network) < 0 ||
          btech_special_bind_int(context->c3node, 3, node_index) < 0 ||
          btech_special_bind_int(context->c3node, 4, node) < 0 ||
          btech_special_step(context->c3node) < 0)
        context->result = -1;
    }
    for (index = 0; context->result == 0 && index < NUM_TICS; index++) {
      for (slot = 0; context->result == 0 && slot < TICLONGS; slot++) {
        if (btech_special_bind_int(context->tic, 1, (DbRef)key) < 0 ||
            btech_special_bind_int(context->tic, 2, index) < 0 ||
            btech_special_bind_int(context->tic, 3, slot) < 0 ||
            bind_unsigned_long(context->tic, 4, snapshot.tics[index][slot]) <
                0 ||
            btech_special_step(context->tic) < 0)
          context->result = -1;
      }
    }
    for (index = 0; context->result == 0 && index < FREQS; index++) {
      if (btech_special_bind_int(context->frequency, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->frequency, 2, index) < 0 ||
          btech_special_bind_int(context->frequency, 3,
                                 snapshot.frequencies[index]) < 0 ||
          btech_special_bind_int(context->frequency, 4,
                                 snapshot.frequency_modes[index]) < 0 ||
          sqlite3_bind_text(context->frequency, 5,
                            snapshot.channel_titles[index], -1,
                            SQLITE_TRANSIENT) != SQLITE_OK ||
          btech_special_step(context->frequency) < 0)
        context->result = -1;
    }
    if (context->result == 0) {
      runtime_index = 1;
      if (bind_runtime_int(context->runtime, &runtime_index, (DbRef)key) < 0 ||
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
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.critstatus) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.status) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.status2) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.specials) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.specials2) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.specialsstatus) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.tankcritstatus) < 0 ||
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
                           snapshot.runtime.staggerDamage) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.lastStaggerNotify) < 0 ||
          bind_runtime_int(context->runtime, &runtime_index,
                           snapshot.runtime.critstatus2) < 0 ||
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
                           snapshot.runtime.lastStaggerCheck) < 0 ||
          btech_special_step(context->runtime) < 0)
        context->result = -1;
    }
    for (index = 0; context->result == 0 && index < 5; index++) {
      if (btech_special_bind_int(context->runtime_unused, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->runtime_unused, 2, index) < 0 ||
          btech_special_bind_int(context->runtime_unused, 3,
                                 snapshot.runtime.unused[index]) < 0 ||
          btech_special_step(context->runtime_unused) < 0)
        context->result = -1;
    }
#ifndef BT_CALCULATE_BV
    for (index = 0; context->result == 0 && index < 8; index++) {
      if (btech_special_bind_int(context->unit_aux, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->unit_aux, 2, index) < 0 ||
          btech_special_bind_int(context->unit_aux, 3,
                                 snapshot.definition.unused[index]) < 0 ||
          btech_special_step(context->unit_aux) < 0)
        context->result = -1;
    }
#else
    if (context->result == 0 &&
        (btech_special_bind_int(context->unit_aux, 1, (DbRef)key) < 0 ||
         btech_special_bind_int(context->unit_aux, 2, 0) < 0 ||
         btech_special_bind_int(context->unit_aux, 3,
                                snapshot.definition.mechbv_last) < 0 ||
         btech_special_step(context->unit_aux) < 0))
      context->result = -1;
#endif
    for (index = 0; context->result == 0 && index < 3; index++) {
      if (btech_special_bind_int(context->unit_aux, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->unit_aux, 2, 8 + index) < 0 ||
          btech_special_bind_int(context->unit_aux, 3,
                                 snapshot.definition.unused_char[index]) < 0 ||
          btech_special_step(context->unit_aux) < 0)
        context->result = -1;
    }
    for (index = 0;
         context->result == 0 && mech_stagger_damage_get(mech, index, &damage);
         index++) {
      if (btech_special_bind_int(context->stagger_damage, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->stagger_damage, 2, index) < 0 ||
          btech_special_bind_int(context->stagger_damage, 3, damage.amount) <
              0 ||
          btech_special_bind_int(context->stagger_damage, 4,
                                 (sqlite3_int64)damage.occurred_at) < 0 ||
          btech_special_bind_int(context->stagger_damage, 5, damage.attacker) <
              0 ||
          btech_special_bind_int(context->stagger_damage, 6, damage.counted) <
              0 ||
          btech_special_step(context->stagger_damage) < 0)
        context->result = -1;
    }
  } else if (xcode->type == GTYPE_MECHREP) {
    mechrep = (RepairFacility *)xcode;
    if (btech_special_bind_int(context->mechrep, 1, (DbRef)key) < 0 ||
        btech_special_bind_int(context->mechrep, 2, mechrep->current_target) <
            0 ||
        btech_special_step(context->mechrep) < 0)
      context->result = -1;
  } else if (xcode->type == GTYPE_TURRET) {
    turret = (Turret *)xcode;
    if (btech_special_bind_int(context->turret, 1, (DbRef)key) < 0 ||
        btech_special_bind_int(context->turret, 2, turret->arcs) < 0 ||
        btech_special_bind_int(context->turret, 3, turret->parent) < 0 ||
        btech_special_bind_int(context->turret, 4, turret->gunner) < 0 ||
        btech_special_bind_int(context->turret, 5, turret->target) < 0 ||
        btech_special_bind_int(context->turret, 6, turret->targx) < 0 ||
        btech_special_bind_int(context->turret, 7, turret->targy) < 0 ||
        btech_special_bind_int(context->turret, 8, turret->targz) < 0 ||
        btech_special_bind_int(context->turret, 9, turret->lockmode) < 0 ||
        btech_special_step(context->turret) < 0)
      context->result = -1;
    for (index = 0; context->result == 0 && index < NUM_TICS; index++) {
      if (btech_special_bind_int(context->turret_tic, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->turret_tic, 2, index) < 0 ||
          bind_unsigned_long(context->turret_tic, 3, turret->tic[index]) < 0 ||
          btech_special_step(context->turret_tic) < 0)
        context->result = -1;
    }
  } else if (xcode->type == GTYPE_AUTO) {
    autopilot = (Autopilot *)xcode;
    if (btech_special_bind_int(context->autopilot, 1, (DbRef)key) < 0 ||
        btech_special_bind_int(context->autopilot, 2, autopilot->mymechnum) <
            0 ||
        btech_special_bind_int(context->autopilot, 3, autopilot->mapindex) <
            0 ||
        btech_special_bind_int(context->autopilot, 4, autopilot->speed) < 0 ||
        btech_special_bind_int(context->autopilot, 5, autopilot->ofsx) < 0 ||
        btech_special_bind_int(context->autopilot, 6, autopilot->ofsy) < 0 ||
        btech_special_bind_int(context->autopilot, 7,
                               autopilot->verbose_level) < 0 ||
        btech_special_bind_int(context->autopilot, 8, autopilot->target) < 0 ||
        btech_special_bind_int(context->autopilot, 9, autopilot->target_score) <
            0 ||
        btech_special_bind_int(context->autopilot, 10,
                               autopilot->target_threshold) < 0 ||
        btech_special_bind_int(context->autopilot, 11,
                               autopilot->target_update_tick) < 0 ||
        btech_special_bind_int(context->autopilot, 12,
                               autopilot->chase_target) < 0 ||
        btech_special_bind_int(context->autopilot, 13,
                               autopilot->chasetarg_update_tick) < 0 ||
        btech_special_bind_int(context->autopilot, 14,
                               autopilot->follow_update_tick) < 0 ||
        btech_special_bind_int(context->autopilot, 15, autopilot->flags) < 0 ||
        btech_special_bind_int(context->autopilot, 16,
                               autopilot->mech_max_range) < 0 ||
        btech_special_bind_int(context->autopilot, 17, autopilot->roam_type) <
            0 ||
        btech_special_bind_int(context->autopilot, 18,
                               autopilot->roam_update_tick) < 0 ||
        btech_special_bind_int(context->autopilot, 19,
                               autopilot->roam_target_hex_x) < 0 ||
        btech_special_bind_int(context->autopilot, 20,
                               autopilot->roam_target_hex_y) < 0 ||
        btech_special_bind_int(context->autopilot, 21,
                               autopilot->roam_anchor_hex_x) < 0 ||
        btech_special_bind_int(context->autopilot, 22,
                               autopilot->roam_anchor_hex_y) < 0 ||
        btech_special_bind_int(context->autopilot, 23,
                               autopilot->roam_anchor_distance) < 0 ||
        btech_special_bind_int(context->autopilot, 24, autopilot->ahead_ok) <
            0 ||
        btech_special_bind_int(context->autopilot, 25, autopilot->auto_cmode) <
            0 ||
        btech_special_bind_int(context->autopilot, 26, autopilot->auto_cdist) <
            0 ||
        btech_special_bind_int(context->autopilot, 27,
                               autopilot->auto_goweight) < 0 ||
        btech_special_bind_int(context->autopilot, 28,
                               autopilot->auto_fweight) < 0 ||
        btech_special_bind_int(context->autopilot, 29,
                               autopilot->auto_nervous) < 0 ||
        btech_special_bind_int(context->autopilot, 30, autopilot->b_msc) < 0 ||
        btech_special_bind_int(context->autopilot, 31, autopilot->w_msc) < 0 ||
        btech_special_bind_int(context->autopilot, 32, autopilot->b_bsc) < 0 ||
        btech_special_bind_int(context->autopilot, 33, autopilot->w_bsc) < 0 ||
        btech_special_bind_int(context->autopilot, 34, autopilot->b_dan) < 0 ||
        btech_special_bind_int(context->autopilot, 35, autopilot->w_dan) < 0 ||
        btech_special_bind_int(context->autopilot, 36, autopilot->last_upd) <
            0 ||
        btech_special_step(context->autopilot) < 0)
      context->result = -1;
    for (index = 1; context->result == 0 && autopilot->commands &&
                    index <= doubly_linked_list_size(autopilot->commands);
         index++) {
      command = (AutopilotCommand *)doubly_linked_list_get_node(
          autopilot->commands, index);
      if (!command || command->argcount >= AUTOPILOT_MAX_ARGS ||
          btech_special_bind_int(context->autopilot_command, 1, (DbRef)key) <
              0 ||
          btech_special_bind_int(context->autopilot_command, 2, index - 1) <
              0 ||
          btech_special_bind_int(context->autopilot_command, 3,
                                 command->command_enum) < 0 ||
          btech_special_bind_int(context->autopilot_command, 4,
                                 command->argcount + 1) < 0 ||
          btech_special_step(context->autopilot_command) < 0) {
        context->result = -1;
        break;
      }
      for (argument_index = 0;
           context->result == 0 && argument_index <= command->argcount;
           argument_index++) {
        if (!command->args[argument_index] ||
            btech_special_bind_int(context->autopilot_command_arg, 1,
                                   (DbRef)key) < 0 ||
            btech_special_bind_int(context->autopilot_command_arg, 2,
                                   index - 1) < 0 ||
            btech_special_bind_int(context->autopilot_command_arg, 3,
                                   argument_index) < 0 ||
            sqlite3_bind_text(context->autopilot_command_arg, 4,
                              command->args[argument_index], -1,
                              SQLITE_TRANSIENT) != SQLITE_OK ||
            btech_special_step(context->autopilot_command_arg) < 0)
          context->result = -1;
      }
    }
    for (index = 1; context->result == 0 && autopilot->astar_path &&
                    index <= doubly_linked_list_size(autopilot->astar_path);
         index++) {
      path_node = (AutopilotPathNode *)doubly_linked_list_get_node(
          autopilot->astar_path, index);
      if (!path_node ||
          btech_special_bind_int(context->autopilot_path, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->autopilot_path, 2, index - 1) < 0 ||
          btech_special_bind_int(context->autopilot_path, 3, path_node->x) <
              0 ||
          btech_special_bind_int(context->autopilot_path, 4, path_node->y) <
              0 ||
          btech_special_bind_int(context->autopilot_path, 5,
                                 path_node->x_parent) < 0 ||
          btech_special_bind_int(context->autopilot_path, 6,
                                 path_node->y_parent) < 0 ||
          btech_special_bind_int(context->autopilot_path, 7,
                                 path_node->g_score) < 0 ||
          btech_special_bind_int(context->autopilot_path, 8,
                                 path_node->h_score) < 0 ||
          btech_special_bind_int(context->autopilot_path, 9,
                                 path_node->f_score) < 0 ||
          btech_special_bind_int(context->autopilot_path, 10,
                                 path_node->hexoffset) < 0 ||
          btech_special_step(context->autopilot_path) < 0) {
        context->result = -1;
        break;
      }
    }
  }
  return context->result == 0;
}

/* Store one map's scalar state plus its explicit occupancy and LOS matrices. */
