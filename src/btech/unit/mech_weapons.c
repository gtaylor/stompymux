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
  return IsArtillery(weapon_index);
}

bool weapon_catalogue_supports_indirect_fire(int weapon_index) {
  return MechWeapons[weapon_index].special & IDF;
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

/* ASSERTION: Weapons must be located next to each other in criticals. */
int FindWeapons_Advanced(Mech *mech, int index, unsigned char *weaparray,
                         unsigned char *weapdataarray, int *critical,
                         int whine) {
  int loop;
  int weapcount = 0;
  int temp, data, lastweap = -1;
  int num_crits = 0, i;

  for (loop = 0; loop < MAX_WEAPS_SECTION; loop++) {
    temp = GetPartType(mech, index, loop);
    data = GetPartData(mech, index, loop);
    if (IsWeapon(temp)) {
      temp = Weapon2I(temp);
      if (weapcount == 0) {
        lastweap = temp;
        weapdataarray[weapcount] = data;
        weaparray[weapcount] = temp;
        critical[weapcount] = loop;
        weapcount++;
        num_crits = 1;
        continue;
      }
      if (!num_crits || temp != lastweap ||
          (num_crits == GetWeaponCrits(mech, temp))) {
        UGLYTEST;
        weaparray[weapcount] = temp;
        weapdataarray[weapcount] = data;
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
      temp = GetPartType(mech, index, loop);
      if (IsAmmo(temp)) {
        data = GetPartData(mech, index, loop);
        mode = (GetPartAmmoMode(mech, index, loop) & AMMO_MODES);
        temp = Ammo2Weapon(temp);
        duplicate = 0;

        for (i = 0; i < weapcount; i++) {
          if (temp == weaparray[i] && mode == modearray[i]) {
            if (!(PartIsNonfunctional(mech, index, loop)))
              ammoarray[i] += data;
            ammomaxarray[i] += FullAmmo(mech, index, loop);
            duplicate = 1;
          }
        }

        if (!duplicate) {
          weaparray[weapcount] = temp;

          if (!(PartIsNonfunctional(mech, index, loop)))
            ammoarray[weapcount] = data;
          else
            ammoarray[weapcount] = 0;

          ammomaxarray[weapcount] = FullAmmo(mech, index, loop);
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
    if (GetPartType(mech, LLEG, loop) == I2Special((HEAT_SINK)) &&
        !PartIsNonfunctional(mech, LLEG, loop))
      heatsinks++;
    if (GetPartType(mech, RLEG, loop) == I2Special((HEAT_SINK)) &&
        !PartIsNonfunctional(mech, RLEG, loop))
      heatsinks++;
    /*
     * Added by Kipsta on 8/5/99
     * Quads can get 'arm' HS in the water too
     */

    if (MechIsQuad(mech)) {
      if (GetPartType(mech, LARM, loop) == I2Special((HEAT_SINK)) &&
          !PartIsNonfunctional(mech, LARM, loop))
        heatsinks++;
      if (GetPartType(mech, RARM, loop) == I2Special((HEAT_SINK)) &&
          !PartIsNonfunctional(mech, RARM, loop))
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
    num_weaps = FindWeapons(mech, loop, weaparray, weapdata, critical);

    if (num_weaps <= 0)
      continue;

    if (number < running_sum + num_weaps) {
      /* we found it... */
      index = number - running_sum;
      if (PartIsNonfunctional(mech, loop, critical[index])) {
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

        if (MechSections(mech)[loop].recycle &&
            (MechType(mech) == CLASS_MECH ||
             MechType(mech) == CLASS_VEH_GROUND ||
             MechType(mech) == CLASS_VTOL) &&
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
    num_weaps = FindWeapons(mech, loop, weaparray, weapdata, critical);
    for (index = 0; index < num_weaps; index++)
      if (weaparray[index] == weapindx) {
        *section = loop;
        *crit = critical[index];
        if (!PartIsNonfunctional(mech, loop, index) &&
            !WpnIsRecycling(mech, loop, index))
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
    num_weaps = FindWeapons(mech, loop, weaparray, weapdata, critical);
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

int FullAmmo(Mech *mech, int loc, int pos) {
  int baseammo;
  int overage;

  baseammo = MechWeapons[Ammo2I(GetPartType(mech, loc, pos))].ammoperton;
  if ((GetPartAmmoMode(mech, loc, pos) & AC_AP_MODE) ||
      (GetPartAmmoMode(mech, loc, pos) & AC_PRECISION_MODE) ||
      (GetPartFireMode(mech, loc, pos) & HALFTON_MODE)) {
    return baseammo >> 1;
  }

  if ((GetPartAmmoMode(mech, loc, pos) & AC_CASELESS_MODE)) {
    return baseammo << 1;
  }

  if ((GetPartAmmoMode(mech, loc, pos) & MML_LRM_MODE)) {
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
    if (GetPartType(mech, section, wIter) == type &&
        !PartIsNonfunctional(mech, section, wIter) &&
        (!nogof || !(GetPartAmmoMode(mech, section, wIter) & nogof)) &&
        (!gof || (GetPartAmmoMode(mech, section, wIter) & gof))) {

      if (!PartIsNonfunctional(mech, section, wIter) &&
          GetPartData(mech, section, wIter) > 0)
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

  desired = I2Ammo(weapindx);

  /* The data on the desired location */
  if ((weapSection > -1) && (weapCritical > -1)) {
    wCritType = GetPartType(mech, weapSection, weapCritical);
    wWeapSize = GetWeaponCrits(mech, Weapon2I(wCritType));
    wFirstCrit = FindFirstWeaponCrit(mech, weapSection, weapCritical, 0,
                                     wCritType, wWeapSize);

    wDesiredLoc = GetPartDesiredAmmoLoc(mech, weapSection, wFirstCrit);

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

  wAmmoIdx = I2Ammo(weapindx);

  for (wSecIter = 0; wSecIter < NUM_SECTIONS; wSecIter++) {
    for (wSlotIter = 0; wSlotIter < NUM_CRITICALS; wSlotIter++) {
      if ((GetPartType(mech, wSecIter, wSlotIter) == wAmmoIdx) &&
          !PartIsNonfunctional(mech, wSecIter, wSlotIter) &&
          (GetPartData(mech, wSecIter, wSlotIter) > 0))
        wcAmmo += GetPartData(mech, wSecIter, wSlotIter);
    }
  }

  return wcAmmo;
}

/* Function taken from 3065. Credit to RebelST) */
int FindArtemisForWeapon(Mech *mech, int section, int critical) {
  int critloop;
  int desired;

  desired = I2Special(ARTEMIS_IV);
  for (critloop = 0; critloop < NUM_CRITICALS; critloop++) {
    if (GetPartType(mech, section, critloop) == desired &&
        !PartIsNonfunctional(mech, section, critloop)) {
      if (GetPartData(mech, section, critloop) == (critical + 1))
        return 1;
    }
  }
  if (MechType(mech) == CLASS_MECH &&
      section == CTORSO) { // if it's mech, and torso missile, search in head
    for (critloop = 0; critloop < 6; critloop++) {
      if (GetPartType(mech, HEAD, critloop) == desired &&
          !PartIsNonfunctional(mech, HEAD, critloop)) {
        if (GetPartData(mech, HEAD, critloop) == (critical + 1))
          return 1;
      }
    }
  } else if (MechType(mech) == CLASS_VEH_GROUND &&
             section == TURRET) { // same thing for turret & aft
    for (critloop = 0; critloop < NUM_CRITICALS; critloop++) {
      if (GetPartType(mech, BSIDE, critloop) == desired &&
          !PartIsNonfunctional(mech, BSIDE, critloop)) {
        if (GetPartData(mech, BSIDE, critloop) == (critical + 1))
          return 1;
      }
    }
  }
  return 0;
}

int ReverseSplitCritLoc(Mech *mech, int sect, int crit) {
  if (MechType(mech) != CLASS_MECH)
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
    return (Special2I(GetPartType(mech, sect, crit)) == SPLIT_CRIT_RIGHT
                ? RTORSO
                : LTORSO);
  }
  return -1;
}

int FindSplitCrits(Mech *mech, int sect, int type, int crit) {
  int i;

  for (i = 0; i < CritsInLoc(mech, sect); i++)
    if (GetPartType(mech, sect, i) == type &&
        GetPartData(mech, sect, i) == crit)
      return i;

  return -1;
}
int GetSplitData(Mech *mech, int sect, int data, int *ssect, int *scrit,
                 int *stype) {
  switch (sect) {
  case RARM: // right arm goes to right torso
    *stype = I2Special(SPLIT_CRIT_RIGHT);
    if ((*scrit = FindSplitCrits(mech, RTORSO, *stype, data)) >= 0) {
      *ssect = RTORSO;
      return 1;
    }
    break;
  case LARM: // left arm goes to left torso
    *stype = I2Special(SPLIT_CRIT_LEFT);
    if ((*scrit = FindSplitCrits(mech, LTORSO, *stype, data)) >= 0) {
      *ssect = LTORSO;
      return 1;
    }
    break;
  case RTORSO: // torso more complex, need to go thru arm, leg, torso
    *stype = I2Special(SPLIT_CRIT_RIGHT);
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
    *stype = I2Special(SPLIT_CRIT_LEFT);
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
      if (IsAmmo(GetPartType(mech, loop, critloop)) &&
          !PartIsDestroyed(mech, loop, critloop)) {
        data = GetPartData(mech, loop, critloop);
        type = GetPartType(mech, loop, critloop);
        weapindx = Ammo2WeaponI(type);
        damage = data * MechWeapons[weapindx].damage;
        if (MechWeapons[weapindx].special & GAUSS)
          continue;
        if (IsMissile(weapindx) || IsArtillery(weapindx)) {
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
      if (IsAmmo(GetPartType(mech, loop, critloop)) &&
          !PartIsDestroyed(mech, loop, critloop)) {
        data = GetPartData(mech, loop, critloop);
        type = GetPartType(mech, loop, critloop);
        mode = GetPartAmmoMode(mech, loop, critloop);
        if (!(mode & INFERNO_MODE))
          continue;
        weapindx = Ammo2WeaponI(type);
        damage = data * MechWeapons[weapindx].damage;
        if (MechWeapons[weapindx].special & GAUSS)
          continue;
        if (IsMissile(weapindx) || IsArtillery(weapindx)) {
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

  desired = I2Ammo(weapindx);
  for (loop = 0; loop < NUM_SECTIONS; loop++)
    for (critloop = 0; critloop < NUM_CRITICALS; critloop++)
      if (GetPartType(mech, loop, critloop) == desired &&
          !PartIsNonfunctional(mech, loop, critloop))
        found += GetPartData(mech, loop, critloop);
  return found;
}

char *quad_locs[NUM_SECTIONS + 1] = {"Front Left Leg",
                                     "Front Right Leg",
                                     "Left Torso",
                                     "Right Torso",
                                     "Center Torso",
                                     "Rear Left Leg",
                                     "Rear Right Leg",
                                     "Head",
                                     NULL};

char *mech_locs[NUM_SECTIONS + 1] = {
    "Left Arm", "Right Arm", "Left Torso", "Right Torso", "Center Torso",
    "Left Leg", "Right Leg", "Head",       NULL};

char *bsuit_locs[NUM_BSUIT_MEMBERS + 1] = {"Suit 1", "Suit 2", "Suit 3",
                                           "Suit 4", "Suit 5", "Suit 6",
                                           "Suit 7", "Suit 8", NULL};

char *veh_locs[NUM_VEH_SECTIONS + 1] = {"Left Side", "Right Side", "Front Side",
                                        "Aft Side",  "Turret",     "Rotor",
                                        NULL};

char *aero_locs[NUM_AERO_SECTIONS + 1] = {"Nose", "Left Wing", "Right Wing",
                                          "Aft Side", NULL};

char *ds_locs[NUM_DS_SECTIONS + 1] = {
    "Right Wing", "Left Wing", "Left Rear Wing", "Right Rear Wing", "Aft",
    "Nose",       NULL};

char *ds_spher_locs[NUM_DS_SECTIONS + 1] = {"Front Right Side",
                                            "Front Left Side",
                                            "Rear Left Side",
                                            "Rear Right Side",
                                            "Aft",
                                            "Nose",
                                            NULL};

char **ProperSectionStringFromType(int type, int mtype) {
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
