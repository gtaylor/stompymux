/* Implements BattleTech character mechanics for pcombat. */

#include <stdlib.h>

#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "pcombat_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

typedef struct PersonalArmorDefinition {
  const char *name;
  int loc;
  int loci;
  int deft;
  int defmin;
  int defpros;
  int defmax;
} PersonalArmorDefinition;

static const PersonalArmorDefinition PERSONAL_ARMOR[] = {
    {"Helmet", HEAD, 1, PC_IMPA | PC_HEAT, 10, 30, 30},
    {"Combat Helmet", HEAD, 2, PC_IMPA | PC_SHAR | PC_HEAT, 20, 50, 60},
    {"Gloves", RARM, 1, PC_SHAR | PC_HEAT, 10, 30, 20},
    {"Combat Gloves", RARM, 2, PC_SHAR | PC_HEAT | PC_IMPA, 20, 50, 60},
    {"Boots", RLEG, 1, PC_SHAR | PC_IMPA, 20, 40, 50},
    {"Combat Boots", RLEG, 2, PC_SHAR | PC_IMPA | PC_HEAT, 40, 50, 100},
    {"Flak Vest", CTORSO, 1, PC_SHAR, 20, 30, 60},
    {"Abrasive Vest", CTORSO, 2, PC_IMPA | PC_HEAT, 30, 40, 80},
    {"Combat Vest", CTORSO, 3, PC_IMPA | PC_HEAT | PC_SHAR, 40, 50, 100},
    {"Flak Armorplate", CTORSO, 4, PC_SHAR, 40, 50, 80},
    {"Abrasive Armorplate", CTORSO, 5, PC_IMPA | PC_HEAT, 40, 50, 80},
    {"Combat Armorplate", CTORSO, 6, PC_IMPA | PC_SHAR | PC_HEAT, 50, 50, 120},
    {"DEST Armor", CTORSO, 7, PC_IMPA | PC_SHAR | PC_HEAT, 50, 70, 120},
    {"Black Robes", CTORSO, 8, PC_IMPA | PC_SHAR | PC_HEAT, 60, 80, 140},
    {nullptr, -1, -1, 0, 0, 0, 0}};

static const PersonalArmorDefinition *personal_armor_at(size_t index) {
  return checked_storage_at_const(
      PERSONAL_ARMOR, sizeof(PERSONAL_ARMOR) / sizeof(*PERSONAL_ARMOR),
      sizeof(*PERSONAL_ARMOR), index);
}

int personal_combat_damage_to_unit(
    const PersonalCombatDamageConversion *conversion) {
  Mech *target = conversion->target;
  const int WEAPINDX = conversion->weapon_index;
  int dam = conversion->damage;
  int i = 0;

  if (mech_class(target) == CLASS_MW)
    return dam;
  if (WEAPINDX < 0 || !weapon_catalogue_is_personal_combat(WEAPINDX))
    return dam;
  i = dam / 100;
  dam = dam % 100;
  if (btech_random_range_int(mech_context(target), 1, 100) <= dam)
    i++;
  return i;
}

int unit_damage_to_personal_combat(
    const PersonalCombatDamageConversion *conversion) {
  Mech *target = conversion->target;
  const int WEAPINDX = conversion->weapon_index;
  const int DAM = conversion->damage;
  int i = 0, j;

  if (WEAPINDX >= 0 && weapon_catalogue_is_personal_combat(WEAPINDX))
    return DAM;
  if (mech_class(target) != CLASS_MW)
    return DAM;
  /* Target is MW _and_ we have yet to convert damage */
  for (j = 0; j < DAM; j++)
    i += btech_random_range_int(mech_context(target), 80, 130);
  return i;
}

static int pcombat_hitloc(int loc) {
  switch (loc) {
  case LTORSO:
  case RTORSO:
    return CTORSO;
  case LARM:
    return RARM;
  case LLEG:
    return RLEG;
  }
  return loc;
}

int personal_armor_reduce_damage(const PersonalArmorDamageRequest *request) {
  Mech *wounded = request->wounded;
  const int CAUSE = request->cause;
  int hitloc = request->hit_location;
  int int_damage = request->damage;
  const int ID = request->damage_identifier;
  size_t armor_index;
  int block;
  int noblock = 0;

  if (ID != -2)
    int_damage =
        (int_damage * btech_random_range_int(mech_context(wounded), 75, 125)) /
        100;
  if (mech_class(wounded) != CLASS_MW)
    return int_damage;
  hitloc = pcombat_hitloc(hitloc);
  if (!mech_section_armor(wounded, hitloc))
    return int_damage;
  const size_t ARMOR_COUNT = sizeof(PERSONAL_ARMOR) / sizeof(*PERSONAL_ARMOR);
  for (armor_index = 0; armor_index < ARMOR_COUNT; armor_index++) {
    const PersonalArmorDefinition *candidate = personal_armor_at(armor_index);
    if (candidate->name == nullptr ||
        (candidate->loc == hitloc &&
         candidate->loci == mech_section_armor(wounded, hitloc)))
      break;
  }
  if (btech_random_range_int(mech_context(wounded), 1, 5) == 1) {
    if (btech_random_range_int(mech_context(wounded), 1, 2) == 1)
      int_damage = int_damage * 2;
    else
      noblock = 1;
  } else if (btech_random_range_int(mech_context(wounded), 1, 10) == 2) {
    int_damage = int_damage / 2;
  }
  const PersonalArmorDefinition *armor = personal_armor_at(armor_index);
  if (!armor->name)
    return int_damage;
  const int PERSONAL_COMBAT_FLAGS =
      CAUSE >= 0 ? weapon_catalogue_personal_combat_flags(CAUSE) : 0;
  if (CAUSE >= 0 && !(armor->deft & PERSONAL_COMBAT_FLAGS) &&
      PERSONAL_COMBAT_FLAGS)
    return int_damage;
  block = bounded(
      btech_random_range_int(mech_context(wounded), 1, (armor->defmin / 2)),
      abs(int_damage * armor->defpros / 100), armor->defmax / 2);
  if (noblock)
    block = 0;
  if (abs(int_damage) < block) {
    mech_printf(wounded, MECHALL, "Your armor blocks all of the damage!");
    return 0;
  }
  if (block) {
    mech_printf(wounded, MECHALL, "Armor blocks %d points of the damage!",
                block);
  }
  return (abs(int_damage) - block) * int_damage / abs(int_damage);
}
