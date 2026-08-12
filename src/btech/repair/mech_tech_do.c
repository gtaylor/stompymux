/* Implements BattleTech repair mechanics for unit tech do. */

#include <stdlib.h>
#include <string.h>

#include "equipment_types.h"
#include "mech_parts.h"
#include "mux/server/platform.h"
/* All the *_{succ|fail|econ} functions belong here */
#include "btech/context.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_tech_api.h"
#include "mech_tech_do_api.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "repair_job.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static bool parts_consume_one(DbRef player, Mech *mech, int location, int part,
                              int brand, int count) {
  const MechPartRequirement REQUIREMENT = {
      .part = mech_parts_alias(
          &(MechPartLocation){.mech = mech, .section = location, .part = part}),
      .brand = brand,
      .count = count,
  };
  return mech_parts_consume(mech, player, &REQUIREMENT, 1);
}

static bool parts_consume_two(DbRef player, Mech *mech, int first_part,
                              int first_brand, int first_count, int second_part,
                              int second_brand, int second_count) {
  const MechPartRequirement REQUIREMENTS[] = {
      {.part = first_part, .brand = first_brand, .count = first_count},
      {.part = second_part, .brand = second_brand, .count = second_count},
  };
  return mech_parts_consume(mech, player, REQUIREMENTS,
                            sizeof(REQUIREMENTS) / sizeof(REQUIREMENTS[0]));
}

static bool parts_consume_four(DbRef player, Mech *mech, int first_part,
                               int first_brand, int first_count,
                               int second_part, int second_brand,
                               int second_count, int third_part,
                               int third_brand, int third_count,
                               int fourth_part, int fourth_brand,
                               int fourth_count) {
  const MechPartRequirement REQUIREMENTS[] = {
      {.part = first_part, .brand = first_brand, .count = first_count},
      {.part = second_part, .brand = second_brand, .count = second_count},
      {.part = third_part, .brand = third_brand, .count = third_count},
      {.part = fourth_part, .brand = fourth_brand, .count = fourth_count},
  };
  return mech_parts_consume(mech, player, REQUIREMENTS,
                            sizeof(REQUIREMENTS) / sizeof(REQUIREMENTS[0]));
}

typedef struct AmmoType {
  char name;         /* Letter identifying the ammo in 'reload' */
  const char *lname; /* Long name (for printing) */
  int aflag;         /* Flag to set on the crittype */
  int rtype;         /* required type flag: if non-negative, weapon has
                                to be this type to allow this ammo */
  int ntype;         /* disallowed type flag: if non-negative, weapon
                                cannot be this type to allow this ammo */
  int rspec;         /* required 'special' flags: if non-zero,
                                weapon has to have at least one of these
                                bits in the 'special' flag for it to allow
                                this ammo */
  int nspec;         /* disallowes 'special' flags: if non-zero,
                                weapon cannot have any of these bits set,
                                in the special flag, to allow this ammo */
} AmmoType;

static const AmmoType AMMO_TYPES[] = {
    {'-', "normal", 0, -1, -1, 0, 0},
    {'L', "cluster", LBX_MODE, -1, -1, LBX, 0},
    {'A', "artemis", ARTEMIS_MODE, TMISSILE, -1, 0, DAR | NARC | INARC},
    {'N', "narc", NARC_MODE, TMISSILE, -1, 0, DAR | NARC | INARC},
    {'S', "swarm", SWARM_MODE, TMISSILE, -1, IDF, DAR | NARC | INARC},
    {'1', "swarm-1", SWARM1_MODE, TMISSILE, -1, IDF, DAR | NARC | INARC},
    {'I', "inferno", INFERNO_MODE, TMISSILE, -1, 0, IDF | DAR | NARC | INARC},
    {'X', "explosive", INARC_EXPLO_MODE, TMISSILE, -1, INARC, 0},
    {'Y', "haywire", INARC_HAYWIRE_MODE, TMISSILE, -1, INARC, 0},
    {'E', "ecm", INARC_ECM_MODE, TMISSILE, -1, INARC, 0},
    {'Z', "nemesis", INARC_NEMESIS_MODE, TMISSILE, -1, INARC, 0},
    {'R', "ap", AC_AP_MODE, TAMMO, -1, RFAC, 0},
    {'F', "flechette", AC_FLECHETTE_MODE, TAMMO, -1, RFAC, 0},
    {'D', "incendiary", AC_INCENDIARY_MODE, TAMMO, -1, RFAC, 0},
    {'P', "precision", AC_PRECISION_MODE, TAMMO, -1, RFAC, 0},
    {'T', "stinger", STINGER_MODE, TMISSILE, -1, IDF, DAR},
    {'U', "caseless", AC_CASELESS_MODE, TAMMO, -1, RFAC, 0},
    {'G', "semiguided", SGUIDED_MODE, TMISSILE, -1, IDF, DAR},
    {'H', "highexplosive", ATM_HE_MODE, TMISSILE, -1, IDF, DAR},
    {'V', "extendedrange", ATM_ER_MODE, TMISSILE, -1, IDF, DAR},
    {'#', "lrmmode", MML_LRM_MODE, TMISSILE, -1, IDF, DAR},
    {0, NULL, 0, 0, 0, 0, 0}};

static const AmmoType *ammo_type(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(AMMO_TYPES, 22, sizeof(*AMMO_TYPES),
                                  (size_t)index);
}

int valid_ammo_mode(Mech *mech, int loc, int part, int let) {
  int w;
  int i;

  if (!equipment_is_ammunition(mech_critical_part_type(mech, loc, part)) ||
      !let)
    return -1;
  let = (unsigned char)ascii_to_upper(clamp_int_to_char(let));
  w = ammunition_to_weapon_index(mech_critical_part_type(mech, loc, part));

  if (weapon_catalogue_has_special(w, NOSPA))
    return -1;

  for (i = 0; ammo_type(i)->name; i++) {
    const AmmoType *type = ammo_type(i);
    if (type->name != let)
      continue;
    if (type->rtype >= 0 && weapon_catalogue_type(w) != type->rtype)
      continue;
    if (type->rspec && !weapon_catalogue_has_special(w, type->rspec))
      continue;
    if (type->ntype >= 0 && weapon_catalogue_type(w) == type->ntype)
      continue;
    if (type->nspec && weapon_catalogue_has_special(w, type->nspec))
      continue;
    return type->aflag;
  }
  return -1;
}

int find_ammo_type(Mech *mech, int loc, int part) {
  int t = mech_critical_part_type(mech, loc, part);
  int m = mech_critical_ammo_mode(mech, loc, part);
  int base = -1;

  if (!equipment_is_ammunition(t))
    return t;
  t = ammunition_to_weapon_index(t);

  if (strstr(weapon_catalogue_name(t), "StreakSRM"))
    base = SSRM_AMMO;
  else if (strstr(weapon_catalogue_name(t), "StreakLRM"))
    base = SLRM_AMMO;
  else if (strstr(weapon_catalogue_name(t), "ELRM"))
    base = ELRM_AMMO;
  else if (strstr(weapon_catalogue_name(t), "LR_DFM"))
    base = LR_DFM_AMMO;
  else if (strstr(weapon_catalogue_name(t), "SR_DFM"))
    base = SR_DFM_AMMO;
  else if (strstr(weapon_catalogue_name(t), "LRM"))
    base = LRM_AMMO;
  else if (strstr(weapon_catalogue_name(t), "SRM"))
    base = SRM_AMMO;
  else if (strstr(weapon_catalogue_name(t), "MRM"))
    base = MRM_AMMO;

  if (!(m & AMMO_MODES)) {
    if (base < 0)
      return ammunition_equipment_index(t);
    return cargo_equipment_index(base);
  }

  if (m & LBX_MODE) {
    if (strstr(weapon_catalogue_name(t), "LB20"))
      base = LBX20_AMMO;
    else if (strstr(weapon_catalogue_name(t), "LB10"))
      base = LBX10_AMMO;
    else if (strstr(weapon_catalogue_name(t), "LB5"))
      base = LBX5_AMMO;
    else if (strstr(weapon_catalogue_name(t), "LB2"))
      base = LBX2_AMMO;
    if (base < 0)
      return ammunition_equipment_index(t);
    return cargo_equipment_index(base);
  }

  if (m & AC_MODES) {
    if (m & AC_AP_MODE) {
      if (strstr(weapon_catalogue_name(t), "AC/2"))
        base = AC2_AP_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/5"))
        base = AC5_AP_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/10"))
        base = AC10_AP_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/20"))
        base = AC20_AP_AMMO;
      if (strstr(weapon_catalogue_name(t), "LightAC/2"))
        base = LAC2_AP_AMMO;
      if (strstr(weapon_catalogue_name(t), "LightAC/5"))
        base = LAC5_AP_AMMO;
    }

    if (m & AC_FLECHETTE_MODE) {
      if (strstr(weapon_catalogue_name(t), "AC/2"))
        base = AC2_FLECHETTE_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/5"))
        base = AC5_FLECHETTE_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/10"))
        base = AC10_FLECHETTE_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/20"))
        base = AC20_FLECHETTE_AMMO;
      if (strstr(weapon_catalogue_name(t), "LightAC/2"))
        base = LAC2_FLECHETTE_AMMO;
      if (strstr(weapon_catalogue_name(t), "LightAC/5"))
        base = LAC5_FLECHETTE_AMMO;
    }

    if (m & AC_INCENDIARY_MODE) {
      if (strstr(weapon_catalogue_name(t), "AC/2"))
        base = AC2_INCENDIARY_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/5"))
        base = AC5_INCENDIARY_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/10"))
        base = AC10_INCENDIARY_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/20"))
        base = AC20_INCENDIARY_AMMO;
      if (strstr(weapon_catalogue_name(t), "LightAC/2"))
        base = LAC2_INCENDIARY_AMMO;
      if (strstr(weapon_catalogue_name(t), "LightAC/5"))
        base = LAC5_INCENDIARY_AMMO;
    }

    if (m & AC_PRECISION_MODE) {
      if (strstr(weapon_catalogue_name(t), "AC/2"))
        base = AC2_PRECISION_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/5"))
        base = AC5_PRECISION_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/10"))
        base = AC10_PRECISION_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/20"))
        base = AC20_PRECISION_AMMO;
      if (strstr(weapon_catalogue_name(t), "LightAC/2"))
        base = LAC2_PRECISION_AMMO;
      if (strstr(weapon_catalogue_name(t), "LightAC/5"))
        base = LAC5_PRECISION_AMMO;
    }

    if (m & AC_CASELESS_MODE) {
      if (strstr(weapon_catalogue_name(t), "AC/2"))
        base = AC2_CASELESS_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/5"))
        base = AC5_CASELESS_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/10"))
        base = AC10_CASELESS_AMMO;
      if (strstr(weapon_catalogue_name(t), "AC/20"))
        base = AC20_CASELESS_AMMO;
      if (strstr(weapon_catalogue_name(t), "LightAC/2"))
        base = LAC2_CASELESS_AMMO;
      if (strstr(weapon_catalogue_name(t), "LightAC/5"))
        base = LAC5_CASELESS_AMMO;
    }
    if (base < 0)
      return ammunition_equipment_index(t);
    return cargo_equipment_index(base);
  }

  if (m & INARC_EXPLO_MODE)
    return cargo_equipment_index(INARC_EXPLO_AMMO);
  if (m & INARC_HAYWIRE_MODE)
    return cargo_equipment_index(INARC_HAYWIRE_AMMO);
  if (m & INARC_ECM_MODE)
    return cargo_equipment_index(INARC_ECM_AMMO);
  if (m & INARC_NEMESIS_MODE)
    return cargo_equipment_index(INARC_NEMESIS_AMMO);

  if (base < 0)
    return ammunition_equipment_index(t);
  if (m & NARC_MODE)
    return cargo_equipment_index(base) + NARC_LRM_AMMO - LRM_AMMO;
  if (m & ARTEMIS_MODE)
    return cargo_equipment_index(base) + ARTEMIS_LRM_AMMO - LRM_AMMO;
  if (m & SWARM_MODE)
    return cargo_equipment_index(base) + SWARM_LRM_AMMO - LRM_AMMO;
  if (m & SWARM1_MODE)
    return cargo_equipment_index(base) + SWARM1_LRM_AMMO - LRM_AMMO;
  if (m & INFERNO_MODE)
    return cargo_equipment_index(base) + INFERNO_SRM_AMMO - SRM_AMMO;
  if (m & STINGER_MODE)
    return cargo_equipment_index(base) + AMMO_LRM_STINGER - LRM_AMMO;
  if (m & SGUIDED_MODE)
    return cargo_equipment_index(base) + AMMO_LRM_SGUIDED - LRM_AMMO;
  return cargo_equipment_index(base);
}

int replace_econ(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  int part = call->selection.part;
  if (equipment_is_ammunition(mech_critical_part_type(mech, loc, part)))
    return 0;
  if (!parts_consume_one(player, mech, loc,
                         mech_critical_part_type(mech, loc, part),
                         mech_critical_brand(mech, loc, part), 1))
    return -1;
  return 0;
}

int reload_econ(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  int part = call->selection.part;
  int ammotype = find_ammo_type(mech, loc, part);

  if (!parts_consume_one(player, mech, loc, ammotype,
                         mech_critical_brand(mech, loc, part), 1))
    return -1;
  return 0;
}

int fixarmor_econ(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  int *val = call->amount;
  if (!parts_consume_one(player, mech, loc, tech_proper_armor_part(mech), 0,
                         *val))
    return -1;
  return 0;
}

int fixinternal_econ(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  int *val = call->amount;
  if (!parts_consume_one(player, mech, loc, tech_proper_internal_part(mech), 0,
                         *val))
    return -1;
  return 0;
}

int repair_econ(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  int part = call->selection.part;
  if (equipment_is_ammunition(mech_critical_part_type(mech, loc, part)))
    return 0;
  int destroyed = mech_critical_is_destroyed(mech, loc, part) ? 3 : 1;
  if (!parts_consume_two(player, mech, cargo_equipment_index(S_ELECTRONIC), 0,
                         destroyed, tech_proper_internal_part(mech), 0,
                         destroyed))
    return -1;
  return 0;
}

int repairenhcrit_econ(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  if (!parts_consume_one(player, mech, loc, cargo_equipment_index(S_ELECTRONIC),
                         0, 1))
    return -1;
  return 0;
}

int reattach_econ(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
#ifndef BT_COMPLEXREPAIRS
  if (!parts_consume_two(player, mech, tech_proper_internal_part(mech), 0,
                         mech_section_original_internal(mech, loc),
                         cargo_equipment_index(S_ELECTRONIC), 0,
                         mech_section_original_internal(mech, loc)))
    return -1;
#else
  if (btech_context_uses_complex_repairs(mech_context(mech))) {
    if (mech_class(mech) == CLASS_MECH) {
      if (!parts_consume_two(player, mech, tech_proper_internal_part(mech), 0,
                             mech_section_original_internal(mech, loc),
                             ProperMyomer(mech), 0, 1))
        return -1;
    } else {
      if (!parts_consume_one(player, mech, loc, tech_proper_internal_part(mech),
                             0, mech_section_original_internal(mech, loc)))
        return -1;
    }
  } else {
    if (!parts_consume_two(player, mech, tech_proper_internal_part(mech), 0,
                           mech_section_original_internal(mech, loc),
                           cargo_equipment_index(S_ELECTRONIC), 0,
                           mech_section_original_internal(mech, loc)))
      return -1;
  }
#endif
  return 0;
}

#define BSUIT_REPAIR_INTERNAL_NEEDED 10
#define BSUIT_REPAIR_SENSORS_NEEDED 2
#define BSUIT_REPAIR_LIFESUPPORT_NEEDED 2
#define BSUIT_REPAIR_ELECTRONICS_NEEDED 10

int replacesuit_econ(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  if (!parts_consume_four(
          player, mech, tech_proper_internal_part(mech), 0,
          BSUIT_REPAIR_INTERNAL_NEEDED, cargo_equipment_index(BSUIT_SENSOR), 0,
          BSUIT_REPAIR_SENSORS_NEEDED, cargo_equipment_index(BSUIT_LIFESUPPORT),
          0, BSUIT_REPAIR_LIFESUPPORT_NEEDED,
          cargo_equipment_index(BSUIT_ELECTRONIC), 0,
          BSUIT_REPAIR_ELECTRONICS_NEEDED))
    return -1;
  return 0;
}

/*
 * Added for new flood code by Kipsta
 * 8/4/99
 */

int reseal_econ(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  if (!parts_consume_two(player, mech, tech_proper_internal_part(mech), 0,
                         mech_section_original_internal(mech, loc),
                         cargo_equipment_index(S_ELECTRONIC), 0,
                         mech_section_original_internal(mech, loc)))
    return -1;
  return 0;
}

/* -------------------------------------------- Successes */

/* Replace success is just that ; success, therefore the fake
   functions here */
int replacep_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}
int replaceg_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}
int reload_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}
int fixinternal_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}
int fixarmor_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}
int reattach_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}
int reseal_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}
int replacesuit_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}

/* Repairs _Should_ have some averse effects */
int repairg_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}
int repairenhcrit_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}
int repairp_succ(const RepairOperationCall *call) {
  (void)call;
  return 0;
}

/* -------------------------------------------- Failures */

/* Replace failures give you one chance to roll for object recovery,
   otherwise it's irretrieavbly lost */
int replaceg_fail(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  int part = call->selection.part;
  int w = equipment_is_weapon(mech_critical_part_type(mech, loc, part));

  if (tech_roll(player, mech, REPLACE_DIFFICULTY) < 0) {
    notify_printf(btech_context_evaluation(mech_context(mech)), player,
                  "You muck around, wasting the %s in the progress.",
                  w ? "weapon" : "part");
    return -1;
  }
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Despite messing the repair, you manage not to waste the %s.",
                w ? "weapon" : "part");
#ifndef BT_COMPLEXREPAIRS
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 find_ammo_type(mech, loc, part),
                 mech_critical_brand(mech, loc, part), 1);
#else
  mech_parts_add(mech, loc, FindAmmoType(mech, loc, part),
                 mech_critical_brand(mech, loc, part), 1);
#endif
  return -1;
}

int repairg_fail(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  int part = call->selection.part;
  if (mech_critical_is_destroyed(mech, loc, part))
    /* If we are calling repairgun on a thing that is actually destroyed
     * the following check *should not* be necessary. Nevertheless... */
    if (get_weapon_crits(mech, weapon_from_equipment_index(
                                   mech_critical_part_type(mech, loc, part))) >
        4) {
      mech_critical_destroy(mech, loc, part + 1);
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You muck around, trashing the gun in the process.");
      return -1;
    }
  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "Your repair fails.. all the parts are wasted for good.");
  return -1;
}

int repairenhcrit_fail(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "You don't manage to repair the damage.");
  return -1;
}

/* Replacepart = Replacegun, for now */
int replacep_fail(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "Your repair fails.. all the parts are wasted for good.");
  return -1;
}

/* Repairpart = Repairgun, for now */
int repairp_fail(const RepairOperationCall *call) { return repairg_fail(call); }

/* Reload fail = ammo is wasted and some time, but no averse effects (yet) */
int reload_fail(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "You fumble around, wasting the ammo in the progress.");
  return -1;
}

/* Fixarmor/fixinternal failure means that at least 1, or at worst
   _all_, points are wasted */
int fixarmor_fail(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int *val = call->amount;
  int tot = 0;
  int should = *val;

  if (tech_roll(player, mech, FIXARMOR_DIFFICULTY) >= 0)
    tot += 50;
  tot += btech_random_range(mech_context(mech), 5, 44);
  tot = (tot * should) / 100;
  if (tot == 0)
    tot = 1;
  if (tot == should)
    tot = should - 1;
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Your armor patching isn't exactly perfect.. "
                "You managed to fix %d out of %d.",
                tot, should);
  *val = tot;
  return 0;
}

int fixinternal_fail(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int *val = call->amount;
  int tot = 0;
  int should = *val;

  if (tech_roll(player, mech, FIXARMOR_DIFFICULTY) >= 0)
    tot += 50;
  tot += btech_random_range(mech_context(mech), 5, 44);
  tot = (tot * should) / 100;
  if (tot == 0)
    tot = 1;
  if (tot == should)
    tot = should - 1;
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Your internal patching isn't exactly perfect.. You managed to "
                "fix %d out of %d.",
                tot, should);
  *val = tot;
  return 0;
}

/* Reattach has 2 failures:
   - if you succeed in second roll, it takes just 1.5x time
   - if you don't, some (random %) of stuff is wasted and nothing is
   done (yet some techtime goes nonetheless */
int reattach_fail(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  int tot;

  if (tech_roll(player, mech, REATTACH_DIFFICULTY) >= 0)
    return 0;
  tot = btech_random_range_int(mech_context(mech), 5, 94);
  notify_printf(
      btech_context_evaluation(mech_context(mech)), player,
      "Despite your disastrous failure, you recover %d%% of the materials.",
      tot);
  tot = (tot * mech_section_original_internal(mech, loc)) / 100;
  if (tot == 0)
    tot = 1;
  if (tot == mech_section_original_internal(mech, loc))
    tot = mech_section_original_internal(mech, loc) - 1;
#ifndef BT_COMPLEXREPAIRS
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 cargo_equipment_index(S_ELECTRONIC), 0, tot);
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 tech_proper_internal_part(mech), 0, tot);
#else
  mech_parts_add(mech, loc, cargo_equipment_index(S_ELECTRONIC), 0, tot);
  mech_parts_add(mech, loc, tech_proper_internal_part(mech), 0, tot);
  if (btech_context_uses_complex_repairs(mech_context(mech)) &&
      mech_class(mech) == CLASS_MECH)
    mech_parts_add(mech, loc, ProperMyomer(mech), 0, 1);
#endif
  return -1;
}

int replacesuit_fail(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int w_rand = 0;

  if (tech_roll(player, mech, REATTACH_DIFFICULTY) >= 0)
    return 0;

  w_rand = btech_random_range_int(mech_context(mech), 5, 94);
  notify_printf(
      btech_context_evaluation(mech_context(mech)), player,
      "Despite your disastrous failure, you recover %d%% of the materials.",
      w_rand);
#ifndef BT_COMPLEXREPAIRS
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 cargo_equipment_index(BSUIT_SENSOR), 0,
                 max(((BSUIT_REPAIR_SENSORS_NEEDED * w_rand) / 100), 1));
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 cargo_equipment_index(BSUIT_LIFESUPPORT), 0,
                 ((BSUIT_REPAIR_LIFESUPPORT_NEEDED * w_rand) / 100));
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 cargo_equipment_index(BSUIT_ELECTRONIC), 0,
                 ((BSUIT_REPAIR_ELECTRONICS_NEEDED * w_rand) / 100));
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 tech_proper_internal_part(mech), 0,
                 max(((BSUIT_REPAIR_INTERNAL_NEEDED * w_rand) / 100), 1));
#else
  int loc = call->selection.location;
  mech_parts_add(mech, loc, cargo_equipment_index(BSUIT_SENSOR), 0,
                 MAX(((BSUIT_REPAIR_SENSORS_NEEDED * wRand) / 100), 1));
  mech_parts_add(mech, loc, cargo_equipment_index(BSUIT_LIFESUPPORT), 0,
                 ((BSUIT_REPAIR_LIFESUPPORT_NEEDED * wRand) / 100));
  mech_parts_add(mech, loc, cargo_equipment_index(BSUIT_ELECTRONIC), 0,
                 ((BSUIT_REPAIR_ELECTRONICS_NEEDED * wRand) / 100));
  mech_parts_add(mech, loc, tech_proper_internal_part(mech), 0,
                 MAX(((BSUIT_REPAIR_INTERNAL_NEEDED * wRand) / 100), 1));
#endif
  return -1;
}

/*
 * Added by Kipsta for flooding code
 * 8/4/99
 */

int reseal_fail(const RepairOperationCall *call) {
  DbRef player = call->player;
  Mech *mech = call->mech;
  int loc = call->selection.location;
  int tot;

  if (tech_roll(player, mech, RESEAL_DIFFICULTY) >= 0)
    return 0;
  tot = btech_random_range_int(mech_context(mech), 5, 94);
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "You don't manage to get all the water out and seal the "
                "section, though you recover %d%% of the materials.",
                tot);
  tot = (tot * mech_section_original_internal(mech, loc)) / 100;
  if (tot == 0)
    tot = 1;
  if (tot == mech_section_original_internal(mech, loc))
    tot = mech_section_original_internal(mech, loc) - 1;
#ifndef BT_COMPLEXREPAIRS
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 cargo_equipment_index(S_ELECTRONIC), 0, tot);
  mech_parts_add(mech, MECH_PART_LOCATION_UNUSED,
                 tech_proper_internal_part(mech), 0, tot);
#else
  mech_parts_add(mech, loc, cargo_equipment_index(S_ELECTRONIC), 0, tot);
  mech_parts_add(mech, loc, tech_proper_internal_part(mech), 0, tot);
#endif
  return -1;
}
