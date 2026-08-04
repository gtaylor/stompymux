#include "mech_equipment_api.h"

#include "mech_internal.h"

int mech_critical_part_type(const Mech *mech, int section, int critical) {
  return mech->ud.sections[section].criticals[critical].type;
}

int mech_section_original_armor(const Mech *mech, int section) {
  return mech->ud.sections[section].armor_orig;
}

int mech_section_original_rear_armor(const Mech *mech, int section) {
  return mech->ud.sections[section].rear_orig;
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
