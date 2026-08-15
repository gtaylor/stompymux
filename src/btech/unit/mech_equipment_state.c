#include "equipment_types.h"
#include "mech_equipment_api.h"

#include "checked_conversion.h"
#include "mech_api_types.h"
#include "mech_internal.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"
#include <stddef.h>

static const struct MechSection *section_at(const Mech *mech, int section) {
  return checked_storage_at_const(mech->ud.sections, NUM_SECTIONS,
                                  sizeof *mech->ud.sections, (size_t)section);
}

static struct MechSection *section_at_mutable(Mech *mech, int section) {
  return checked_storage_at(mech->ud.sections, NUM_SECTIONS,
                            sizeof *mech->ud.sections, (size_t)section);
}

static const struct CriticalSlot *critical_at(const Mech *mech,
                                              CriticalSlotReference reference) {
  const struct MechSection *section_storage =
      section_at(mech, reference.section);
  return checked_storage_at_const(section_storage->criticals, NUM_CRITICALS,
                                  sizeof *section_storage->criticals,
                                  (size_t)reference.critical);
}

static struct CriticalSlot *
critical_at_mutable(Mech *mech, CriticalSlotReference reference) {
  struct MechSection *section_storage =
      section_at_mutable(mech, reference.section);
  return checked_storage_at(section_storage->criticals, NUM_CRITICALS,
                            sizeof *section_storage->criticals,
                            (size_t)reference.critical);
}

int mech_critical_part_type(const Mech *mech, int section, int critical) {
  return critical_at(mech, (CriticalSlotReference){section, critical})->type;
}

int mech_critical_brand(const Mech *mech, int section, int critical) {
  return critical_at(mech, (CriticalSlotReference){section, critical})->brand %
         16;
}

void mech_critical_brand_set(const CriticalSlotBrandSet *request) {
  struct CriticalSlot *slot = critical_at_mutable(request->mech, request->slot);
  int failure_bits = slot->brand & 0xF0;
  int brand = request->brand;
  int clamped_brand = brand;
  if (brand < 0)
    clamped_brand = 0;
  else if (brand > 15)
    clamped_brand = 15;
  slot->brand = clamp_int_to_unsigned_char(failure_bits | clamped_brand);
}

int mech_critical_data(const Mech *mech, int section, int critical) {
  return critical_at(mech, (CriticalSlotReference){section, critical})->data;
}

int mech_critical_fire_mode(const Mech *mech, int section, int critical) {
  return clamp_unsigned_int_to_int(
      critical_at(mech, (CriticalSlotReference){section, critical})->firemode);
}

int mech_critical_ammo_mode(const Mech *mech, int section, int critical) {
  return clamp_unsigned_int_to_int(
      critical_at(mech, (CriticalSlotReference){section, critical})->ammomode);
}

void mech_critical_fire_mode_set(Mech *mech, int section, int critical,
                                 int modes) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})
      ->firemode = clamp_int_to_unsigned_int(modes);
}

void mech_critical_ammo_mode_set(Mech *mech, int section, int critical,
                                 int modes) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})
      ->ammomode = clamp_int_to_unsigned_int(modes);
}

int mech_critical_damage_flags(const Mech *mech, int section, int critical) {
  return clamp_unsigned_int_to_int(
      critical_at(mech, (CriticalSlotReference){section, critical})
          ->weap_damage_flags);
}

void mech_critical_damage_flags_set(Mech *mech, int section, int critical,
                                    int flags) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})
      ->weap_damage_flags = clamp_int_to_unsigned_int(flags);
}

int mech_critical_desired_ammo_section(const Mech *mech, int section,
                                       int critical) {
  return critical_at(mech, (CriticalSlotReference){section, critical})
      ->desired_ammo_loc;
}

void mech_critical_desired_ammo_section_set(Mech *mech, int section,
                                            int critical, int ammo_section) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})
      ->desired_ammo_loc = clamp_int_to_short(ammo_section);
}

int mech_critical_temporary_failure(const Mech *mech, int section,
                                    int critical) {
  return critical_at(mech, (CriticalSlotReference){section, critical})->brand >>
         4;
}

int mech_critical_full_ammunition(const Mech *mech, int section, int critical) {
  return full_ammo(mech, section, critical);
}

float mech_ammunition_slot_multiplier(const Mech *mech, int section,
                                      int critical) {
  int part = mech_critical_part_type(mech, section, critical);
  int fire_mode = mech_critical_fire_mode(mech, section, critical);
  int ammo_mode = mech_critical_ammo_mode(mech, section, critical);
  if (!equipment_is_ammunition(part) || (fire_mode & HALFTON_MODE) ||
      (ammo_mode & (AC_AP_MODE | AC_PRECISION_MODE)))
    return 1.0F;
  return ammo_mode & AC_CASELESS_MODE ? 0.5F : 2.0F;
}

bool mech_critical_is_disabled(const Mech *mech, int section, int critical) {
  return (mech_critical_fire_mode(mech, section, critical) & DISABLED_MODE) !=
         0;
}

bool mech_critical_is_destroyed(const Mech *mech, int section, int critical) {
  return (mech_critical_fire_mode(mech, section, critical) & DESTROYED_MODE) !=
         0;
}

bool mech_critical_is_broken(const Mech *mech, int section, int critical) {
  return (mech_critical_fire_mode(mech, section, critical) &
          (DESTROYED_MODE | BROKEN_MODE)) != 0;
}

bool mech_critical_is_damaged(const Mech *mech, int section, int critical) {
  return (mech_critical_fire_mode(mech, section, critical) & DAMAGED_MODE) != 0;
}

bool mech_critical_is_nonfunctional(const Mech *mech, int section,
                                    int critical) {
  return (mech_critical_is_disabled(mech, section, critical) ||
          mech_critical_is_broken(mech, section, critical)) != 0;
}

void mech_critical_temporary_failure_set(
    const CriticalSlotFailureSet *request) {
  struct CriticalSlot *slot = critical_at_mutable(request->mech, request->slot);
  int failure = request->failure;
  int clamped_failure = failure;
  if (failure < 0)
    clamped_failure = 0;
  else if (failure > 15)
    clamped_failure = 15;
  slot->brand =
      clamp_int_to_unsigned_char((slot->brand & 0x0F) | (clamped_failure << 4));
}

void mech_critical_data_set(Mech *mech, int section, int critical, int data) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})->data =
      clamp_int_to_unsigned_char(data);
}

void mech_critical_fire_mode_clear(Mech *mech, int section, int critical,
                                   int modes) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})
      ->firemode &= ~clamp_int_to_unsigned_int(modes);
}

void mech_critical_fire_mode_add(Mech *mech, int section, int critical,
                                 int modes) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})
      ->firemode |= clamp_int_to_unsigned_int(modes);
}

void mech_critical_ammo_mode_clear(Mech *mech, int section, int critical,
                                   int modes) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})
      ->ammomode &= ~clamp_int_to_unsigned_int(modes);
}

void mech_critical_ammo_mode_add(Mech *mech, int section, int critical,
                                 int modes) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})
      ->ammomode |= clamp_int_to_unsigned_int(modes);
}

void mech_critical_damage_flags_add(Mech *mech, int section, int critical,
                                    int flags) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})
      ->weap_damage_flags |= clamp_int_to_unsigned_int(flags);
}

void mech_critical_damage_repair(Mech *mech, int section, int critical) {
  struct CriticalSlot *slot =
      critical_at_mutable(mech, (CriticalSlotReference){section, critical});
  slot->firemode &= ~clamp_int_to_unsigned_int(DAMAGED_MODE);
  slot->weap_damage_flags = 0;
  slot->brand %= 16;
}

void mech_critical_part_type_set(Mech *mech, int section, int critical,
                                 int part_type) {
  critical_at_mutable(mech, (CriticalSlotReference){section, critical})->type =
      clamp_int_to_unsigned_short(part_type);
}

void mech_critical_destroyed_set(Mech *mech, int section, int critical,
                                 bool destroyed) {
  struct CriticalSlot *slot =
      critical_at_mutable(mech, (CriticalSlotReference){section, critical});
  if (destroyed)
    slot->firemode |= clamp_int_to_unsigned_int(DESTROYED_MODE);
  else
    slot->firemode &= ~clamp_int_to_unsigned_int(DESTROYED_MODE);
}

void mech_critical_destroy(Mech *mech, int section, int critical) {
  struct CriticalSlot *slot =
      critical_at_mutable(mech, (CriticalSlotReference){section, critical});
  slot->firemode |= clamp_int_to_unsigned_int(DESTROYED_MODE);
  slot->firemode &=
      ~clamp_int_to_unsigned_int(BROKEN_MODE | DISABLED_MODE | DAMAGED_MODE);
  slot->weap_damage_flags = 0;
  slot->brand %= 16;
}

void mech_critical_restore(Mech *mech, int section, int critical) {
  struct CriticalSlot *slot =
      critical_at_mutable(mech, (CriticalSlotReference){section, critical});
  slot->firemode &=
      ~clamp_int_to_unsigned_int(DESTROYED_MODE | HOTLOAD_MODE | DISABLED_MODE |
                                 BROKEN_MODE | DAMAGED_MODE);
  slot->weap_damage_flags = 0;
  slot->brand %= 16;
}

void mech_critical_jettison(Mech *mech, int section, int critical) {
  struct CriticalSlot *slot =
      critical_at_mutable(mech, (CriticalSlotReference){section, critical});
  slot->firemode |=
      clamp_int_to_unsigned_int(DESTROYED_MODE | IS_JETTISONED_MODE);
  slot->firemode &= ~clamp_int_to_unsigned_int(BROKEN_MODE | DISABLED_MODE);
}

int mech_section_original_armor(const Mech *mech, int section) {
  return section_at(mech, section)->armor_orig;
}

int mech_section_original_rear_armor(const Mech *mech, int section) {
  return section_at(mech, section)->rear_orig;
}

int mech_section_armor(const Mech *mech, int section) {
  return section_at(mech, section)->armor;
}

int mech_section_rear_armor(const Mech *mech, int section) {
  return section_at(mech, section)->rear;
}

int mech_section_internal(const Mech *mech, int section) {
  return section_at(mech, section)->internal;
}

int mech_section_original_internal(const Mech *mech, int section) {
  return section_at(mech, section)->internal_orig;
}

bool mech_section_is_destroyed(const Mech *mech, int section) {
  int unit_class = (unsigned char)mech->ud.type;
  bool is_dropship =
      (unit_class == CLASS_DS || unit_class == CLASS_SPHEROID_DS) != 0;
  bool is_aerospace = (unit_class == CLASS_AERO || is_dropship) != 0;
  const struct MechSection *section_storage = section_at(mech, section);
  return (section_storage->armor == 0 &&
          (is_aerospace || section_storage->internal == 0) && !is_dropship) !=
         0;
}

bool mech_section_is_flooded(const Mech *mech, int section) {
  return (section_at(mech, section)->config & SECTION_FLOODED) != 0;
}

bool mech_section_is_breached(const Mech *mech, int section) {
  return (section_at(mech, section)->config & SECTION_BREACHED) != 0;
}

void mech_section_flooded_set(Mech *mech, int section, bool flooded) {
  if (flooded)
    section_at_mutable(mech, section)->config |= SECTION_FLOODED;
  else
    section_at_mutable(mech, section)->config &= ~SECTION_FLOODED;
}

void mech_section_breached_set(Mech *mech, int section, bool breached) {
  if (breached)
    section_at_mutable(mech, section)->config |= SECTION_BREACHED;
  else
    section_at_mutable(mech, section)->config &= ~SECTION_BREACHED;
}

bool mech_critical_is_operational_special(const CriticalSpecialCheck *check) {
  const struct CriticalSlot *slot = critical_at(check->mech, check->slot);
  return (slot->type == special_equipment_index(check->special) &&
          !(slot->firemode & (DISABLED_MODE | DESTROYED_MODE | BROKEN_MODE))) !=
         0;
}

bool mech_section_carries_club(const Mech *mech, int section) {
  return (section_at(mech, section)->specials & CARRYING_CLUB) != 0;
}

bool mech_section_has_special(const Mech *mech, int section, int special) {
  return (section_at(mech, section)->specials & special) != 0;
}

int mech_section_specials(const Mech *mech, int section) {
  return section_at(mech, section)->specials;
}

void mech_section_specials_set(Mech *mech, int section, int specials) {
  section_at_mutable(mech, section)->specials =
      clamp_int_to_unsigned_short(specials);
}

bool mech_section_configuration_has(const Mech *mech, int section,
                                    int configuration) {
  return (section_at(mech, section)->config & configuration) != 0;
}

int mech_section_configuration(const Mech *mech, int section) {
  return section_at(mech, section)->config;
}

void mech_section_configuration_set(Mech *mech, int section,
                                    int configuration) {
  section_at_mutable(mech, section)->config = clamp_int_to_char(configuration);
}

void mech_section_configuration_add(Mech *mech, int section,
                                    int configuration) {
  section_at_mutable(mech, section)->config |= configuration;
}

void mech_section_configuration_remove(Mech *mech, int section,
                                       int configuration) {
  section_at_mutable(mech, section)->config &= ~configuration;
}

bool mech_has_section_special(const Mech *mech, int special) {
  for (int section = 0; section < NUM_SECTIONS; section++)
    if (mech_section_has_special(mech, section, special))
      return true;
  return false;
}

void mech_section_special_add(Mech *mech, int section, int special) {
  section_at_mutable(mech, section)->specials |= special;
}

void mech_section_special_remove(Mech *mech, int section, int special) {
  section_at_mutable(mech, section)->specials &= ~special;
}

void mech_section_specials_clear(Mech *mech, int section) {
  section_at_mutable(mech, section)->specials = 0;
}

bool mech_has_attached_inarc_ecm(const Mech *mech) {
  for (int section = 0; section < NUM_SECTIONS; section++)
    if (section_at(mech, section)->internal &&
        (section_at(mech, section)->specials & INARC_ECM_ATTACHED))
      return true;
  return false;
}

bool mech_has_attached_homing_beacon(const Mech *mech) {
  for (int section = 0; section < NUM_SECTIONS; section++)
    if (section_at(mech, section)->specials &
        (NARC_ATTACHED | INARC_HOMING_ATTACHED))
      return true;
  return false;
}

bool mech_limbs_are_recycling(const Mech *mech) {
  return (section_at(mech, RARM)->recycle || section_at(mech, LARM)->recycle ||
          section_at(mech, RLEG)->recycle || section_at(mech, LLEG)->recycle) !=
         0;
}

bool mech_weapon_is_recycling_at(const Mech *mech, int section, int critical) {
  return (mech_critical_data(mech, section, critical) > 0 &&
          equipment_is_weapon(
              mech_critical_part_type(mech, section, critical)) &&
          !mech_critical_is_nonfunctional(mech, section, critical) &&
          !mech_section_is_destroyed(mech, section)) != 0;
}

bool mech_section_has_recycling_weapon(Mech *mech, int section) {
  return sect_has_busy_weap(mech, section);
}

bool mech_weapon_is_nonfunctional_at(Mech *mech, int section, int critical,
                                     int weapon_index) {
  return weapon_is_nonfunctional(mech, section, critical,
                                 get_weapon_crits(mech, weapon_index)) > 0;
}

int mech_section_recycle_ticks(const Mech *mech, int section) {
  return section_at(mech, section)->recycle;
}

void mech_section_recycle_ticks_set(Mech *mech, int section, int ticks) {
  section_at_mutable(mech, section)->recycle = clamp_int_to_char(ticks);
}

int mech_last_weapon_recycle_tick(const Mech *mech) {
  return clamp_intptr_to_int(mech->rd.last_weapon_recycle);
}

void mech_last_weapon_recycle_tick_set(Mech *mech, int tick) {
  mech->rd.last_weapon_recycle = tick;
}

int mech_section_base_to_hit(const Mech *mech, int section) {
  return section_at(mech, section)->basetohit;
}

void mech_section_base_to_hit_set(Mech *mech, int section, int modifier) {
  section_at_mutable(mech, section)->basetohit = clamp_int_to_char(modifier);
}

void mech_section_base_to_hit_add(Mech *mech, int section, int modifier) {
  section_at_mutable(mech, section)->basetohit += modifier;
}

int mech_section_critical_count(Mech *mech, int section) {
  return crits_in_loc(mech, section);
}

bool mech_part_is_structural_placeholder(int part_type) {
  return (part_type == special_equipment_index(ENDO_STEEL) ||
          part_type == special_equipment_index(FERRO_FIBROUS) ||
          part_type == special_equipment_index(TRIPLE_STRENGTH_MYOMER) ||
          part_type == special_equipment_index(STEALTH_ARMOR) ||
          part_type == special_equipment_index(HVY_FERRO_FIBROUS) ||
          part_type == special_equipment_index(LT_FERRO_FIBROUS)) != 0;
}

void mech_section_armor_set(Mech *mech, int section, int armor) {
  section_at_mutable(mech, section)->armor = clamp_int_to_unsigned_char(armor);
  mech->rd.critstatus &= ~OWEIGHT_OK;
}

void mech_section_rear_armor_set(Mech *mech, int section, int armor) {
  section_at_mutable(mech, section)->rear = clamp_int_to_unsigned_char(armor);
  mech->rd.critstatus &= ~OWEIGHT_OK;
}

void mech_section_original_armor_set(Mech *mech, int section, int armor) {
  section_at_mutable(mech, section)->armor_orig =
      clamp_int_to_unsigned_char(armor);
}

void mech_section_original_rear_armor_set(Mech *mech, int section, int armor) {
  section_at_mutable(mech, section)->rear_orig =
      clamp_int_to_unsigned_char(armor);
}

void mech_section_internal_set(Mech *mech, int section, int internal) {
  section_at_mutable(mech, section)->internal =
      clamp_int_to_unsigned_char(internal);
  mech->rd.critstatus &= ~OWEIGHT_OK;
}

void mech_section_original_internal_set(Mech *mech, int section, int internal) {
  section_at_mutable(mech, section)->internal_orig =
      clamp_int_to_unsigned_char(internal);
}

void mech_critical_configure(const CriticalSlotConfiguration *configuration) {
  struct CriticalSlot *slot =
      critical_at_mutable(configuration->mech, configuration->slot);
  slot->type = clamp_int_to_unsigned_short(configuration->part_type);
  slot->data = clamp_int_to_unsigned_char(configuration->data);
  slot->firemode = clamp_int_to_unsigned_int(configuration->fire_mode);
  slot->ammomode = clamp_int_to_unsigned_int(configuration->ammo_mode);
}
