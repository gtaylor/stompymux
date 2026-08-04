/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <stdlib.h>

#include "command_handlers_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "section_types.h"

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

int personal_combat_damage_to_unit(Mech *target, int weapindx, int dam) {
  int i = 0;

  if (mech_class(target) == CLASS_MW)
    return dam;
  if (weapindx < 0 || !(MechWeapons[weapindx].special & PCOMBAT))
    return dam;
  i = dam / 100;
  dam = dam % 100;
  if (btech_random_range(mech_context(target), 1, 100) <= dam)
    i++;
  return i;
}

int unit_damage_to_personal_combat(Mech *target, int weapindx, int dam) {
  int i = 0, j;

  if (weapindx >= 0 && MechWeapons[weapindx].special & PCOMBAT)
    return dam;
  if (mech_class(target) != CLASS_MW)
    return dam;
  /* Target is MW _and_ we have yet to convert damage */
  for (j = 0; j < dam; j++)
    i += btech_random_range(mech_context(target), 80, 130);
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

int personal_armor_reduce_damage(Mech *wounded, int cause, int hitloc,
                                 int intDamage, int id) {
  int i;
  int block;
  int noblock = 0;

  if (id != -2)
    intDamage =
        (intDamage * btech_random_range(mech_context(wounded), 75, 125)) / 100;
  if (mech_class(wounded) != CLASS_MW)
    return intDamage;
  hitloc = pcombat_hitloc(hitloc);
  if (!mech_section_armor(wounded, hitloc))
    return intDamage;
  for (i = 0; PERSONAL_ARMOR[i].name; i++)
    if (PERSONAL_ARMOR[i].loc == hitloc &&
        PERSONAL_ARMOR[i].loci == mech_section_armor(wounded, hitloc))
      break;
  if (btech_random_range(mech_context(wounded), 1, 5) == 1) {
    if (btech_random_range(mech_context(wounded), 1, 2) == 1)
      intDamage = intDamage * 2;
    else
      noblock = 1;
  } else if (btech_random_range(mech_context(wounded), 1, 10) == 2)
    intDamage = intDamage / 2;
  if (!PERSONAL_ARMOR[i].name)
    return intDamage;
  if (cause >= 0 &&
      !((PERSONAL_ARMOR[i].deft) & (MechWeapons[cause].special & PCOMBAT)) &&
      (MechWeapons[cause].special & PCOMBAT))
    return intDamage;
  block = BOUNDED(btech_random_range(mech_context(wounded), 1,
                                     (PERSONAL_ARMOR[i].defmin / 2)),
                  abs(intDamage * PERSONAL_ARMOR[i].defpros / 100),
                  PERSONAL_ARMOR[i].defmax / 2);
  if (noblock)
    block = 0;
  if (abs(intDamage) < block) {
    mech_printf(wounded, MECHALL, "Your armor blocks all of the damage!");
    return 0;
  }
  if (block) {
    mech_printf(wounded, MECHALL, "Armor blocks %d points of the damage!",
                block);
  }
  return (abs(intDamage) - block) * intDamage / abs(intDamage);
}
