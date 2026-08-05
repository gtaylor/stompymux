#include "mech_equipment_api.h"

#include "mech_internal.h"
#include "mech_macros.h"
#include "mech_utils_api.h"

int mech_critical_part_type(const Mech *mech, int section, int critical) {
  return mech->ud.sections[section].criticals[critical].type;
}

int mech_critical_brand(const Mech *mech, int section, int critical) {
  return mech->ud.sections[section].criticals[critical].brand % 16;
}

int mech_critical_data(const Mech *mech, int section, int critical) {
  return mech->ud.sections[section].criticals[critical].data;
}

int mech_critical_fire_mode(const Mech *mech, int section, int critical) {
  return mech->ud.sections[section].criticals[critical].firemode;
}

int mech_critical_ammo_mode(const Mech *mech, int section, int critical) {
  return mech->ud.sections[section].criticals[critical].ammomode;
}

int mech_critical_temporary_failure(const Mech *mech, int section,
                                    int critical) {
  return mech->ud.sections[section].criticals[critical].brand >> 4;
}

int mech_critical_full_ammunition(const Mech *mech, int section, int critical) {
  return FullAmmo((Mech *)mech, section, critical);
}

bool mech_critical_is_disabled(const Mech *mech, int section, int critical) {
  return mech_critical_fire_mode(mech, section, critical) & DISABLED_MODE;
}

bool mech_critical_is_destroyed(const Mech *mech, int section, int critical) {
  return mech_critical_fire_mode(mech, section, critical) & DESTROYED_MODE;
}

bool mech_critical_is_broken(const Mech *mech, int section, int critical) {
  return mech_critical_fire_mode(mech, section, critical) &
         (DESTROYED_MODE | BROKEN_MODE);
}

bool mech_critical_is_damaged(const Mech *mech, int section, int critical) {
  return mech_critical_fire_mode(mech, section, critical) & DAMAGED_MODE;
}

bool mech_critical_is_nonfunctional(const Mech *mech, int section,
                                    int critical) {
  return mech_critical_is_disabled(mech, section, critical) ||
         mech_critical_is_broken(mech, section, critical);
}

void mech_critical_temporary_failure_set(Mech *mech, int section, int critical,
                                         int failure) {
  struct CriticalSlot *slot = &mech->ud.sections[section].criticals[critical];
  slot->brand = (slot->brand % 16) + (failure << 4);
}

void mech_critical_data_set(Mech *mech, int section, int critical, int data) {
  mech->ud.sections[section].criticals[critical].data = data;
}

void mech_critical_part_type_set(Mech *mech, int section, int critical,
                                 int part_type) {
  mech->ud.sections[section].criticals[critical].type = part_type;
}

void mech_critical_destroyed_set(Mech *mech, int section, int critical,
                                 bool destroyed) {
  struct CriticalSlot *slot = &mech->ud.sections[section].criticals[critical];
  if (destroyed)
    slot->firemode |= DESTROYED_MODE;
  else
    slot->firemode &= ~DESTROYED_MODE;
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

bool mech_section_is_flooded(const Mech *mech, int section) {
  return mech->ud.sections[section].config & SECTION_FLOODED;
}

bool mech_critical_is_operational_special(const Mech *mech, int section,
                                          int critical, int special) {
  const struct CriticalSlot *slot =
      &mech->ud.sections[section].criticals[critical];
  return slot->type == I2Special(special) &&
         !(slot->firemode & (DISABLED_MODE | DESTROYED_MODE | BROKEN_MODE));
}

bool mech_section_carries_club(const Mech *mech, int section) {
  return mech->ud.sections[section].specials & CARRYING_CLUB;
}

bool mech_has_attached_inarc_ecm(const Mech *mech) {
  for (int section = 0; section < NUM_SECTIONS; section++)
    if (mech->ud.sections[section].internal &&
        (mech->ud.sections[section].specials & INARC_ECM_ATTACHED))
      return true;
  return false;
}

bool mech_has_attached_homing_beacon(const Mech *mech) {
  for (int section = 0; section < NUM_SECTIONS; section++)
    if (mech->ud.sections[section].specials &
        (NARC_ATTACHED | INARC_HOMING_ATTACHED))
      return true;
  return false;
}

bool mech_limbs_are_recycling(const Mech *mech) {
  return mech->ud.sections[RARM].recycle || mech->ud.sections[LARM].recycle ||
         mech->ud.sections[RLEG].recycle || mech->ud.sections[LLEG].recycle;
}

bool mech_weapon_is_recycling_at(const Mech *mech, int section, int critical) {
  return mech_critical_data(mech, section, critical) > 0 &&
         IsWeapon(mech_critical_part_type(mech, section, critical)) &&
         !mech_critical_is_nonfunctional(mech, section, critical) &&
         !mech_section_is_destroyed(mech, section);
}

bool mech_weapon_is_nonfunctional_at(Mech *mech, int section, int critical,
                                     int weapon_index) {
  return WeaponIsNonfunctional(mech, section, critical,
                               GetWeaponCrits(mech, weapon_index)) > 0;
}

int mech_section_recycle_ticks(const Mech *mech, int section) {
  return mech->ud.sections[section].recycle;
}

bool mech_part_is_structural_placeholder(int part_type) {
  return part_type == I2Special(ENDO_STEEL) ||
         part_type == I2Special(FERRO_FIBROUS) ||
         part_type == I2Special(TRIPLE_STRENGTH_MYOMER) ||
         part_type == I2Special(STEALTH_ARMOR) ||
         part_type == I2Special(HVY_FERRO_FIBROUS) ||
         part_type == I2Special(LT_FERRO_FIBROUS);
}

void mech_section_armor_set(Mech *mech, int section, int armor) {
  mech->ud.sections[section].armor = armor;
}

void mech_section_rear_armor_set(Mech *mech, int section, int armor) {
  mech->ud.sections[section].rear = armor;
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
