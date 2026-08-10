#include "btech/context.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_equipment_api.h"
#include "mech_utils_api.h"
#include "mech_utils_internal.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include <stddef.h>

int CountAmmoForWeapon(Mech *mech, int weapindx) {
  int wSecIter;
  int wSlotIter;
  int wcAmmo = 0;
  int wAmmoIdx;

  wAmmoIdx = ammunition_equipment_index(weapindx);

  for (wSecIter = 0; wSecIter < NUM_SECTIONS; wSecIter++) {
    for (wSlotIter = 0; wSlotIter < NUM_CRITICALS; wSlotIter++) {
      if ((mech_critical_part_type(mech, wSecIter, wSlotIter) == wAmmoIdx) &&
          !mech_critical_is_nonfunctional(mech, wSecIter, wSlotIter) &&
          (mech_critical_data(mech, wSecIter, wSlotIter) > 0))
        wcAmmo += mech_critical_data(mech, wSecIter, wSlotIter);
    }
  }

  return wcAmmo;
}

/* Function taken from 3065. Credit to RebelST) */
int FindArtemisForWeapon(Mech *mech, int section, int critical) {
  int critloop;
  int desired;

  desired = special_equipment_index(ARTEMIS_IV);
  for (critloop = 0; critloop < NUM_CRITICALS; critloop++) {
    if (mech_critical_part_type(mech, section, critloop) == desired &&
        !mech_critical_is_nonfunctional(mech, section, critloop)) {
      if (mech_critical_data(mech, section, critloop) == (critical + 1))
        return 1;
    }
  }
  if (((mech)->ud.type) == CLASS_MECH &&
      section == CTORSO) { // if it's mech, and torso missile, search in head
    for (critloop = 0; critloop < 6; critloop++) {
      if (mech_critical_part_type(mech, HEAD, critloop) == desired &&
          !mech_critical_is_nonfunctional(mech, HEAD, critloop)) {
        if (mech_critical_data(mech, HEAD, critloop) == (critical + 1))
          return 1;
      }
    }
  } else if (((mech)->ud.type) == CLASS_VEH_GROUND &&
             section == TURRET) { // same thing for turret & aft
    for (critloop = 0; critloop < NUM_CRITICALS; critloop++) {
      if (mech_critical_part_type(mech, BSIDE, critloop) == desired &&
          !mech_critical_is_nonfunctional(mech, BSIDE, critloop)) {
        if (mech_critical_data(mech, BSIDE, critloop) == (critical + 1))
          return 1;
      }
    }
  }
  return 0;
}

int ReverseSplitCritLoc(Mech *mech, int sect, int crit) {
  if (((mech)->ud.type) != CLASS_MECH)
    return -1;

  switch (sect) {
  case LARM:
  case LLEG:
    return LTORSO;
  case RARM:
  case RLEG:
    return RTORSO;
  case RTORSO:
    return RARM;
  case LTORSO:
    return LARM;
  case CTORSO:
    return (special_from_equipment_index(
                mech_critical_part_type(mech, sect, crit)) == SPLIT_CRIT_RIGHT
                ? RTORSO
                : LTORSO);
  }
  return -1;
}

int FindSplitCrits(Mech *mech, int sect, int type, int crit) {
  int i;

  for (i = 0; i < CritsInLoc(mech, sect); i++)
    if (mech_critical_part_type(mech, sect, i) == type &&
        mech_critical_data(mech, sect, i) == crit)
      return i;

  return -1;
}
SplitCriticalLookup split_critical_find(Mech *mech,
                                        CriticalSlotReference source) {
  int sect = source.section;
  int data = source.critical;
  SplitCriticalLookup result = {0};
  switch (sect) {
  case RARM: // right arm goes to right torso
    result.part_type = special_equipment_index(SPLIT_CRIT_RIGHT);
    result.slot.critical = FindSplitCrits(mech, RTORSO, result.part_type, data);
    if (result.slot.critical >= 0) {
      result.found = true;
      result.slot.section = RTORSO;
      return result;
    }
    break;
  case LARM: // left arm goes to left torso
    result.part_type = special_equipment_index(SPLIT_CRIT_LEFT);
    result.slot.critical = FindSplitCrits(mech, LTORSO, result.part_type, data);
    if (result.slot.critical >= 0) {
      result.found = true;
      result.slot.section = LTORSO;
      return result;
    }
    break;
  case RTORSO: // torso more complex, need to go thru arm, leg, torso
    result.part_type = special_equipment_index(SPLIT_CRIT_RIGHT);
    result.slot.critical = FindSplitCrits(mech, CTORSO, result.part_type, data);
    if (result.slot.critical >= 0) {
      result.found = true;
      result.slot.section = CTORSO;
      return result;
    }
    result.slot.critical = FindSplitCrits(mech, RARM, result.part_type, data);
    if (result.slot.critical >= 0) {
      result.found = true;
      result.slot.section = RARM;
      return result;
    }
    result.slot.critical = FindSplitCrits(mech, RLEG, result.part_type, data);
    if (result.slot.critical >= 0) {
      result.found = true;
      result.slot.section = RLEG;
      return result;
    }
    break;
  case LTORSO: // same for left torso
    result.part_type = special_equipment_index(SPLIT_CRIT_LEFT);
    result.slot.critical = FindSplitCrits(mech, CTORSO, result.part_type, data);
    if (result.slot.critical >= 0) {
      result.found = true;
      result.slot.section = CTORSO;
      return result;
    }
    result.slot.critical = FindSplitCrits(mech, LARM, result.part_type, data);
    if (result.slot.critical >= 0) {
      result.found = true;
      result.slot.section = LARM;
      return result;
    }
    result.slot.critical = FindSplitCrits(mech, LLEG, result.part_type, data);
    if (result.slot.critical >= 0) {
      result.found = true;
      result.slot.section = LLEG;
      return result;
    }
    break;
  }
  return result;
}

AmmunitionHazardResult destructive_ammunition_find(Mech *mech) {
  AmmunitionHazardResult result = {0};
  int loop;
  int critloop;
  int maxdamage = 0;
  int damage;
  [[maybe_unused]] int weapindx;
  int type, data;

  for (loop = 0; loop < NUM_SECTIONS; loop++)
    for (critloop = 0; critloop < NUM_CRITICALS; critloop++)
      if (equipment_is_ammunition(
              mech_critical_part_type(mech, loop, critloop)) &&
          !mech_critical_is_destroyed(mech, loop, critloop)) {
        data = mech_critical_data(mech, loop, critloop);
        type = mech_critical_part_type(mech, loop, critloop);
        weapindx = ammunition_to_weapon_index(type);
        damage = data * weapon_catalogue_damage(weapindx);
        if (weapon_catalogue_has_special(weapindx, GAUSS))
          continue;
        if (weapon_catalogue_is_missile(weapindx) ||
            weapon_catalogue_is_artillery(weapindx)) {
          if (btech_context_has_missile_hit_table(mech->xcode.context,
                                                  weapindx))
            damage *= btech_context_missile_hit_count(
                &(MissileHitLookup){.context = mech->xcode.context,
                                    .weapon = weapindx,
                                    .roll = 10});
        }
        if (damage > maxdamage) {
          result.slot = (CriticalSlotReference){loop, critloop};
          maxdamage = damage;
        }
      }
  result.damage = maxdamage;
  return result;
}

AmmunitionHazardResult inferno_ammunition_find(Mech *mech) {
  AmmunitionHazardResult result = {0};
  int loop;
  int critloop;
  int maxdamage = 0;
  int damage;
  int weapindx;
  int type, data;
  int mode;

  for (loop = 0; loop < NUM_SECTIONS; loop++)
    for (critloop = 0; critloop < NUM_CRITICALS; critloop++)
      if (equipment_is_ammunition(
              mech_critical_part_type(mech, loop, critloop)) &&
          !mech_critical_is_destroyed(mech, loop, critloop)) {
        data = mech_critical_data(mech, loop, critloop);
        type = mech_critical_part_type(mech, loop, critloop);
        mode = mech_critical_ammo_mode(mech, loop, critloop);
        if (!(mode & INFERNO_MODE))
          continue;
        weapindx = ammunition_to_weapon_index(type);
        damage = data * weapon_catalogue_damage(weapindx);
        if (weapon_catalogue_has_special(weapindx, GAUSS))
          continue;
        if (weapon_catalogue_is_missile(weapindx) ||
            weapon_catalogue_is_artillery(weapindx)) {
          if (btech_context_has_missile_hit_table(mech->xcode.context,
                                                  weapindx))
            damage *= btech_context_missile_hit_count(
                &(MissileHitLookup){.context = mech->xcode.context,
                                    .weapon = weapindx,
                                    .roll = 10});
        }
        if (damage > maxdamage) {
          result.slot = (CriticalSlotReference){loop, critloop};
          maxdamage = damage;
        }
      }
  result.damage = maxdamage;
  return result;
}

int FindRoundsForWeapon(Mech *mech, int weapindx) {
  int loop;
  int critloop;
  int desired;
  int found = 0;

  desired = ammunition_equipment_index(weapindx);
  for (loop = 0; loop < NUM_SECTIONS; loop++)
    for (critloop = 0; critloop < NUM_CRITICALS; critloop++)
      if (mech_critical_part_type(mech, loop, critloop) == desired &&
          !mech_critical_is_nonfunctional(mech, loop, critloop))
        found += mech_critical_data(mech, loop, critloop);
  return found;
}

const char *quad_locs[NUM_SECTIONS + 1] = {"Front Left Leg",
                                           "Front Right Leg",
                                           "Left Torso",
                                           "Right Torso",
                                           "Center Torso",
                                           "Rear Left Leg",
                                           "Rear Right Leg",
                                           "Head",
                                           NULL};

const char *mech_locs[NUM_SECTIONS + 1] = {
    "Left Arm", "Right Arm", "Left Torso", "Right Torso", "Center Torso",
    "Left Leg", "Right Leg", "Head",       NULL};

const char *bsuit_locs[NUM_BSUIT_MEMBERS + 1] = {"Suit 1", "Suit 2", "Suit 3",
                                                 "Suit 4", "Suit 5", "Suit 6",
                                                 "Suit 7", "Suit 8", NULL};

const char *veh_locs[NUM_VEH_SECTIONS + 1] = {
    "Left Side", "Right Side", "Front Side", "Aft Side",
    "Turret",    "Rotor",      NULL};

const char *aero_locs[NUM_AERO_SECTIONS + 1] = {"Nose", "Left Wing",
                                                "Right Wing", "Aft Side", NULL};

const char *ds_locs[NUM_DS_SECTIONS + 1] = {
    "Right Wing", "Left Wing", "Left Rear Wing", "Right Rear Wing", "Aft",
    "Nose",       NULL};

const char *ds_spher_locs[NUM_DS_SECTIONS + 1] = {"Front Right Side",
                                                  "Front Left Side",
                                                  "Rear Left Side",
                                                  "Rear Right Side",
                                                  "Aft",
                                                  "Nose",
                                                  NULL};

const char *const *ProperSectionStringFromType(int type, int mtype) {
  switch (type) {
  case CLASS_BSUIT:
    return bsuit_locs;
  case CLASS_MECH:
  case CLASS_MW:
    if (mtype == MOVE_QUAD)
      return quad_locs;
    return mech_locs;
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
  case CLASS_VTOL:
    return veh_locs;
  case CLASS_AERO:
    return aero_locs;
  case CLASS_SPHEROID_DS:
    return ds_spher_locs;
  case CLASS_DS:
    return ds_locs;
  }
  return NULL;
}

size_t unit_section_name_count(const UnitSectionCatalog *catalog) {
  switch (catalog->unit_type) {
  case CLASS_BSUIT:
    return NUM_BSUIT_MEMBERS;
  case CLASS_MECH:
  case CLASS_MW:
    return NUM_SECTIONS;
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
    return NUM_VEH_SECTIONS - 1;
  case CLASS_VTOL:
    return NUM_VEH_SECTIONS;
  case CLASS_AERO:
    return NUM_AERO_SECTIONS;
  case CLASS_SPHEROID_DS:
  case CLASS_DS:
    return NUM_DS_SECTIONS;
  }
  return 0;
}

const char *unit_section_name(const UnitSectionCatalog *catalog, size_t index) {
  const char *const *names =
      ProperSectionStringFromType(catalog->unit_type, catalog->movement_type);
  const size_t count = unit_section_name_count(catalog);
  if (names == nullptr || index >= count)
    return nullptr;
  return *(const char *const *)checked_storage_at_const(
      (const void *)names, count + 1, sizeof(*names), index);
}
