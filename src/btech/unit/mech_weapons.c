#include "btech/context.h"
#include "checked_conversion.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_utils_internal.h"
#include "weapon_catalogue_api.h"

// Added i < 9 for Split crit tests
#define UGLYTEST                                                               \
  if (num_crits) {                                                             \
    if (num_crits != (i = GetWeaponCrits(mech, lastweap)) && i < 9) {          \
      if (whine)                                                               \
        btech_channel_send(                                                    \
            mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",              \
            tprintf("Error in the numcriticals for weapon on #%ld! "           \
                    "(Should be: %d, is: %d)",                                 \
                    mech->mynum, i, num_crits));                               \
      return -1;                                                               \
    }                                                                          \
    num_crits = 0;                                                             \
  }

bool weapon_catalogue_is_artillery(int weapon_index) {
  return MechWeapons[weapon_index].type == TARTILLERY;
}

bool weapon_catalogue_is_missile(int weapon_index) {
  return MechWeapons[weapon_index].type == TMISSILE;
}

bool weapon_catalogue_is_ballistic(int weapon_index) {
  return MechWeapons[weapon_index].type == TAMMO;
}

bool weapon_catalogue_is_energy(int weapon_index) {
  return MechWeapons[weapon_index].type == TBEAM;
}

bool weapon_catalogue_is_flamer(int weapon_index) {
  return strstr(MechWeapons[weapon_index].name, "Flamer") != nullptr;
}

bool weapon_catalogue_is_coolant(int weapon_index) {
  return strstr(MechWeapons[weapon_index].name, "Coolant") != nullptr;
}

bool weapon_catalogue_is_acid(int weapon_index) {
  return strstr(MechWeapons[weapon_index].name, "Acid") != nullptr;
}

bool weapon_catalogue_supports_indirect_fire(int weapon_index) {
  return MechWeapons[weapon_index].special & IDF;
}

bool weapon_catalogue_is_anti_missile(int weapon_index) {
  return MechWeapons[weapon_index].special & AMS;
}

bool equipment_can_use_targeting_computer(int equipment_index) {
  int weapon_index = weapon_from_equipment_index(equipment_index);
  const char *name = &MechWeapons[weapon_index].name[3];
  return (MechWeapons[weapon_index].type == TBEAM ||
          MechWeapons[weapon_index].type == TAMMO) &&
         strcmp(name, "Flamer") && strcmp(name, "MachineGun") &&
         strcmp(name, "LightMachineGun") && strcmp(name, "HeavyMachineGun") &&
         !(MechWeapons[weapon_index].special & PCOMBAT);
}

bool weapon_catalogue_is_hot_loaded(int weapon_index, int fire_mode) {
  return (fire_mode & HOTLOAD_MODE) &&
         (MechWeapons[weapon_index].special & IDF);
}

const char *weapon_catalogue_name(int weapon_index) {
  return MechWeapons[weapon_index].name;
}

int weapon_catalogue_damage(int weapon_index) {
  return MechWeapons[weapon_index].damage;
}

int weapon_catalogue_cluster_size(int weapon_index) {
  const struct WeaponDefinition *weapon = &MechWeapons[weapon_index];
  return (weapon->special & (IDF | MRM | ROCKET)) && weapon->damage == 1 ? 5
                                                                         : 1;
}

int weapon_catalogue_effective_range(int weapon_index, bool extended) {
  int normal =
      weapon_catalogue_is_artillery(weapon_index)
          ? ARTILLERY_MAPSHEET_SIZE * MechWeapons[weapon_index].longrange
          : MechWeapons[weapon_index].longrange;
  int extended_range = MechWeapons[weapon_index].medrange * 2;
  return extended && extended_range > normal ? extended_range : normal;
}

int weapon_catalogue_effective_water_range(int weapon_index, bool extended) {
  int normal = MechWeapons[weapon_index].longrange_water > 0
                   ? MechWeapons[weapon_index].longrange_water
               : MechWeapons[weapon_index].medrange_water > 0
                   ? MechWeapons[weapon_index].medrange_water
               : MechWeapons[weapon_index].shortrange_water > 0
                   ? MechWeapons[weapon_index].shortrange_water
                   : 0;
  int extended_range = MechWeapons[weapon_index].medrange_water * 2;
  return extended && extended_range > normal &&
                 MechWeapons[weapon_index].longrange_water > 0
             ? extended_range
             : normal;
}

int weapon_catalogue_range_for_section(const Mech *mech, int section,
                                       int weapon_index, bool extended) {
  if (mech_section_is_underwater(mech, section))
    return weapon_catalogue_effective_water_range(weapon_index, extended);
  return weapon_catalogue_effective_range(weapon_index, extended);
}

/* ASSERTION: Weapons must be located next to each other in criticals. */
int FindWeapons_Advanced(Mech *mech, int index, unsigned char *weaparray,
                         unsigned char *weapdataarray, int *critical,
                         int whine) {
  int loop;
  int weapcount = 0;
  int temp, data, lastweap = -1;
  int num_crits = 0, i;

  for (loop = 0; loop < MAX_WEAPS_SECTION; loop++) {
    temp = mech_critical_part_type(mech, index, loop);
    data = mech_critical_data(mech, index, loop);
    if (equipment_is_weapon(temp)) {
      temp = weapon_from_equipment_index(temp);
      if (weapcount == 0) {
        lastweap = temp;
        weapdataarray[weapcount] = clamp_int_to_unsigned_char(data);
        weaparray[weapcount] = clamp_int_to_unsigned_char(temp);
        critical[weapcount] = loop;
        weapcount++;
        num_crits = 1;
        continue;
      }
      if (!num_crits || temp != lastweap ||
          (num_crits == GetWeaponCrits(mech, temp))) {
        UGLYTEST;
        weaparray[weapcount] = clamp_int_to_unsigned_char(temp);
        weapdataarray[weapcount] = clamp_int_to_unsigned_char(data);
        critical[weapcount] = loop;
        lastweap = temp;
        num_crits = 1;
        weapcount++;
      } else
        num_crits++;
    } else
      UGLYTEST;
  }
  UGLYTEST;
  return (weapcount);
}

int FindAmmunition(Mech *mech, unsigned char *weaparray,
                   unsigned short *ammoarray, unsigned short *ammomaxarray,
                   unsigned int *modearray, int returnall) {
  int loop;
  int weapcount = 0;
  int temp, data;
  unsigned int mode;
  int index, i, j, duplicate;

  for (index = 0; index < NUM_SECTIONS; index++)
    for (loop = 0; loop < MAX_WEAPS_SECTION; loop++) {
      temp = mech_critical_part_type(mech, index, loop);
      if (equipment_is_ammunition(temp)) {
        data = mech_critical_data(mech, index, loop);
        mode = (mech_critical_ammo_mode(mech, index, loop) & AMMO_MODES);
        temp = ammunition_to_weapon_index(temp);
        duplicate = 0;

        for (i = 0; i < weapcount; i++) {
          if (temp == weaparray[i] && mode == modearray[i]) {
            if (!(mech_critical_is_nonfunctional(mech, index, loop)))
              ammoarray[i] = clamp_int_to_unsigned_short(ammoarray[i] + data);
            ammomaxarray[i] = clamp_int_to_unsigned_short(
                ammomaxarray[i] + FullAmmo(mech, index, loop));
            duplicate = 1;
          }
        }

        if (!duplicate) {
          weaparray[weapcount] = clamp_int_to_unsigned_char(temp);

          if (!(mech_critical_is_nonfunctional(mech, index, loop)))
            ammoarray[weapcount] = clamp_int_to_unsigned_short(data);
          else
            ammoarray[weapcount] = 0;

          ammomaxarray[weapcount] =
              clamp_int_to_unsigned_short(FullAmmo(mech, index, loop));
          modearray[weapcount] = mode;

          weapcount++;
        }
      }
    }
  /* Then, prune entries with 0 ammo left */
  if (!returnall) {
    for (i = 0; i < weapcount; i++)
      if (!ammoarray[i]) {
        for (j = i + 1; j < weapcount; j++) {
          weaparray[j - 1] = weaparray[j];
          ammoarray[j - 1] = ammoarray[j];
          ammomaxarray[j - 1] = ammomaxarray[j];
          modearray[j - 1] = modearray[j];
        }
        i--;
        weapcount--;
      }
  }
  return (weapcount);
}

int FindLegHeatSinks(Mech *mech) {
  int loop;
  int heatsinks = 0;

  for (loop = 0; loop < NUM_CRITICALS; loop++) {
    if (mech_critical_part_type(mech, LLEG, loop) ==
            special_equipment_index((HEAT_SINK)) &&
        !mech_critical_is_nonfunctional(mech, LLEG, loop))
      heatsinks++;
    if (mech_critical_part_type(mech, RLEG, loop) ==
            special_equipment_index((HEAT_SINK)) &&
        !mech_critical_is_nonfunctional(mech, RLEG, loop))
      heatsinks++;
    /*
     * Added by Kipsta on 8/5/99
     * Quads can get 'arm' HS in the water too
     */

    if (mech_is_quad(mech)) {
      if (mech_critical_part_type(mech, LARM, loop) ==
              special_equipment_index((HEAT_SINK)) &&
          !mech_critical_is_nonfunctional(mech, LARM, loop))
        heatsinks++;
      if (mech_critical_part_type(mech, RARM, loop) ==
              special_equipment_index((HEAT_SINK)) &&
          !mech_critical_is_nonfunctional(mech, RARM, loop))
        heatsinks++;
    }
  }
  return (heatsinks);
}

/* Added for tic support. */

/* returns the weapon index- -1 for not found, -2 for destroyed, -3, -4 */

/* for reloading/recycling */
int FindWeaponNumberOnMech_Advanced(Mech *mech, int number, int *section,
                                    int *crit, int sight) {
  int loop;
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int running_sum = 0;
  int num_weaps;
  int index;

  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    num_weaps =
        FindWeapons_Advanced(mech, loop, weaparray, weapdata, critical, 1);

    if (num_weaps <= 0)
      continue;

    if (number < running_sum + num_weaps) {
      /* we found it... */
      index = number - running_sum;
      if (mech_critical_is_nonfunctional(mech, loop, critical[index])) {
        *section = loop;
        *crit = critical[index];
        return TIC_NUM_DESTROYED;
      } else if (weapdata[index] > 0 && !sight) {
        *section = loop;
        *crit = critical[index];
        return (MechWeapons[weaparray[index]].type == TBEAM)
                   ? TIC_NUM_RECYCLING
                   : TIC_NUM_RELOADING;
      } else {

        if (((mech)->ud.sections)[loop].recycle &&
            (((mech)->ud.type) == CLASS_MECH ||
             ((mech)->ud.type) == CLASS_VEH_GROUND ||
             ((mech)->ud.type) == CLASS_VTOL) &&
            !sight) {

          *section = loop;
          *crit = critical[index];
          /* just did a physical attack */
          return TIC_NUM_PHYSICAL;
        }

        /* The recylce data for the weapon is clear- it is ready to fire! */
        *section = loop;
        *crit = critical[index];
        return weaparray[index];
      }
    } else
      running_sum += num_weaps;
  }
  return -1;
}

int FindWeaponNumberOnMech(Mech *mech, int number, int *section, int *crit) {
  return FindWeaponNumberOnMech_Advanced(mech, number, section, crit, 0);
}

int FindWeaponFromIndex(Mech *mech, int weapindx, int *section, int *crit) {
  int loop;
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int num_weaps;
  int index;

  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    num_weaps =
        FindWeapons_Advanced(mech, loop, weaparray, weapdata, critical, 1);
    for (index = 0; index < num_weaps; index++)
      if (weaparray[index] == weapindx) {
        *section = loop;
        *crit = critical[index];
        if (!mech_critical_is_nonfunctional(mech, loop, index) &&
            !mech_weapon_is_recycling_at(mech, loop, index))
          return 1;
        /* Return if not Recycling/Destroyed */
        /* Otherwise keep looking */
      }
  }
  return 0;
}

int FindWeaponIndex(Mech *mech, int number) {
  int loop;
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int running_sum = 0;
  int num_weaps;
  int index;

  if (number < 0)
    return -1; /* Anti-crash */
  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    num_weaps =
        FindWeapons_Advanced(mech, loop, weaparray, weapdata, critical, 1);
    if (num_weaps <= 0)
      continue;
    if (number < running_sum + num_weaps) {
      /* we found it... */
      index = number - running_sum;
      return weaparray[index];
    }
    running_sum += num_weaps;
  }
  return -1;
}

int FullAmmo(const Mech *mech, int loc, int pos) {
  int baseammo;
  int overage;

  baseammo = MechWeapons[ammunition_to_weapon_index(
                             mech_critical_part_type(mech, loc, pos))]
                 .ammoperton;
  if ((mech_critical_ammo_mode(mech, loc, pos) & AC_AP_MODE) ||
      (mech_critical_ammo_mode(mech, loc, pos) & AC_PRECISION_MODE) ||
      (mech_critical_fire_mode(mech, loc, pos) & HALFTON_MODE)) {
    return baseammo >> 1;
  }

  if ((mech_critical_ammo_mode(mech, loc, pos) & AC_CASELESS_MODE)) {
    return baseammo << 1;
  }

  if ((mech_critical_ammo_mode(mech, loc, pos) & MML_LRM_MODE)) {
    baseammo = baseammo * 1200;
    overage = baseammo % 1000;
    if (overage > 499)
      baseammo = (baseammo / 1000) + 1;
    else
      baseammo = (baseammo / 1000);
    return baseammo;
  }

  return baseammo;
}

int findAmmoInSection(Mech *mech, int section, int type, int nogof, int gof) {
  int wIter;

  /* Can't use LBX ammo as normal, but can use Narc and Artemis as normal */
  for (wIter = 0; wIter < NUM_CRITICALS; wIter++) {
    if (mech_critical_part_type(mech, section, wIter) == type &&
        !mech_critical_is_nonfunctional(mech, section, wIter) &&
        (!nogof || !(mech_critical_ammo_mode(mech, section, wIter) & nogof)) &&
        (!gof || (mech_critical_ammo_mode(mech, section, wIter) & gof))) {

      if (!mech_critical_is_nonfunctional(mech, section, wIter) &&
          mech_critical_data(mech, section, wIter) > 0)
        return wIter;
    }
  }

  return -1;
}

int FindAmmoForWeapon_sub(Mech *mech, int weapSection, int weapCritical,
                          int weapindx, int start, int *section, int *critical,
                          int nogof, int gof) {
  int loop;
  int foundSlot;
  int desired;
  int wCritType = 0;
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wDesiredLoc = -1;

  desired = ammunition_equipment_index(weapindx);

  /* The data on the desired location */
  if ((weapSection > -1) && (weapCritical > -1)) {
    wCritType = mech_critical_part_type(mech, weapSection, weapCritical);
    wWeapSize = GetWeaponCrits(mech, weapon_from_equipment_index(wCritType));
    wFirstCrit = FindFirstWeaponCrit(mech, weapSection, weapCritical, 0,
                                     wCritType, wWeapSize);

    wDesiredLoc =
        mech_critical_desired_ammo_section(mech, weapSection, wFirstCrit);

    if (wDesiredLoc >= 0) {
      foundSlot = findAmmoInSection(mech, wDesiredLoc, desired, nogof, gof);

      if (foundSlot >= 0) {
        *section = wDesiredLoc;
        *critical = foundSlot;

        return 1;
      }
    }
  }

  /* Now lets search the current section */
  foundSlot = findAmmoInSection(mech, start, desired, nogof, gof);

  if (foundSlot >= 0) {
    *section = start;
    *critical = foundSlot;

    return 1;
  }

  /* If all else fails, start hunting for ammo */
  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    if ((loop == start) || (loop == wDesiredLoc))
      continue;

    foundSlot = findAmmoInSection(mech, loop, desired, nogof, gof);

    if (foundSlot >= 0) {
      *section = loop;
      *critical = foundSlot;

      return 1;
    }
  }

  return 0;
}

int FindAmmoForWeapon(Mech *mech, int weapindx, int start, int *section,
                      int *critical) {
  return FindAmmoForWeapon_sub(mech, -1, -1, weapindx, start, section, critical,
                               AMMO_MODES, 0);
}

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
int GetSplitData(Mech *mech, int sect, int data, int *ssect, int *scrit,
                 int *stype) {
  switch (sect) {
  case RARM: // right arm goes to right torso
    *stype = special_equipment_index(SPLIT_CRIT_RIGHT);
    if ((*scrit = FindSplitCrits(mech, RTORSO, *stype, data)) >= 0) {
      *ssect = RTORSO;
      return 1;
    }
    break;
  case LARM: // left arm goes to left torso
    *stype = special_equipment_index(SPLIT_CRIT_LEFT);
    if ((*scrit = FindSplitCrits(mech, LTORSO, *stype, data)) >= 0) {
      *ssect = LTORSO;
      return 1;
    }
    break;
  case RTORSO: // torso more complex, need to go thru arm, leg, torso
    *stype = special_equipment_index(SPLIT_CRIT_RIGHT);
    if ((*scrit = FindSplitCrits(mech, CTORSO, *stype, data)) >= 0) {
      *ssect = CTORSO;
      return 1;
    } else if ((*scrit = FindSplitCrits(mech, RARM, *stype, data)) >= 0) {
      *ssect = RARM;
      return 1;
    } else if ((*scrit = FindSplitCrits(mech, RLEG, *stype, data)) >= 0) {
      *ssect = RLEG;
      return 1;
    }
    break;
  case LTORSO: // same for left torso
    *stype = special_equipment_index(SPLIT_CRIT_LEFT);
    if ((*scrit = FindSplitCrits(mech, CTORSO, *stype, data)) >= 0) {
      *ssect = CTORSO;
      return 1;
    } else if ((*scrit = FindSplitCrits(mech, LARM, *stype, data)) >= 0) {
      *ssect = LARM;
      return 1;
    } else if ((*scrit = FindSplitCrits(mech, LLEG, *stype, data)) >= 0) {
      *ssect = LLEG;
      return 1;
    }
    break;
  }
  return 0;
}

int FindDestructiveAmmo(Mech *mech, int *section, int *critical) {
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
        damage = data * MechWeapons[weapindx].damage;
        if (MechWeapons[weapindx].special & GAUSS)
          continue;
        if (weapon_catalogue_is_missile(weapindx) ||
            weapon_catalogue_is_artillery(weapindx)) {
          const MissileHitEntry *entry = missile_hit_registry_find_weapon(
              &mech->xcode.context->missile_hits, weapindx);
          if (entry != nullptr)
            damage *= entry->num_missiles[10];
        }
        if (damage > maxdamage) {
          *section = loop;
          *critical = critloop;
          maxdamage = damage;
        }
      }
  return (maxdamage);
}

int FindInfernoAmmo(Mech *mech, int *section, int *critical) {
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
        damage = data * MechWeapons[weapindx].damage;
        if (MechWeapons[weapindx].special & GAUSS)
          continue;
        if (weapon_catalogue_is_missile(weapindx) ||
            weapon_catalogue_is_artillery(weapindx)) {
          const MissileHitEntry *entry = missile_hit_registry_find_weapon(
              &mech->xcode.context->missile_hits, weapindx);
          if (entry != nullptr)
            damage *= entry->num_missiles[10];
        }
        if (damage > maxdamage) {
          *section = loop;
          *critical = critloop;
          maxdamage = damage;
        }
      }
  return (maxdamage);
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
