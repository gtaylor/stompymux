#include "btconfig.h"
#include "btech_channel.h"
#include "checked_conversion.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_utils_api.h"
#include "mech_utils_internal.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

extern const int num_def_weapons;

static const struct WeaponDefinition *weapon_catalogue_entry(int weapon_index) {
  if (weapon_index < 0)
    abort();
  return checked_storage_at_const(MechWeapons, (size_t)num_def_weapons,
                                  sizeof(struct WeaponDefinition),
                                  (size_t)weapon_index);
}

static unsigned char *weapon_byte_slot(unsigned char *values, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(values, MAX_WEAPS_SECTION, sizeof(*values),
                            (size_t)index);
}

static unsigned short *weapon_short_slot(unsigned short *values, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(values, MAX_WEAPS_SECTION, sizeof(*values),
                            (size_t)index);
}

static unsigned int *weapon_mode_slot(unsigned int *values, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(values, MAX_WEAPS_SECTION, sizeof(*values),
                            (size_t)index);
}

static int *weapon_critical_slot(int *values, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(values, MAX_WEAPS_SECTION, sizeof(*values),
                            (size_t)index);
}

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
  return weapon_catalogue_entry(weapon_index)->type == TARTILLERY;
}

bool weapon_catalogue_is_missile(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->type == TMISSILE;
}

bool weapon_catalogue_is_ballistic(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->type == TAMMO;
}

bool weapon_catalogue_is_energy(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->type == TBEAM;
}

bool weapon_catalogue_is_hand_to_hand(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->type == THAND;
}

bool weapon_catalogue_is_flamer(int weapon_index) {
  return strstr(weapon_catalogue_entry(weapon_index)->name, "Flamer") !=
         nullptr;
}

bool weapon_catalogue_is_coolant(int weapon_index) {
  return strstr(weapon_catalogue_entry(weapon_index)->name, "Coolant") !=
         nullptr;
}

bool weapon_catalogue_is_acid(int weapon_index) {
  return strstr(weapon_catalogue_entry(weapon_index)->name, "Acid") != nullptr;
}

bool weapon_catalogue_supports_indirect_fire(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->special & IDF;
}

bool weapon_catalogue_is_anti_missile(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->special & AMS;
}

bool weapon_catalogue_is_personal_combat(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & PCOMBAT) != 0;
}

bool weapon_catalogue_is_gauss(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & GAUSS) != 0;
}

bool weapon_catalogue_does_not_explode(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & NOBOOM) != 0;
}

bool weapon_catalogue_is_narc(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & NARC) != 0;
}

bool weapon_catalogue_is_inarc(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & INARC) != 0;
}

bool weapon_catalogue_is_clan_anti_missile(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & CLAT) != 0;
}

bool weapon_catalogue_is_pulse(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & PULSE) != 0;
}

bool weapon_catalogue_is_mrm(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & MRM) != 0;
}

bool weapon_catalogue_is_heavy(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & HVYW) != 0;
}

bool weapon_catalogue_is_rocket(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & ROCKET) != 0;
}

bool weapon_catalogue_is_only_rocket(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->special == ROCKET;
}

bool weapon_catalogue_is_dead_fire_missile(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & DFM) != 0;
}

bool weapon_catalogue_is_extended_lrm(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & ELRM) != 0;
}

bool weapon_catalogue_is_streak(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & STREAK) != 0;
}

bool weapon_catalogue_is_rotary_autocannon(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & RAC) != 0;
}

bool weapon_catalogue_is_heavy_gauss(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & HVYGAUSS) != 0;
}

bool weapon_catalogue_is_snub_ppc(int weapon_index) {
  return (weapon_catalogue_entry(weapon_index)->special & SNUBPPC) != 0;
}

bool weapon_catalogue_can_ignite_terrain(int weapon_index) {
  const char *name =
      checked_string_suffix(weapon_catalogue_name(weapon_index), 3);
  return strcmp(name, "ERSmallLaser") && strcmp(name, "SmallLaser") &&
         strcmp(name, "SmallPulseLaser") && strcmp(name, "X-SmallPulseLaser") &&
         strcmp(name, "ERSmallPulseLaser") && strcmp(name, "HeavySmallLaser") &&
         strcmp(name, "GaussRifle") && strcmp(name, "LightGaussRifle") &&
         strcmp(name, "HeavyGaussRifle") && strcmp(name, "MagshotGaussRifle") &&
         strcmp(name, "MachineGun") && strcmp(name, "LightMachineGun") &&
         strcmp(name, "HeavyMachineGun") && strcmp(name, "StreakSRM-2") &&
         strcmp(name, "SRM-2") && strcmp(name, "NarcBeacon") &&
         strcmp(name, "iNarcBeacon");
}

bool weapon_catalogue_can_clear_terrain(int weapon_index) {
  const char *name =
      checked_string_suffix(weapon_catalogue_name(weapon_index), 3);
  return strcmp(name, "ERSmallLaser") && strcmp(name, "SmallLaser") &&
         strcmp(name, "SmallPulseLaser") && strcmp(name, "X-SmallPulseLaser") &&
         strcmp(name, "ERSmallPulseLaser") && strcmp(name, "HeavySmallLaser") &&
         strcmp(name, "MachineGun") && strcmp(name, "LightMachineGun") &&
         strcmp(name, "HeavyMachineGun") && strcmp(name, "AC/2") &&
         strcmp(name, "UltraAC/2") && strcmp(name, "CaselessAC/2") &&
         strcmp(name, "HyperAC/2") && strcmp(name, "LightAC/2") &&
         strcmp(name, "RotaryAC/2") && strcmp(name, "LB2-XAC") &&
         strcmp(name, "AC/5") && strcmp(name, "UltraAC/5") &&
         strcmp(name, "CaselessAC/5") && strcmp(name, "HyperAC/5") &&
         strcmp(name, "LightAC/5") && strcmp(name, "RotaryAC/5") &&
         strcmp(name, "LB5-XAC") && strcmp(name, "StreakSRM-2") &&
         strcmp(name, "SRM-2");
}

bool weapon_catalogue_is_terrain_flamer(int weapon_index) {
  const char *name =
      checked_string_suffix(weapon_catalogue_name(weapon_index), 3);
  return !strcmp(name, "Flamer") || !strcmp(name, "HeavyFlamer");
}

int weapon_catalogue_personal_combat_flags(int weapon_index) {
  return (int)(weapon_catalogue_entry(weapon_index)->special & PCOMBAT);
}

int weapon_catalogue_type(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->type;
}

long weapon_catalogue_specials(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->special;
}

bool weapon_catalogue_has_special(int weapon_index, int special) {
  return (weapon_catalogue_entry(weapon_index)->special & special) != 0;
}

bool equipment_can_use_targeting_computer(int equipment_index) {
  int weapon_index = weapon_from_equipment_index(equipment_index);
  const struct WeaponDefinition *weapon = weapon_catalogue_entry(weapon_index);
  const char *name = strchr(weapon->name, '.');
  if (name == nullptr)
    return false;
  return (weapon->type == TBEAM || weapon->type == TAMMO) &&
         strcmp(name, ".Flamer") && strcmp(name, ".MachineGun") &&
         strcmp(name, ".LightMachineGun") && strcmp(name, ".HeavyMachineGun") &&
         !(weapon->special & PCOMBAT);
}

bool weapon_catalogue_is_hot_loaded(int weapon_index, int fire_mode) {
  return (fire_mode & HOTLOAD_MODE) &&
         (weapon_catalogue_entry(weapon_index)->special & IDF);
}

const char *weapon_catalogue_name(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->name;
}

int weapon_catalogue_damage(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->damage;
}

int weapon_catalogue_heat(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->heat;
}

int weapon_catalogue_recycle_time(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->vrt;
}

int weapon_catalogue_ammunition_per_ton(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->ammoperton;
}

int weapon_catalogue_explosion_damage(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->explosiondamage;
}

int weapon_catalogue_weight(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->weight;
}

int weapon_catalogue_cost(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->cost;
}

int weapon_catalogue_ammunition_cost(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->ammo_cost;
}

int weapon_catalogue_battle_value(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->battlevalue;
}

int weapon_catalogue_ammunition_battle_value(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->ammo_bv;
}

int weapon_catalogue_critical_slots(int weapon_index) {
  return weapon_catalogue_entry(weapon_index)->criticals;
}

WeaponRangeProfile weapon_catalogue_ranges(int weapon_index) {
  const struct WeaponDefinition *weapon = weapon_catalogue_entry(weapon_index);
  return (WeaponRangeProfile){
      .minimum = weapon->min,
      .short_range = weapon->shortrange,
      .medium_range = weapon->medrange,
      .long_range = weapon->longrange,
      .water_minimum = weapon->min_water,
      .water_short_range = weapon->shortrange_water,
      .water_medium_range = weapon->medrange_water,
      .water_long_range = weapon->longrange_water,
  };
}

int weapon_catalogue_cluster_size(int weapon_index) {
  const struct WeaponDefinition *weapon = weapon_catalogue_entry(weapon_index);
  return (weapon->special & (IDF | MRM | ROCKET)) && weapon->damage == 1 ? 5
                                                                         : 1;
}

int weapon_catalogue_effective_range(int weapon_index, bool extended) {
  const struct WeaponDefinition *weapon = weapon_catalogue_entry(weapon_index);
  int normal = weapon_catalogue_is_artillery(weapon_index)
                   ? ARTILLERY_MAPSHEET_SIZE * weapon->longrange
                   : weapon->longrange;
  int extended_range = weapon->medrange * 2;
  return extended && extended_range > normal ? extended_range : normal;
}

int weapon_catalogue_effective_water_range(int weapon_index, bool extended) {
  const struct WeaponDefinition *weapon = weapon_catalogue_entry(weapon_index);
  int normal = weapon->longrange_water > 0    ? weapon->longrange_water
               : weapon->medrange_water > 0   ? weapon->medrange_water
               : weapon->shortrange_water > 0 ? weapon->shortrange_water
                                              : 0;
  int extended_range = weapon->medrange_water * 2;
  return extended && extended_range > normal && weapon->longrange_water > 0
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
        *weapon_byte_slot(weapdataarray, weapcount) =
            clamp_int_to_unsigned_char(data);
        *weapon_byte_slot(weaparray, weapcount) =
            clamp_int_to_unsigned_char(temp);
        *weapon_critical_slot(critical, weapcount) = loop;
        weapcount++;
        num_crits = 1;
        continue;
      }
      if (!num_crits || temp != lastweap ||
          (num_crits == GetWeaponCrits(mech, temp))) {
        UGLYTEST;
        *weapon_byte_slot(weaparray, weapcount) =
            clamp_int_to_unsigned_char(temp);
        *weapon_byte_slot(weapdataarray, weapcount) =
            clamp_int_to_unsigned_char(data);
        *weapon_critical_slot(critical, weapcount) = loop;
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
          if (temp == *weapon_byte_slot(weaparray, i) &&
              mode == *weapon_mode_slot(modearray, i)) {
            if (!(mech_critical_is_nonfunctional(mech, index, loop)))
              *weapon_short_slot(ammoarray, i) = clamp_int_to_unsigned_short(
                  *weapon_short_slot(ammoarray, i) + data);
            *weapon_short_slot(ammomaxarray, i) = clamp_int_to_unsigned_short(
                *weapon_short_slot(ammomaxarray, i) +
                FullAmmo(mech, index, loop));
            duplicate = 1;
          }
        }

        if (!duplicate) {
          *weapon_byte_slot(weaparray, weapcount) =
              clamp_int_to_unsigned_char(temp);

          if (!(mech_critical_is_nonfunctional(mech, index, loop)))
            *weapon_short_slot(ammoarray, weapcount) =
                clamp_int_to_unsigned_short(data);
          else
            *weapon_short_slot(ammoarray, weapcount) = 0;

          *weapon_short_slot(ammomaxarray, weapcount) =
              clamp_int_to_unsigned_short(FullAmmo(mech, index, loop));
          *weapon_mode_slot(modearray, weapcount) = mode;

          weapcount++;
        }
      }
    }
  /* Then, prune entries with 0 ammo left */
  if (!returnall) {
    for (i = 0; i < weapcount; i++)
      if (!*weapon_short_slot(ammoarray, i)) {
        for (j = i + 1; j < weapcount; j++) {
          *weapon_byte_slot(weaparray, j - 1) = *weapon_byte_slot(weaparray, j);
          *weapon_short_slot(ammoarray, j - 1) =
              *weapon_short_slot(ammoarray, j);
          *weapon_short_slot(ammomaxarray, j - 1) =
              *weapon_short_slot(ammomaxarray, j);
          *weapon_mode_slot(modearray, j - 1) = *weapon_mode_slot(modearray, j);
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
      int critical_index = *weapon_critical_slot(critical, index);
      int weapon_index = *weapon_byte_slot(weaparray, index);
      if (mech_critical_is_nonfunctional(mech, loop, critical_index)) {
        *section = loop;
        *crit = critical_index;
        return TIC_NUM_DESTROYED;
      } else if (*weapon_byte_slot(weapdata, index) > 0 && !sight) {
        *section = loop;
        *crit = critical_index;
        return (weapon_catalogue_type(weapon_index) == TBEAM)
                   ? TIC_NUM_RECYCLING
                   : TIC_NUM_RELOADING;
      } else {

        if (mech_section_recycle_ticks(mech, loop) &&
            (((mech)->ud.type) == CLASS_MECH ||
             ((mech)->ud.type) == CLASS_VEH_GROUND ||
             ((mech)->ud.type) == CLASS_VTOL) &&
            !sight) {

          *section = loop;
          *crit = critical_index;
          /* just did a physical attack */
          return TIC_NUM_PHYSICAL;
        }

        /* The recylce data for the weapon is clear- it is ready to fire! */
        *section = loop;
        *crit = critical_index;
        return weapon_index;
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
      if (*weapon_byte_slot(weaparray, index) == weapindx) {
        *section = loop;
        *crit = *weapon_critical_slot(critical, index);
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
      return *weapon_byte_slot(weaparray, index);
    }
    running_sum += num_weaps;
  }
  return -1;
}

int FullAmmo(const Mech *mech, int loc, int pos) {
  int baseammo;
  int overage;

  baseammo = weapon_catalogue_ammunition_per_ton(
      ammunition_to_weapon_index(mech_critical_part_type(mech, loc, pos)));
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
