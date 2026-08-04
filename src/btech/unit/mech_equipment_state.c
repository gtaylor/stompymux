#include "mech_equipment_api.h"

#include "mech_internal.h"

int mech_critical_part_type(const Mech *mech, int section, int critical) {
  return mech->ud.sections[section].criticals[critical].type;
}

int mech_critical_brand(const Mech *mech, int section, int critical) {
  return mech->ud.sections[section].criticals[critical].brand % 16;
}

void mech_critical_temporary_failure_set(Mech *mech, int section, int critical,
                                         int failure) {
  struct CriticalSlot *slot = &mech->ud.sections[section].criticals[critical];
  slot->brand = (slot->brand % 16) + (failure << 4);
}

int mech_section_original_armor(const Mech *mech, int section) {
  return mech->ud.sections[section].armor_orig;
}

int mech_section_original_rear_armor(const Mech *mech, int section) {
  return mech->ud.sections[section].rear_orig;
}

int mech_section_armor(const Mech *mech, int section) {
  return mech->ud.sections[section].armor;
}

int mech_section_rear_armor(const Mech *mech, int section) {
  return mech->ud.sections[section].rear;
}

int mech_section_internal(const Mech *mech, int section) {
  return mech->ud.sections[section].internal;
}

int mech_section_original_internal(const Mech *mech, int section) {
  return mech->ud.sections[section].internal_orig;
}

bool mech_section_is_destroyed(const Mech *mech, int section) {
  int unit_class = mech->ud.type;
  bool is_dropship = unit_class == CLASS_DS || unit_class == CLASS_SPHEROID_DS;
  bool is_aerospace = unit_class == CLASS_AERO || is_dropship;
  return mech->ud.sections[section].armor == 0 &&
         (is_aerospace || mech->ud.sections[section].internal == 0) &&
         !is_dropship;
}

bool mech_critical_is_operational_special(const Mech *mech, int section,
                                          int critical, int special) {
  const struct CriticalSlot *slot =
      &mech->ud.sections[section].criticals[critical];
  return slot->type == I2Special(special) &&
         !(slot->firemode & (DISABLED_MODE | DESTROYED_MODE | BROKEN_MODE));
}

void mech_section_armor_set(Mech *mech, int section, int armor) {
  mech->ud.sections[section].armor = armor;
}

void mech_section_original_armor_set(Mech *mech, int section, int armor) {
  mech->ud.sections[section].armor_orig = armor;
}

void mech_section_internal_set(Mech *mech, int section, int internal) {
  mech->ud.sections[section].internal = internal;
}

void mech_section_original_internal_set(Mech *mech, int section, int internal) {
  mech->ud.sections[section].internal_orig = internal;
}

void mech_critical_configure(Mech *mech, int section, int critical,
                             int part_type, int data, int fire_mode,
                             int ammo_mode) {
  struct CriticalSlot *slot = &mech->ud.sections[section].criticals[critical];
  slot->type = part_type;
  slot->data = data;
  slot->firemode = fire_mode;
  slot->ammomode = ammo_mode;
}
