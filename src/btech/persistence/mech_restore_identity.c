#include "ai_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_persistence.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "sqlite_internal.h"
#include <limits.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

static struct MechSection *snapshot_section(MechPersistenceSnapshot *snapshot,
                                            int index) {
  if (index < 0)
    abort();
  return checked_storage_at(snapshot->definition.sections, NUM_SECTIONS,
                            sizeof(*snapshot->definition.sections),
                            (size_t)index);
}

static struct CriticalSlot *snapshot_critical(MechPersistenceSnapshot *snapshot,
                                              CriticalSlotReference reference) {
  if (reference.critical < 0)
    abort();
  struct MechSection *section_data =
      snapshot_section(snapshot, reference.section);
  return checked_storage_at(section_data->criticals, NUM_CRITICALS,
                            sizeof(*section_data->criticals),
                            (size_t)reference.critical);
}

int btech_special_load_mech_parents(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  MechPersistenceSnapshot snapshot;
  char mech_name[sizeof(snapshot.definition.mech_name)];
  char mech_type[sizeof(snapshot.definition.mech_type)];
  char unit_era[sizeof(snapshot.definition.unit_era)];
  char unit_tro[sizeof(snapshot.definition.unit_tro)];
  DbRef mech_dbref;
  long map_dbref;
  float max_speed;
  float template_max_speed;
  int battle_value;
  int cargo_space;
  int carrier_max_tons;
  int computer;
  int fuel;
  int fuel_original;
  int heat_sink_override;
  int heat_sinks;
  int id_0;
  int id_1;
  int lrs_range;
  int map_number;
  int movement_type;
  int radio;
  int radio_info;
  int radio_range;
  int result;
  int run_speed;
  int scan_range;
  int step;
  int structural_integrity;
  int structural_integrity_original;
  int tactical_range;
  int targeting_computer;
  int tons;
  int unit_class;
  int walk_speed;
  int brief;

  statement = NULL;
  result =
      SQLITE3_PREPARE_V2(
          sqlite,
          "SELECT dbref, id_0, id_1, brief, map_number, map_dbref, "
          "mech_name, mech_type, unit_era, unit_tro, unit_class, "
          "movement_type, tactical_range, lrs_range, scan_range, heat_sinks, "
          "heat_sink_override, computer, radio, radio_info, "
          "structural_integrity, structural_integrity_original, radio_range, "
          "fuel, fuel_original, tons, walk_speed, run_speed, max_speed, "
          "template_max_speed, battle_value, cargo_space, targeting_computer, "
          "carrier_max_tons FROM btech_mechs ORDER BY dbref;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0) {
      result = -1;
      break;
    }
    mech = btech_context_get_mech(context, mech_dbref);
    if (!mech || btech_special_column_int(statement, 1, &id_0) < 0 ||
        btech_special_column_int(statement, 2, &id_1) < 0 ||
        btech_special_column_int(statement, 3, &brief) < 0 ||
        btech_special_column_int(statement, 4, &map_number) < 0 ||
        btech_special_column_long(statement, 5, &map_dbref) < 0 ||
        btech_special_column_text(statement, 6, mech_name, sizeof(mech_name)) <
            0 ||
        btech_special_column_text(statement, 7, mech_type, sizeof(mech_type)) <
            0 ||
        btech_special_column_text(statement, 8, unit_era, sizeof(unit_era)) <
            0 ||
        btech_special_column_text(statement, 9, unit_tro, sizeof(unit_tro)) <
            0 ||
        btech_special_column_int(statement, 10, &unit_class) < 0 ||
        btech_special_column_int(statement, 11, &movement_type) < 0 ||
        btech_special_column_int(statement, 12, &tactical_range) < 0 ||
        btech_special_column_int(statement, 13, &lrs_range) < 0 ||
        btech_special_column_int(statement, 14, &scan_range) < 0 ||
        btech_special_column_int(statement, 15, &heat_sinks) < 0 ||
        btech_special_column_int(statement, 16, &heat_sink_override) < 0 ||
        btech_special_column_int(statement, 17, &computer) < 0 ||
        btech_special_column_int(statement, 18, &radio) < 0 ||
        btech_special_column_int(statement, 19, &radio_info) < 0 ||
        btech_special_column_int(statement, 20, &structural_integrity) < 0 ||
        btech_special_column_int(statement, 21,
                                 &structural_integrity_original) < 0 ||
        btech_special_column_int(statement, 22, &radio_range) < 0 ||
        btech_special_column_int(statement, 23, &fuel) < 0 ||
        btech_special_column_int(statement, 24, &fuel_original) < 0 ||
        btech_special_column_int(statement, 25, &tons) < 0 ||
        btech_special_column_int(statement, 26, &walk_speed) < 0 ||
        btech_special_column_int(statement, 27, &run_speed) < 0 ||
        btech_special_column_real(statement, 28, &max_speed) < 0 ||
        btech_special_column_real(statement, 29, &template_max_speed) < 0 ||
        btech_special_column_int(statement, 30, &battle_value) < 0 ||
        btech_special_column_int(statement, 31, &cargo_space) < 0 ||
        btech_special_column_int(statement, 32, &targeting_computer) < 0 ||
        btech_special_column_int(statement, 33, &carrier_max_tons) < 0 ||
        id_0 < CHAR_MIN || id_0 > CHAR_MAX || id_1 < CHAR_MIN ||
        id_1 > CHAR_MAX || brief < CHAR_MIN || brief > CHAR_MAX ||
        unit_class < CHAR_MIN || unit_class > CHAR_MAX ||
        movement_type < CHAR_MIN || movement_type > CHAR_MAX ||
        tactical_range < CHAR_MIN || tactical_range > CHAR_MAX ||
        lrs_range < CHAR_MIN || lrs_range > CHAR_MAX || scan_range < CHAR_MIN ||
        scan_range > CHAR_MAX || heat_sinks < CHAR_MIN ||
        heat_sinks > CHAR_MAX || computer < CHAR_MIN || computer > CHAR_MAX ||
        radio < CHAR_MIN || radio > CHAR_MAX || radio_info < 0 ||
        radio_info > UCHAR_MAX || structural_integrity < CHAR_MIN ||
        structural_integrity > CHAR_MAX ||
        structural_integrity_original < CHAR_MIN ||
        structural_integrity_original > CHAR_MAX || radio_range < SHRT_MIN ||
        radio_range > SHRT_MAX || targeting_computer < CHAR_MIN ||
        targeting_computer > CHAR_MAX || carrier_max_tons < CHAR_MIN ||
        carrier_max_tons > CHAR_MAX ||
        (map_dbref != NOTHING && !btech_context_get_map(context, map_dbref))) {
      result = -1;
      break;
    }
    mech_persistence_snapshot_export(mech, &snapshot);
    snapshot.id[0] = (char)id_0;
    snapshot.id[1] = (char)id_1;
    snapshot.brief = (char)brief;
    snapshot.map_number = map_number;
    snapshot.map_dbref = map_dbref;
    memcpy(snapshot.definition.mech_name, mech_name, sizeof(mech_name));
    memcpy(snapshot.definition.mech_type, mech_type, sizeof(mech_type));
    memcpy(snapshot.definition.unit_era, unit_era, sizeof(unit_era));
    memcpy(snapshot.definition.unit_tro, unit_tro, sizeof(unit_tro));
    snapshot.definition.type = (char)unit_class;
    snapshot.definition.move = (char)movement_type;
    snapshot.definition.tac_range = (char)tactical_range;
    snapshot.definition.lrs_range = (char)lrs_range;
    snapshot.definition.scan_range = (char)scan_range;
    snapshot.definition.numsinks = (char)heat_sinks;
    snapshot.definition.hsengoverride = heat_sink_override;
    snapshot.definition.computer = (char)computer;
    snapshot.definition.radio = (char)radio;
    snapshot.definition.radioinfo = (unsigned char)radio_info;
    snapshot.definition.si = (char)structural_integrity;
    snapshot.definition.si_orig = (char)structural_integrity_original;
    snapshot.definition.radio_range = (short)radio_range;
    snapshot.definition.fuel = fuel;
    snapshot.definition.fuel_orig = fuel_original;
    snapshot.definition.tons = tons;
    snapshot.definition.walkspeed = walk_speed;
    snapshot.definition.runspeed = run_speed;
    snapshot.definition.maxspeed = max_speed;
    snapshot.definition.template_maxspeed = template_max_speed;
    snapshot.definition.mechbv = battle_value;
    snapshot.definition.cargospace = cargo_space;
    snapshot.definition.targcomp = (char)targeting_computer;
    snapshot.definition.carmaxton = (char)carrier_max_tons;
    mech_persistence_identity_restore(mech, &snapshot);
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore every section row in stable section-index order. */
int btech_special_load_mech_sections(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  MechPersistenceSnapshot snapshot;
  DbRef current_mech;
  DbRef mech_dbref;
  struct MechSection *section;
  int armor;
  int armor_original;
  int base_to_hit;
  int config;
  int expected_section;
  int internal;
  int internal_original;
  int rear;
  int rear_original;
  int recycle;
  int result;
  int section_index;
  int specials;
  int step;

  statement = NULL;
  current_mech = NOTHING;
  expected_section = 0;
  mech = NULL;
  result =
      SQLITE3_PREPARE_V2(
          sqlite,
          "SELECT mech_dbref, section, armor, internal, rear, armor_original, "
          "internal_original, rear_original, base_to_hit, config, recycle, "
          "specials "
          "FROM btech_mech_sections ORDER BY mech_dbref, section;",
          -1, &statement, NULL) == SQLITE_OK
          ? 0
          : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &section_index) < 0 ||
        btech_special_column_int(statement, 2, &armor) < 0 ||
        btech_special_column_int(statement, 3, &internal) < 0 ||
        btech_special_column_int(statement, 4, &rear) < 0 ||
        btech_special_column_int(statement, 5, &armor_original) < 0 ||
        btech_special_column_int(statement, 6, &internal_original) < 0 ||
        btech_special_column_int(statement, 7, &rear_original) < 0 ||
        btech_special_column_int(statement, 8, &base_to_hit) < 0 ||
        btech_special_column_int(statement, 9, &config) < 0 ||
        btech_special_column_int(statement, 10, &recycle) < 0 ||
        btech_special_column_int(statement, 11, &specials) < 0 || armor < 0 ||
        armor > UCHAR_MAX || internal < 0 || internal > UCHAR_MAX || rear < 0 ||
        rear > UCHAR_MAX || armor_original < 0 || armor_original > UCHAR_MAX ||
        internal_original < 0 || internal_original > UCHAR_MAX ||
        rear_original < 0 || rear_original > UCHAR_MAX ||
        base_to_hit < CHAR_MIN || base_to_hit > CHAR_MAX || config < CHAR_MIN ||
        config > CHAR_MAX || recycle < CHAR_MIN || recycle > CHAR_MAX ||
        specials < 0 || specials > USHRT_MAX) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && expected_section != NUM_SECTIONS) {
        result = -1;
        break;
      }
      mech = btech_context_get_mech(context, mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      expected_section = 0;
      mech_persistence_snapshot_export(mech, &snapshot);
    }
    if (section_index != expected_section || section_index >= NUM_SECTIONS) {
      result = -1;
      break;
    }
    section = snapshot_section(&snapshot, section_index);
    section->armor = (unsigned char)armor;
    section->internal = (unsigned char)internal;
    section->rear = (unsigned char)rear;
    section->armor_orig = (unsigned char)armor_original;
    section->internal_orig = (unsigned char)internal_original;
    section->rear_orig = (unsigned char)rear_original;
    section->basetohit = (char)base_to_hit;
    section->config = (char)config;
    section->recycle = (char)recycle;
    section->specials = (unsigned short)specials;
    mech_persistence_section_restore(mech, section_index, section);
    expected_section++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech && expected_section != NUM_SECTIONS)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore all twelve critical slots per section without restoring pointers. */
int btech_special_load_mech_criticals(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement;
  Mech *mech;
  MechPersistenceSnapshot snapshot;
  DbRef current_mech;
  DbRef mech_dbref;
  struct CriticalSlot *critical;
  unsigned int ammo_mode;
  int brand;
  int current_section;
  int data;
  unsigned int damage_flags;
  int desired_ammo_location;
  int expected_slot;
  unsigned int fire_mode;
  int item_type;
  int result;
  int section_index;
  int slot;
  int step;

  statement = NULL;
  current_mech = NOTHING;
  current_section = -1;
  expected_slot = 0;
  mech = NULL;
  result = SQLITE3_PREPARE_V2(sqlite,
                              "SELECT mech_dbref, section, slot, brand, data, "
                              "item_type, fire_mode, "
                              "ammo_mode, damage_flags, desired_ammo_location "
                              "FROM btech_mech_criticals "
                              "ORDER BY mech_dbref, section, slot;",
                              -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (btech_special_column_long(statement, 0, &mech_dbref) < 0 ||
        mech_dbref == NOTHING ||
        btech_special_column_int(statement, 1, &section_index) < 0 ||
        btech_special_column_int(statement, 2, &slot) < 0 ||
        btech_special_column_int(statement, 3, &brand) < 0 ||
        btech_special_column_int(statement, 4, &data) < 0 ||
        btech_special_column_int(statement, 5, &item_type) < 0 ||
        btech_special_column_uint(statement, 6, &fire_mode) < 0 ||
        btech_special_column_uint(statement, 7, &ammo_mode) < 0 ||
        btech_special_column_uint(statement, 8, &damage_flags) < 0 ||
        btech_special_column_int(statement, 9, &desired_ammo_location) < 0 ||
        brand < 0 || brand > UCHAR_MAX || data < 0 || data > UCHAR_MAX ||
        item_type < 0 || item_type > USHRT_MAX ||
        desired_ammo_location < SHRT_MIN || desired_ammo_location > SHRT_MAX) {
      result = -1;
      break;
    }
    if (mech_dbref != current_mech) {
      if (mech && (current_section != NUM_SECTIONS - 1 ||
                   expected_slot != NUM_CRITICALS)) {
        result = -1;
        break;
      }
      mech = btech_context_get_mech(context, mech_dbref);
      if (!mech) {
        result = -1;
        break;
      }
      current_mech = mech_dbref;
      current_section = 0;
      expected_slot = 0;
      mech_persistence_snapshot_export(mech, &snapshot);
    }
    if (section_index != current_section) {
      if (expected_slot != NUM_CRITICALS ||
          section_index != current_section + 1) {
        result = -1;
        break;
      }
      current_section = section_index;
      expected_slot = 0;
    }
    if (section_index < 0 || section_index >= NUM_SECTIONS ||
        slot != expected_slot || slot >= NUM_CRITICALS) {
      result = -1;
      break;
    }
    critical = snapshot_critical(
        &snapshot,
        (CriticalSlotReference){.section = section_index, .critical = slot});
    critical->brand = (unsigned char)brand;
    critical->data = (unsigned char)data;
    critical->type = (unsigned short)item_type;
    critical->firemode = fire_mode;
    critical->ammomode = ammo_mode;
    critical->weap_damage_flags = damage_flags;
    critical->desired_ammo_loc = (short)desired_ammo_location;
    mech_persistence_critical_restore(mech, section_index, slot, critical);
    expected_slot++;
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  if (result == 0 && mech &&
      (current_section != NUM_SECTIONS - 1 || expected_slot != NUM_CRITICALS))
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

/* Restore the non-pointer MECH position record. */
