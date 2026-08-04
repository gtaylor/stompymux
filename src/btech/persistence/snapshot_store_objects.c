#include "sqlite_internal.h"

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
  int index;
  int slot;
  int argument_index;
  int runtime_index;
  AutopilotCommand *command;
  AutopilotPathNode *path_node;
  MechDamageRecord *damage;

  (void)depth;
  if (context->result < 0)
    return 0;
  if (xcode->type == GTYPE_MECH) {
    mech = (Mech *)xcode;
    if (btech_special_bind_int(context->mech, 1, (DbRef)key) < 0 ||
        btech_special_bind_int(context->mech, 2, mech->ID[0]) < 0 ||
        btech_special_bind_int(context->mech, 3, mech->ID[1]) < 0 ||
        btech_special_bind_int(context->mech, 4, mech->brief) < 0 ||
        btech_special_bind_int(context->mech, 5, mech->mapnumber) < 0 ||
        btech_special_bind_int(context->mech, 6, mech->mapindex) < 0 ||
        sqlite3_bind_text(context->mech, 7, mech->ud.mech_name, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(context->mech, 8, mech->ud.mech_type, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(context->mech, 9, mech->ud.unit_era, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(context->mech, 10, mech->ud.unit_tro, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        btech_special_bind_int(context->mech, 11, mech->ud.type) < 0 ||
        btech_special_bind_int(context->mech, 12, mech->ud.move) < 0 ||
        btech_special_bind_int(context->mech, 13, mech->ud.tac_range) < 0 ||
        btech_special_bind_int(context->mech, 14, mech->ud.lrs_range) < 0 ||
        btech_special_bind_int(context->mech, 15, mech->ud.scan_range) < 0 ||
        btech_special_bind_int(context->mech, 16, mech->ud.numsinks) < 0 ||
        btech_special_bind_int(context->mech, 17, mech->ud.hsengoverride) < 0 ||
        btech_special_bind_int(context->mech, 18, mech->ud.computer) < 0 ||
        btech_special_bind_int(context->mech, 19, mech->ud.radio) < 0 ||
        btech_special_bind_int(context->mech, 20, mech->ud.radioinfo) < 0 ||
        btech_special_bind_int(context->mech, 21, mech->ud.si) < 0 ||
        btech_special_bind_int(context->mech, 22, mech->ud.si_orig) < 0 ||
        btech_special_bind_int(context->mech, 23, mech->ud.radio_range) < 0 ||
        btech_special_bind_int(context->mech, 24, mech->ud.fuel) < 0 ||
        btech_special_bind_int(context->mech, 25, mech->ud.fuel_orig) < 0 ||
        btech_special_bind_int(context->mech, 26, mech->ud.tons) < 0 ||
        btech_special_bind_int(context->mech, 27, mech->ud.walkspeed) < 0 ||
        btech_special_bind_int(context->mech, 28, mech->ud.runspeed) < 0 ||
        sqlite3_bind_double(context->mech, 29, mech->ud.maxspeed) !=
            SQLITE_OK ||
        sqlite3_bind_double(context->mech, 30, mech->ud.template_maxspeed) !=
            SQLITE_OK ||
        btech_special_bind_int(context->mech, 31, mech->ud.mechbv) < 0 ||
        btech_special_bind_int(context->mech, 32, mech->ud.cargospace) < 0 ||
        btech_special_bind_int(context->mech, 33, mech->ud.targcomp) < 0 ||
        btech_special_bind_int(context->mech, 34, mech->ud.carmaxton) < 0 ||
        btech_special_step(context->mech) < 0)
      context->result = -1;
    for (index = 0; context->result == 0 && index < NUM_SECTIONS; index++) {
      struct MechSection *section = &mech->ud.sections[index];
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
         btech_special_bind_int(context->position, 2, mech->pd.pilotstatus) <
             0 ||
         btech_special_bind_int(context->position, 3, mech->pd.terrain) < 0 ||
         btech_special_bind_int(context->position, 4, mech->pd.elev) < 0 ||
         sqlite3_bind_double(context->position, 5, mech->pd.hexes_walked) !=
             SQLITE_OK ||
         btech_special_bind_int(context->position, 6, mech->pd.facing) < 0 ||
         btech_special_bind_int(context->position, 7, mech->pd.x) < 0 ||
         btech_special_bind_int(context->position, 8, mech->pd.y) < 0 ||
         btech_special_bind_int(context->position, 9, mech->pd.z) < 0 ||
         btech_special_bind_int(context->position, 10, mech->pd.last_x) < 0 ||
         btech_special_bind_int(context->position, 11, mech->pd.last_y) < 0 ||
         sqlite3_bind_double(context->position, 12, mech->pd.fx) != SQLITE_OK ||
         sqlite3_bind_double(context->position, 13, mech->pd.fy) != SQLITE_OK ||
         sqlite3_bind_double(context->position, 14, mech->pd.fz) != SQLITE_OK ||
         btech_special_bind_int(context->position, 15, mech->pd.team) < 0 ||
         btech_special_bind_int(context->position, 16, mech->pd.unusable_arcs) <
             0 ||
         btech_special_bind_int(context->position, 17, mech->pd.stall) < 0 ||
         btech_special_bind_int(context->position, 18, mech->pd.pilot) < 0 ||
         btech_special_step(context->position) < 0))
      context->result = -1;
    for (index = 0; context->result == 0 && index < NUM_BAYS; index++) {
      if (btech_special_bind_int(context->bay, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->bay, 2, index) < 0 ||
          btech_special_bind_int(context->bay, 3, mech->pd.bay[index]) < 0 ||
          btech_special_step(context->bay) < 0)
        context->result = -1;
    }
    for (index = 0; context->result == 0 && index < NUM_TURRETS; index++) {
      if (btech_special_bind_int(context->mech_turret, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->mech_turret, 2, index) < 0 ||
          btech_special_bind_int(context->mech_turret, 3,
                                 mech->pd.turret[index]) < 0 ||
          btech_special_step(context->mech_turret) < 0)
        context->result = -1;
    }
    if (context->result == 0 &&
        (btech_special_bind_int(context->c3, 1, (DbRef)key) < 0 ||
         sqlite3_bind_text(context->c3, 2, mech->sd.C3ChanTitle, -1,
                           SQLITE_TRANSIENT) != SQLITE_OK ||
         btech_special_bind_int(context->c3, 3, mech->sd.wC3iNetworkSize) < 0 ||
         btech_special_bind_int(context->c3, 4, mech->sd.wC3NetworkSize) < 0 ||
         btech_special_bind_int(context->c3, 5, mech->sd.wTotalC3Masters) < 0 ||
         btech_special_bind_int(context->c3, 6, mech->sd.wWorkingC3Masters) <
             0 ||
         btech_special_bind_int(context->c3, 7, mech->sd.C3FreqMode) < 0 ||
         btech_special_bind_int(context->c3, 8, mech->sd.tagTarget) < 0 ||
         btech_special_bind_int(context->c3, 9, mech->sd.taggedBy) < 0 ||
         btech_special_step(context->c3) < 0))
      context->result = -1;
    for (index = 0;
         context->result == 0 && index < C3I_NETWORK_SIZE + C3_NETWORK_SIZE;
         index++) {
      DbRef node = index < C3I_NETWORK_SIZE
                       ? mech->sd.C3iNetwork[index]
                       : mech->sd.C3Network[index - C3I_NETWORK_SIZE];
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
            btech_special_bind_int(context->tic, 4, mech->tic[index][slot]) <
                0 ||
            btech_special_step(context->tic) < 0)
          context->result = -1;
      }
    }
    for (index = 0; context->result == 0 && index < FREQS; index++) {
      if (btech_special_bind_int(context->frequency, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->frequency, 2, index) < 0 ||
          btech_special_bind_int(context->frequency, 3, mech->freq[index]) <
              0 ||
          btech_special_bind_int(context->frequency, 4,
                                 mech->freqmodes[index]) < 0 ||
          sqlite3_bind_text(context->frequency, 5, mech->chantitle[index], -1,
                            SQLITE_TRANSIENT) != SQLITE_OK ||
          btech_special_step(context->frequency) < 0)
        context->result = -1;
    }
    if (context->result == 0) {
      runtime_index = 1;
#define BTECH_RUNTIME_INT(value)                                               \
  btech_special_bind_int(context->runtime, runtime_index++,                    \
                         (sqlite3_int64)(value))
#define BTECH_RUNTIME_REAL(value)                                              \
  btech_special_bind_real(context->runtime, runtime_index++, (double)(value))
      if (BTECH_RUNTIME_INT((DbRef)key) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.jumptop) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.aim) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.basetohit) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.pilotskillbase) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.engineheat) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.masc_value) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.aim_type) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.sensor[0]) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.sensor[1]) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.fire_adjustment) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.vis_mod) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.chargetimer) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.chargedist) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.staggerstamp) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.mech_prefs) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.jumplength) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.goingx) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.goingy) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.desiredfacing) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.angle) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.jumpheading) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.targx) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.targy) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.targz) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.turretfacing) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.turndamage) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lateral) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.num_seen) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lx) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.ly) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.chgtarget) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.dfatarget) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.target) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.swarming) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.swarmedby) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.carrying) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.spotter) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.heat) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.weapheat) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.plus_heat) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.minus_heat) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.startfx) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.startfy) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.startfz) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.endfz) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.verticalspeed) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.speed) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.desired_speed) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.jumpspeed) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.critstatus) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.status) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.status2) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.specials) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.specials2) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.specialsstatus) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.tankcritstatus) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.last_weapon_recycle) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.cargo_weight) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lastrndu) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.rnd) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.last_ds_msg) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.boom_start) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.maxfuel) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lastused) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.cocoon) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.commconv) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.commconv_last) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.onumsinks) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.disabled_hs) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.autopilot_num) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.heatboom_last) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.sspin) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.can_see) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.row) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.rcw) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.rspd) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.erat) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.per) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.wxf) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.last_startup) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.maxsuits) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.infantry_specials) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.scharge_value) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.staggerDamage) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lastStaggerNotify) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.critstatus2) < 0 ||
          BTECH_RUNTIME_REAL(mech->rd.xpmod) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.shots_fired) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.shots_hit) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.shots_missed) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.damage_taken) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.damage_inflicted) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.units_killed) < 0 ||
          BTECH_RUNTIME_INT(mech->rd.lastStaggerCheck) < 0 ||
          btech_special_step(context->runtime) < 0)
        context->result = -1;
#undef BTECH_RUNTIME_INT
#undef BTECH_RUNTIME_REAL
    }
    for (index = 0; context->result == 0 && index < 5; index++) {
      if (btech_special_bind_int(context->runtime_unused, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->runtime_unused, 2, index) < 0 ||
          btech_special_bind_int(context->runtime_unused, 3,
                                 mech->rd.unused[index]) < 0 ||
          btech_special_step(context->runtime_unused) < 0)
        context->result = -1;
    }
#ifndef BT_CALCULATE_BV
    for (index = 0; context->result == 0 && index < 8; index++) {
      if (btech_special_bind_int(context->unit_aux, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->unit_aux, 2, index) < 0 ||
          btech_special_bind_int(context->unit_aux, 3, mech->ud.unused[index]) <
              0 ||
          btech_special_step(context->unit_aux) < 0)
        context->result = -1;
    }
#else
    if (context->result == 0 &&
        (btech_special_bind_int(context->unit_aux, 1, (DbRef)key) < 0 ||
         btech_special_bind_int(context->unit_aux, 2, 0) < 0 ||
         btech_special_bind_int(context->unit_aux, 3, mech->ud.mechbv_last) <
             0 ||
         btech_special_step(context->unit_aux) < 0))
      context->result = -1;
#endif
    for (index = 0; context->result == 0 && index < 3; index++) {
      if (btech_special_bind_int(context->unit_aux, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->unit_aux, 2, 8 + index) < 0 ||
          btech_special_bind_int(context->unit_aux, 3,
                                 mech->ud.unused_char[index]) < 0 ||
          btech_special_step(context->unit_aux) < 0)
        context->result = -1;
    }
    for (damage = mech->rd.staggerDamageList, index = 0;
         context->result == 0 && damage; damage = damage->next, index++) {
      if (btech_special_bind_int(context->stagger_damage, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->stagger_damage, 2, index) < 0 ||
          btech_special_bind_int(context->stagger_damage, 3, damage->amount) <
              0 ||
          btech_special_bind_int(context->stagger_damage, 4,
                                 (sqlite3_int64)damage->occuredAt) < 0 ||
          btech_special_bind_int(context->stagger_damage, 5,
                                 damage->attackerNum) < 0 ||
          btech_special_bind_int(context->stagger_damage, 6, damage->counted) <
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
          btech_special_bind_int(context->turret_tic, 3, turret->tic[index]) <
              0 ||
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
