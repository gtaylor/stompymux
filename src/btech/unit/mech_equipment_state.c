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
